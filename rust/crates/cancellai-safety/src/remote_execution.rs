//! Remote execution request verification (E18-S02, SI-031, TM-17, [RFC-0001](../../../../docs/rfcs/0001-remote-execution-boundary.md),
//! [ADR-0031](../../../../docs/adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md)).
//!
//! A remote controller (a fleet server, a CI job, another of the user's own machines) is a
//! different actor from an [`cancellai_model::RemoteTarget`] (E18-S01, the machine *being*
//! managed) - this module authenticates and bounds the former's requests, never models the
//! latter. [`RemoteExecutionRequest`] mirrors [`crate::knowledge_bundle::KnowledgeBundle`]'s
//! shape and verification exactly (same `schema_version`/`sequence`/`expires_at`/
//! `content_digest`/`signature` fields, same ed25519/SHA-256 primitives), because it answers
//! the identical question - "did an identified, trusted party really say this, and is it still
//! current" - for a different payload. Unlike `KnowledgeBundle`'s opaque `payload` (reused
//! across several knowledge kinds), this envelope's payload is flat and narrow by design: one
//! `target` [`MachineId`] and one `requested_authority` [`AuthorityLevel`] - nothing else,
//! because nothing else is safe for a request to say.
//!
//! **AC1 ("remote control cannot bypass target safety invariants") is enforced by what this
//! module refuses to accept and refuses to produce**: the payload cannot express a plan, a
//! `SealedPlan` reference, or any `AuthorityInputs` field other than `user_requested` - there is
//! no field for any of those, the same "shape cannot express the claim" pattern
//! `knowledge_bundle`'s own module doc uses for capability/trust claims. [`VerifiedRemoteIntent`]
//! is the only thing [`RemoteExecutionLog::verify_and_record`] can produce from a successful
//! check, and its `requested_authority` is proven (never merely assumed) to be at or below the
//! controller's own [`TrustedRemoteController::authority_ceiling`] before it is ever returned -
//! a request asking for more than its controller is trusted for is rejected outright, never
//! silently clamped down to the ceiling, matching "ambiguity never escalates privilege" (C-03).
//! Plugging the result into [`crate::authority::AuthorityInputs::user_requested`] changes
//! nothing about [`crate::authority::effective_authority`] itself - every other input
//! (`artifact_ceiling`, `confidence`, `activity`, `protection`, `integrity`, `provider_trust`)
//! is still supplied by the target's own local observation and policy resolution, untouched by
//! this module. `remote_and_local_user_requested_reach_identical_effective_authority` below
//! proves there is no second, hidden authority path for the remote case.
//!
//! **AC2 ("every remote request is authenticated, authorized, and audit-linked")**:
//! authenticated by signature verification against a [`TrustedRemoteController`] looked up by
//! `controller_id` (never a trust claim the request carries itself); authorized by that
//! controller's `authority_ceiling` and, separately, by [`RemoteExecutionLog`]'s replay check
//! (mirrors [`crate::knowledge_bundle::KnowledgeStore`]'s own sequence-staleness check exactly -
//! this crate already holds one piece of small, pure in-memory state of this shape, so a second
//! does not change its dependency posture). Audit-linked without adding a dependency:
//! `cancellai-safety` is kernel ring and `cancellai-store` (which owns `EventLedger`) is outer
//! ring specifically so the kernel stays free of `rusqlite` (ADR-0019), so this module cannot
//! write a ledger entry itself. Every call to [`verify_remote_execution_request`] or
//! [`RemoteExecutionLog::verify_and_record`] returns a `Result` whose `Ok`/`Err` together with
//! the `request` the caller already holds carries every field an `EventLedger` entry needs
//! (controller, sequence, target, requested authority, and - on rejection - the specific
//! [`RemoteExecutionError`]); writing that entry is left to whichever outer-ring caller
//! eventually wires a real transport to this function (E18-S03 or later), the same
//! library-primitive-first precedent E13's ledger and E18-S01's `RemoteTarget` already shipped.

use std::collections::HashMap;
use std::fmt;

use cancellai_model::{AuthorityLevel, MachineId};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

/// Schema versions this module accepts, inclusive - see [`crate::knowledge_bundle`]'s identical
/// constant for why a range rather than one fixed value.
pub const SUPPORTED_SCHEMA_VERSIONS: (u32, u32) = (1, 1);

/// A remote controller's request, as received (not yet verified). See the module doc for what
/// this shape deliberately cannot express.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteExecutionRequest {
    pub schema_version: u32,
    /// Identifies which [`TrustedRemoteController`] to verify against - a lookup key, not a
    /// trust claim.
    pub controller_id: String,
    /// Strictly increasing per controller - [`RemoteExecutionLog::verify_and_record`] uses this
    /// to refuse a replayed or out-of-order request from the same controller.
    pub sequence: u64,
    /// Unix seconds.
    pub issued_at: u64,
    /// Unix seconds; `None` means the request never expires on its own (still subject to the
    /// replay check above).
    pub expires_at: Option<u64>,
    /// Which target this request concerns. The target's own local policy resolution - not this
    /// request - decides which of its artifacts, if any, this authority actually reaches.
    pub target: MachineId,
    /// What the controller is asking the target to evaluate up to - never a plan, never an
    /// already-decided outcome. Checked against the controller's own ceiling below; never
    /// trusted as-is.
    pub requested_authority: AuthorityLevel,
    /// Lowercase hex SHA-256 of `target` and `requested_authority`
    /// ([`RemoteExecutionRequest::payload_bytes`]), computed by the controller before signing.
    /// Verified independently of the signature so a bug or attack that targeted only one of the
    /// two checks is still caught by the other.
    pub content_digest: String,
    /// Lowercase hex Ed25519 signature (64 bytes) over [`RemoteExecutionRequest::signing_bytes`].
    pub signature: String,
}

impl RemoteExecutionRequest {
    /// The exact bytes [`TrustedRemoteController::public_key`] must have signed. Length-prefixes
    /// every field (big-endian `u64` length, then the field's own bytes) rather than joining
    /// them with a delimiter, for the identical reason `KnowledgeBundle::signing_bytes` does -
    /// a delimiter inside `controller_id` could otherwise let two different logical requests
    /// serialize to the same byte string. `signature` itself is never included.
    ///
    /// `content_digest` is included as an already-computed field (see
    /// [`RemoteExecutionRequest::payload_bytes`]) - it is never derived from these bytes, which
    /// would be circular, only signed alongside them.
    fn signing_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut push_field = |field: &[u8]| {
            buf.extend_from_slice(&(field.len() as u64).to_be_bytes());
            buf.extend_from_slice(field);
        };
        push_field(&self.schema_version.to_be_bytes());
        push_field(self.controller_id.as_bytes());
        push_field(&self.sequence.to_be_bytes());
        push_field(&self.issued_at.to_be_bytes());
        push_field(&self.expires_at.unwrap_or(0).to_be_bytes());
        push_field(&[u8::from(self.expires_at.is_some())]);
        push_field(self.content_digest.as_bytes());
        push_field(&self.payload_bytes());
        buf
    }

    /// The "what is being requested" fields alone - `target` and `requested_authority` - the
    /// bytes `content_digest` is the SHA-256 of. Kept separate from [`Self::signing_bytes`]
    /// (which includes the already-computed digest as one more field) so digest computation is
    /// never circular.
    fn payload_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut push_field = |field: &[u8]| {
            buf.extend_from_slice(&(field.len() as u64).to_be_bytes());
            buf.extend_from_slice(field);
        };
        push_field(self.target.as_str().as_bytes());
        push_field(
            serde_json::to_string(&self.requested_authority)
                .expect("AuthorityLevel serialization is infallible")
                .as_bytes(),
        );
        buf
    }
}

/// Parses `text` as a [`RemoteExecutionRequest`]. The sole entry point from untrusted bytes - a
/// request missing any required field (including a wholly unsigned one, which is missing
/// `signature`) or carrying an unrecognized field fails here, before verification is ever
/// reached, with a diagnostic naming what was wrong.
pub fn parse_request(text: &str) -> Result<RemoteExecutionRequest, String> {
    serde_json::from_str(text).map_err(|error| format!("invalid remote execution request: {error}"))
}

/// One remote controller this installation trusts, the ed25519 public key it signs with, and
/// the ceiling [`AuthorityLevel`] its requests may reach - assigned by local policy (a human
/// decision), never read from a request itself.
#[derive(Debug, Clone)]
pub struct TrustedRemoteController {
    pub controller_id: String,
    pub public_key: [u8; 32],
    pub authority_ceiling: AuthorityLevel,
}

/// The set of remote controllers this installation trusts. Not `pub` fields - constructed once
/// and looked up by controller id, so a caller cannot mutate it into containing a
/// controller/ceiling pairing the local operator never actually chose.
#[derive(Debug, Clone, Default)]
pub struct TrustedRemoteControllers {
    controllers: Vec<TrustedRemoteController>,
}

impl TrustedRemoteControllers {
    pub fn new(controllers: Vec<TrustedRemoteController>) -> Self {
        Self { controllers }
    }

    fn find(&self, controller_id: &str) -> Option<&TrustedRemoteController> {
        self.controllers
            .iter()
            .find(|c| c.controller_id == controller_id)
    }
}

/// Why verification refused a request. Every variant is fail-closed: no partial authority is
/// ever granted on the way to one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteExecutionError {
    UnsupportedSchemaVersion,
    /// `controller_id` names no [`TrustedRemoteController`] in the [`TrustedRemoteControllers`]
    /// consulted.
    UnknownController,
    /// `signature` was not exactly 64 bytes of valid hex.
    MalformedSignature,
    /// SHA-256 of the signed fields does not match `content_digest`.
    ContentDigestMismatch,
    /// The Ed25519 signature does not verify against the controller's public key.
    InvalidSignature,
    /// `expires_at` is at or before the verification clock.
    Expired,
    /// `requested_authority` exceeds the controller's own `authority_ceiling`. Never clamped -
    /// refused outright (C-03: ambiguity never escalates privilege).
    RequestedAuthorityExceedsCeiling,
    /// [`RemoteExecutionLog::verify_and_record`] only: `sequence` does not strictly exceed the
    /// last accepted sequence from the same controller.
    Stale,
}

impl fmt::Display for RemoteExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::UnsupportedSchemaVersion => "unsupported remote execution request schema version",
            Self::UnknownController => "unknown remote controller",
            Self::MalformedSignature => "malformed remote execution request signature",
            Self::ContentDigestMismatch => "remote execution request content digest mismatch",
            Self::InvalidSignature => "invalid remote execution request signature",
            Self::Expired => "remote execution request has expired",
            Self::RequestedAuthorityExceedsCeiling => {
                "requested authority exceeds the controller's trusted ceiling"
            }
            Self::Stale => "remote execution request is not newer than the last accepted one",
        };
        f.write_str(text)
    }
}

/// A [`RemoteExecutionRequest`] that has passed every check in [`verify_remote_execution_request`].
/// `requested_authority` is copied verbatim from the request - already proven to be at or below
/// the controller's ceiling - never anything computed or inferred here. This is the only value
/// this module hands a caller; feeding `requested_authority` into
/// [`crate::authority::AuthorityInputs::user_requested`] is the caller's own, separate step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRemoteIntent {
    pub controller_id: String,
    pub sequence: u64,
    pub target: MachineId,
    pub requested_authority: AuthorityLevel,
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verifies `request` against `policy` at `now_unix` (Unix seconds - threaded in explicitly
/// rather than read from the system clock, so tests can exercise expiry deterministically).
/// Stateless: performs every check *except* replay/staleness, which needs to compare against
/// previously accepted requests - see [`RemoteExecutionLog::verify_and_record`] for that.
/// Never returns a [`VerifiedRemoteIntent`] for a request that failed any check.
pub fn verify_remote_execution_request(
    request: &RemoteExecutionRequest,
    policy: &TrustedRemoteControllers,
    now_unix: u64,
) -> Result<VerifiedRemoteIntent, RemoteExecutionError> {
    if request.schema_version < SUPPORTED_SCHEMA_VERSIONS.0
        || request.schema_version > SUPPORTED_SCHEMA_VERSIONS.1
    {
        return Err(RemoteExecutionError::UnsupportedSchemaVersion);
    }

    let controller = policy
        .find(&request.controller_id)
        .ok_or(RemoteExecutionError::UnknownController)?;

    let expected_digest = encode_hex(&Sha256::digest(request.payload_bytes()));
    if expected_digest != request.content_digest {
        return Err(RemoteExecutionError::ContentDigestMismatch);
    }

    let signature_bytes = decode_hex(&request.signature)
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .ok_or(RemoteExecutionError::MalformedSignature)?;
    let signature = Signature::from_bytes(&signature_bytes);
    let verifying_key = VerifyingKey::from_bytes(&controller.public_key)
        .map_err(|_| RemoteExecutionError::UnknownController)?;
    verifying_key
        .verify_strict(&request.signing_bytes(), &signature)
        .map_err(|_| RemoteExecutionError::InvalidSignature)?;

    if request
        .expires_at
        .is_some_and(|expires_at| now_unix >= expires_at)
    {
        return Err(RemoteExecutionError::Expired);
    }

    if request.requested_authority > controller.authority_ceiling {
        return Err(RemoteExecutionError::RequestedAuthorityExceedsCeiling);
    }

    Ok(VerifiedRemoteIntent {
        controller_id: request.controller_id.clone(),
        sequence: request.sequence,
        target: request.target.clone(),
        requested_authority: request.requested_authority,
    })
}

/// Holds, per controller, the highest sequence number accepted so far - the smallest amount of
/// state a replay check needs. Pure in-memory Rust data, mirroring
/// [`crate::knowledge_bundle::KnowledgeStore`]'s own shape exactly: this crate already carries
/// one piece of state like this, so a second does not change `cancellai-safety`'s dependency
/// posture (no file I/O, no new crate).
#[derive(Debug, Clone, Default)]
pub struct RemoteExecutionLog {
    last_sequence: HashMap<String, u64>,
}

impl RemoteExecutionLog {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Verifies `request`, then - only if every other check passed - refuses it as
    /// [`RemoteExecutionError::Stale`] when `sequence` does not strictly exceed the last
    /// accepted sequence from the *same* controller, and records the new sequence otherwise.
    /// Scoped per controller: a first request from a controller this log has not seen before is
    /// never stale, and a rejected request never advances the recorded sequence.
    pub fn verify_and_record(
        &mut self,
        request: &RemoteExecutionRequest,
        policy: &TrustedRemoteControllers,
        now_unix: u64,
    ) -> Result<VerifiedRemoteIntent, RemoteExecutionError> {
        let verified = verify_remote_execution_request(request, policy, now_unix)?;
        if self
            .last_sequence
            .get(&verified.controller_id)
            .is_some_and(|&last| verified.sequence <= last)
        {
            return Err(RemoteExecutionError::Stale);
        }
        self.last_sequence
            .insert(verified.controller_id.clone(), verified.sequence);
        Ok(verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authority::{AuthorityInputs, effective_authority};
    use crate::trust_promotion::TrustedTier;
    use cancellai_model::{ActivityState, IntegrityState, KnowledgeConfidence, ProtectionState};
    use ed25519_dalek::{Signer, SigningKey};

    fn keypair() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn signed_request(
        signing_key: &SigningKey,
        controller_id: &str,
        sequence: u64,
        expires_at: Option<u64>,
        requested_authority: AuthorityLevel,
    ) -> RemoteExecutionRequest {
        let mut request = RemoteExecutionRequest {
            schema_version: 1,
            controller_id: controller_id.to_string(),
            sequence,
            issued_at: 1_000,
            expires_at,
            target: MachineId::new("ci-runner-1"),
            requested_authority,
            content_digest: String::new(),
            signature: String::new(),
        };
        let digest = encode_hex(&Sha256::digest(request.payload_bytes()));
        request.content_digest = digest;
        let signature = signing_key.sign(&request.signing_bytes());
        request.signature = encode_hex(&signature.to_bytes());
        request
    }

    fn policy_with(
        controller_id: &str,
        ceiling: AuthorityLevel,
        signing_key: &SigningKey,
    ) -> TrustedRemoteControllers {
        TrustedRemoteControllers::new(vec![TrustedRemoteController {
            controller_id: controller_id.to_string(),
            public_key: signing_key.verifying_key().to_bytes(),
            authority_ceiling: ceiling,
        }])
    }

    #[test]
    fn a_correctly_signed_request_within_ceiling_verifies() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Quarantine);

        let verified = verify_remote_execution_request(&request, &policy, 1_500).expect("verify");
        assert_eq!(verified.controller_id, "fleet-1");
        assert_eq!(verified.requested_authority, AuthorityLevel::Quarantine);
        assert_eq!(verified.target, MachineId::new("ci-runner-1"));
    }

    #[test]
    fn unknown_controller_is_rejected_before_any_crypto_check() {
        let key = keypair();
        let policy = TrustedRemoteControllers::new(vec![]);
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);

        assert_eq!(
            verify_remote_execution_request(&request, &policy, 1_500),
            Err(RemoteExecutionError::UnknownController)
        );
    }

    #[test]
    fn unsupported_schema_version_is_rejected_before_any_crypto_check() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);
        request.schema_version = 99;

        assert_eq!(
            verify_remote_execution_request(&request, &policy, 1_500),
            Err(RemoteExecutionError::UnsupportedSchemaVersion)
        );
    }

    #[test]
    fn tamper_a_digest_updated_to_match_tampered_target_still_fails_signature_verification() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);
        // Attacker retargets the request and recomputes the digest, but cannot re-sign.
        request.target = MachineId::new("victim-machine");
        request.content_digest = encode_hex(&Sha256::digest(request.payload_bytes()));

        assert_eq!(
            verify_remote_execution_request(&request, &policy, 1_500),
            Err(RemoteExecutionError::InvalidSignature)
        );
    }

    #[test]
    fn tamper_a_payload_changed_after_signing_fails_content_digest_check() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);
        request.requested_authority = AuthorityLevel::Autopilot;
        // Digest and signature are left as originally computed - the tamper is detected before
        // the (also-failing) signature check is even reached, because digest is checked first.

        assert_eq!(
            verify_remote_execution_request(&request, &policy, 1_500),
            Err(RemoteExecutionError::ContentDigestMismatch)
        );
    }

    #[test]
    fn malformed_signature_text_is_rejected_without_panicking() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);
        request.signature = "not hex".to_string();

        assert_eq!(
            verify_remote_execution_request(&request, &policy, 1_500),
            Err(RemoteExecutionError::MalformedSignature)
        );
    }

    #[test]
    fn expiry_at_exactly_the_boundary_is_rejected() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let request = signed_request(&key, "fleet-1", 1, Some(2_000), AuthorityLevel::Observe);

        assert!(verify_remote_execution_request(&request, &policy, 1_999).is_ok());
        assert_eq!(
            verify_remote_execution_request(&request, &policy, 2_000),
            Err(RemoteExecutionError::Expired)
        );
    }

    #[test]
    fn a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Autopilot);

        assert_eq!(
            verify_remote_execution_request(&request, &policy, 1_500),
            Err(RemoteExecutionError::RequestedAuthorityExceedsCeiling)
        );
    }

    #[test]
    fn a_request_naming_exactly_the_ceiling_is_accepted() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Quarantine);

        assert!(verify_remote_execution_request(&request, &policy, 1_500).is_ok());
    }

    #[test]
    fn a_first_request_from_a_new_controller_is_never_stale() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut log = RemoteExecutionLog::empty();
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);

        assert!(log.verify_and_record(&request, &policy, 1_500).is_ok());
    }

    #[test]
    fn a_replayed_equal_sequence_is_rejected() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut log = RemoteExecutionLog::empty();
        let first = signed_request(&key, "fleet-1", 5, None, AuthorityLevel::Observe);
        log.verify_and_record(&first, &policy, 1_500)
            .expect("first accepted");

        let replayed = signed_request(&key, "fleet-1", 5, None, AuthorityLevel::Observe);
        assert_eq!(
            log.verify_and_record(&replayed, &policy, 1_500),
            Err(RemoteExecutionError::Stale)
        );
    }

    #[test]
    fn an_out_of_order_lower_sequence_is_rejected_even_from_the_same_controller() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut log = RemoteExecutionLog::empty();
        let newer = signed_request(&key, "fleet-1", 9, None, AuthorityLevel::Observe);
        log.verify_and_record(&newer, &policy, 1_500)
            .expect("higher sequence accepted");

        let older = signed_request(&key, "fleet-1", 3, None, AuthorityLevel::Observe);
        assert_eq!(
            log.verify_and_record(&older, &policy, 1_500),
            Err(RemoteExecutionError::Stale)
        );
    }

    #[test]
    fn a_rejected_request_never_advances_the_recorded_sequence() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let mut log = RemoteExecutionLog::empty();
        let accepted = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);
        log.verify_and_record(&accepted, &policy, 1_500)
            .expect("accepted");

        // Sequence 2 asks above the ceiling - rejected, but must not "use up" sequence 2.
        let over_ceiling = signed_request(&key, "fleet-1", 2, None, AuthorityLevel::Autopilot);
        assert_eq!(
            log.verify_and_record(&over_ceiling, &policy, 1_500),
            Err(RemoteExecutionError::RequestedAuthorityExceedsCeiling)
        );

        // Sequence 2, now correctly scoped, must still be accepted - proving the rejected
        // attempt above left the log's recorded sequence at 1, not 2.
        let retried = signed_request(&key, "fleet-1", 2, None, AuthorityLevel::Observe);
        assert!(log.verify_and_record(&retried, &policy, 1_500).is_ok());
    }

    #[test]
    fn sequences_from_different_controllers_never_interfere() {
        let key_a = keypair();
        let key_b = SigningKey::from_bytes(&[9u8; 32]);
        let policy = TrustedRemoteControllers::new(vec![
            TrustedRemoteController {
                controller_id: "fleet-a".to_string(),
                public_key: key_a.verifying_key().to_bytes(),
                authority_ceiling: AuthorityLevel::Autopilot,
            },
            TrustedRemoteController {
                controller_id: "fleet-b".to_string(),
                public_key: key_b.verifying_key().to_bytes(),
                authority_ceiling: AuthorityLevel::Autopilot,
            },
        ]);
        let mut log = RemoteExecutionLog::empty();
        let a_high = signed_request(&key_a, "fleet-a", 100, None, AuthorityLevel::Observe);
        log.verify_and_record(&a_high, &policy, 1_500)
            .expect("fleet-a accepted at sequence 100");

        // fleet-b's own sequence 1 must not be treated as stale relative to fleet-a's 100.
        let b_low = signed_request(&key_b, "fleet-b", 1, None, AuthorityLevel::Observe);
        assert!(log.verify_and_record(&b_low, &policy, 1_500).is_ok());
    }

    #[test]
    fn malformed_json_is_rejected_by_parse_request_without_panicking() {
        assert!(parse_request("{ not json").is_err());
        assert!(parse_request("{}").is_err());
    }

    #[test]
    fn an_unrecognized_field_is_rejected_before_verification_is_ever_reached() {
        let key = keypair();
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Observe);
        let mut value = serde_json::to_value(&request).expect("serialize");
        value
            .as_object_mut()
            .expect("object")
            .insert("extra_field".to_string(), serde_json::json!("smuggled"));
        let text = serde_json::to_string(&value).expect("serialize");

        assert!(parse_request(&text).is_err());
    }

    #[test]
    fn remote_and_local_user_requested_reach_identical_effective_authority() {
        // Proves AC1's "no hidden second authority path": once extracted, a remote-originated
        // requested_authority is fed into AuthorityInputs exactly like a local one, and
        // effective_authority cannot tell the difference.
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, AuthorityLevel::Quarantine);
        let verified = verify_remote_execution_request(&request, &policy, 1_500).expect("verify");

        let shared_inputs = |user_requested: AuthorityLevel| AuthorityInputs {
            user_requested,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust: TrustedTier::untrusted(),
        };

        let from_remote = effective_authority(shared_inputs(verified.requested_authority));
        let from_local = effective_authority(shared_inputs(AuthorityLevel::Quarantine));
        assert_eq!(from_remote, from_local);
    }

    #[test]
    fn a_local_authority_decision_requires_zero_trusted_remote_controllers() {
        // E18-S03 (docs/PRODUCT.md "Open-source and commercial boundary"): a single-machine
        // workflow must be fully functional with no fleet/commercial coordination configured at
        // all. An empty TrustedRemoteControllers - the state of a fresh install that has never
        // heard of a remote controller - never enters effective_authority's computation; a local
        // user_requested value produces exactly the answer it always did.
        let empty_policy = TrustedRemoteControllers::new(vec![]);
        let _ = &empty_policy; // present, deliberately unused: proves the local path below never
        // needs to consult it.

        let local_inputs = AuthorityInputs {
            user_requested: AuthorityLevel::Quarantine,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust: TrustedTier::untrusted(),
        };

        // Untrusted provider_trust caps the result at Observe (provider_trust_ceiling) - the
        // exact value matters less here than that it is fully, deterministically computed from
        // local_inputs alone, with empty_policy never consulted.
        assert_eq!(
            effective_authority(local_inputs).level,
            AuthorityLevel::Observe
        );
    }
}
