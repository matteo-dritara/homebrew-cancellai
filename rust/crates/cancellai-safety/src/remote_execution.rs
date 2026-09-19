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
//! `target` [`MachineId`] and one `requested_action` [`ActionClass`] - never an [`AuthorityLevel`]
//! directly. [RFC-0001](../../../../docs/rfcs/0001-remote-execution-boundary.md) considered and
//! explicitly rejected the `AuthorityLevel`-over-the-wire shape ("Option B") as "the literal
//! shape TM-17 and SI-031 name as the threat"; [ADR-0032](../../../../docs/adrs/0032-remote-execution-requests-carry-actionclass-not-authoritylevel.md)
//! corrects an earlier version of this module that shipped Option B anyway. `ActionClass`'s
//! range is deliberately narrower than `AuthorityLevel`'s: [`crate::authority::
//! minimum_authority_for`] can only ever produce `Observe`/`Quarantine`/`Govern` from it, so a
//! remote controller can never request `Recommend` or `Autopilot` by construction, not by
//! convention.
//!
//! **AC1 ("remote control cannot bypass target safety invariants") is enforced by what this
//! module refuses to accept and refuses to produce**: the payload cannot express a plan, a
//! `SealedPlan` reference, or any `AuthorityInputs` field other than `user_requested` - there is
//! no field for any of those, the same "shape cannot express the claim" pattern
//! `knowledge_bundle`'s own module doc uses for capability/trust claims. [`VerifiedRemoteIntent`]
//! is the only thing [`RemoteExecutionLog::verify_and_record`] can produce from a successful
//! check, and its `requested_action` is proven (never merely assumed) to require no more than
//! the controller's own [`TrustedRemoteController::authority_ceiling`] - via
//! [`crate::authority::minimum_authority_for`], never a direct `AuthorityLevel`-to-`AuthorityLevel`
//! comparison against a wire-supplied value - before it is ever returned; a request asking for
//! more than its controller is trusted for is rejected outright, never silently clamped down to
//! the ceiling, matching "ambiguity never escalates privilege" (C-03). A caller feeds
//! `minimum_authority_for(verified.requested_action)` into
//! [`crate::authority::AuthorityInputs::user_requested`], which changes nothing about
//! [`crate::authority::effective_authority`] itself - every other input (`artifact_ceiling`,
//! `confidence`, `activity`, `protection`, `integrity`, `provider_trust`) is still supplied by
//! the target's own local observation and policy resolution, untouched by this module.
//! `remote_and_local_user_requested_reach_identical_effective_authority` below proves there is no
//! second, hidden authority path for the remote case.
//!
//! **AC2 ("every remote request is authenticated, authorized, and audit-linked")**:
//! authenticated by signature verification against a [`TrustedRemoteController`] looked up by
//! `controller_id` (never a trust claim the request carries itself); authorized by that
//! controller's `authority_ceiling` (via `minimum_authority_for(requested_action)`) and,
//! separately, by [`RemoteExecutionLog`]'s replay check (mirrors [`crate::knowledge_bundle::
//! KnowledgeStore`]'s own sequence-staleness check exactly - this crate already holds one piece
//! of small, pure in-memory state of this shape, so a second does not change its dependency
//! posture). Audit-linked without adding a dependency: `cancellai-safety` is kernel ring and
//! `cancellai-store` (which owns `EventLedger`) is outer ring specifically so the kernel stays
//! free of `rusqlite` (ADR-0019), so this module cannot write a ledger entry itself. Every call
//! to [`RemoteExecutionLog::verify_and_record`] returns a `Result` whose `Ok`/`Err` together with
//! the `request` the caller already holds carries every field an `EventLedger` entry needs
//! (controller, sequence, target, requested action, and - on rejection - the specific
//! [`RemoteExecutionError`]); writing that entry is left to whichever outer-ring caller
//! eventually wires a real transport to this function (E18-S03 or later), the same
//! library-primitive-first precedent E13's ledger and E18-S01's `RemoteTarget` already shipped.

use std::collections::HashMap;
use std::fmt;

use crate::authority::minimum_authority_for;
use cancellai_model::{ActionClass, AuthorityLevel, MachineId};
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
    /// What the controller is asking the target to evaluate - a semantic action class, never an
    /// `AuthorityLevel` directly (ADR-0032). Checked, via [`crate::authority::
    /// minimum_authority_for`], against the controller's own ceiling below; never trusted as-is.
    pub requested_action: ActionClass,
    /// Lowercase hex SHA-256 of `target` and `requested_action`
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

    /// The "what is being requested" fields alone - `target` and `requested_action` - the
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
            serde_json::to_string(&self.requested_action)
                .expect("ActionClass serialization is infallible")
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
/// decision), never read from a request itself. Read as "the minimum authority any `ActionClass`
/// this controller requests may need" (via [`crate::authority::minimum_authority_for`]), not as
/// "the exact authority level a request may name" - a request never names one (ADR-0032).
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
    /// `request.target` is not the [`MachineId`] the caller is verifying on behalf of - round 1
    /// independent verifier review's exact reproduction: nothing previously bound a verified
    /// request's authority to the target it named, so a request correctly signed for
    /// `ci-runner-1` could be fed into `AuthorityInputs::user_requested` while acting on a
    /// completely different local target. Checked against the caller-supplied expected target,
    /// never inferred, matching this module's "shape cannot express the claim" pattern.
    TargetMismatch,
    /// `expires_at` is at or before the verification clock.
    Expired,
    /// `minimum_authority_for(requested_action)` exceeds the controller's own
    /// `authority_ceiling`. Never clamped - refused outright (C-03: ambiguity never escalates
    /// privilege).
    RequestedActionExceedsCeiling,
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
            Self::TargetMismatch => {
                "remote execution request does not name the target it is being verified for"
            }
            Self::Expired => "remote execution request has expired",
            Self::RequestedActionExceedsCeiling => {
                "requested action exceeds the controller's trusted ceiling"
            }
            Self::Stale => "remote execution request is not newer than the last accepted one",
        };
        f.write_str(text)
    }
}

/// A [`RemoteExecutionRequest`] that has passed every check in [`verify_remote_execution_request`].
/// `requested_action` is copied verbatim from the request - already proven that
/// `minimum_authority_for(requested_action)` is at or below the controller's ceiling - never
/// anything computed or inferred here. This is the only value this module hands a caller; a
/// caller computes `crate::authority::minimum_authority_for(requested_action)` and feeds that
/// into [`crate::authority::AuthorityInputs::user_requested`] as its own, separate step
/// (ADR-0032: never a raw `AuthorityLevel` carried by the request itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRemoteIntent {
    pub controller_id: String,
    pub sequence: u64,
    pub target: MachineId,
    pub requested_action: ActionClass,
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
/// rather than read from the system clock, so tests can exercise expiry deterministically), for
/// the caller's own `expected_target`. Stateless: performs every check *except* replay/
/// staleness, which needs to compare against previously accepted requests. Never returns a
/// [`VerifiedRemoteIntent`] for a request that failed any check.
///
/// Deliberately not `pub`: round 1 independent verifier review reproduced calling this function
/// directly, twice, with the identical signed request, both calls succeeding - a public stateless
/// verifier is itself the replay bypass, independent of whether a well-behaved caller also has a
/// [`RemoteExecutionLog`] available. [`RemoteExecutionLog::verify_and_record`] is the only
/// production path that can ever produce a [`VerifiedRemoteIntent`] outside this module's own
/// tests, and it always consults the log.
fn verify_remote_execution_request(
    request: &RemoteExecutionRequest,
    policy: &TrustedRemoteControllers,
    expected_target: &MachineId,
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

    // Checked only once the signature is confirmed genuine: `target` is itself part of the
    // signed payload, so an attacker who merely re-targets a request (without a valid signature
    // for the new target) is already caught above as `InvalidSignature` - this check instead
    // catches a request that is genuinely, validly signed, but simply for a different target
    // than the one the caller is verifying on behalf of.
    if request.target != *expected_target {
        return Err(RemoteExecutionError::TargetMismatch);
    }

    if request
        .expires_at
        .is_some_and(|expires_at| now_unix >= expires_at)
    {
        return Err(RemoteExecutionError::Expired);
    }

    if minimum_authority_for(request.requested_action) > controller.authority_ceiling {
        return Err(RemoteExecutionError::RequestedActionExceedsCeiling);
    }

    Ok(VerifiedRemoteIntent {
        controller_id: request.controller_id.clone(),
        sequence: request.sequence,
        target: request.target.clone(),
        requested_action: request.requested_action,
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

    /// Verifies `request` against `policy` for `expected_target`, then - only if every other
    /// check passed - refuses it as [`RemoteExecutionError::Stale`] when `sequence` does not
    /// strictly exceed the last accepted sequence from the *same* controller, and records the
    /// new sequence otherwise. Scoped per controller: a first request from a controller this log
    /// has not seen before is never stale, and a rejected request never advances the recorded
    /// sequence. The only production path that can ever produce a [`VerifiedRemoteIntent`] -
    /// see [`verify_remote_execution_request`]'s own doc for why that function is not `pub`.
    pub fn verify_and_record(
        &mut self,
        request: &RemoteExecutionRequest,
        policy: &TrustedRemoteControllers,
        expected_target: &MachineId,
        now_unix: u64,
    ) -> Result<VerifiedRemoteIntent, RemoteExecutionError> {
        let verified = verify_remote_execution_request(request, policy, expected_target, now_unix)?;
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

    /// The target every `signed_request` below names, unless a test deliberately overrides it -
    /// so `verify_remote_execution_request`'s `expected_target` argument reads as "the local
    /// machine this request is actually for" at every call site.
    fn target() -> MachineId {
        MachineId::new("ci-runner-1")
    }

    fn signed_request(
        signing_key: &SigningKey,
        controller_id: &str,
        sequence: u64,
        expires_at: Option<u64>,
        requested_action: ActionClass,
    ) -> RemoteExecutionRequest {
        let mut request = RemoteExecutionRequest {
            schema_version: 1,
            controller_id: controller_id.to_string(),
            sequence,
            issued_at: 1_000,
            expires_at,
            target: MachineId::new("ci-runner-1"),
            requested_action,
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
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Quarantine);

        let verified =
            verify_remote_execution_request(&request, &policy, &target(), 1_500).expect("verify");
        assert_eq!(verified.controller_id, "fleet-1");
        assert_eq!(verified.requested_action, ActionClass::Quarantine);
        assert_eq!(verified.target, MachineId::new("ci-runner-1"));
    }

    #[test]
    fn unknown_controller_is_rejected_before_any_crypto_check() {
        let key = keypair();
        let policy = TrustedRemoteControllers::new(vec![]);
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);

        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 1_500),
            Err(RemoteExecutionError::UnknownController)
        );
    }

    #[test]
    fn unsupported_schema_version_is_rejected_before_any_crypto_check() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);
        request.schema_version = 99;

        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 1_500),
            Err(RemoteExecutionError::UnsupportedSchemaVersion)
        );
    }

    #[test]
    fn tamper_a_digest_updated_to_match_tampered_target_still_fails_signature_verification() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);
        // Attacker retargets the request and recomputes the digest, but cannot re-sign.
        request.target = MachineId::new("victim-machine");
        request.content_digest = encode_hex(&Sha256::digest(request.payload_bytes()));

        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 1_500),
            Err(RemoteExecutionError::InvalidSignature)
        );
    }

    #[test]
    fn tamper_a_payload_changed_after_signing_fails_content_digest_check() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);
        request.requested_action = ActionClass::Delete;
        // Digest and signature are left as originally computed - the tamper is detected before
        // the (also-failing) signature check is even reached, because digest is checked first.

        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 1_500),
            Err(RemoteExecutionError::ContentDigestMismatch)
        );
    }

    #[test]
    fn malformed_signature_text_is_rejected_without_panicking() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);
        request.signature = "not hex".to_string();

        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 1_500),
            Err(RemoteExecutionError::MalformedSignature)
        );
    }

    #[test]
    fn expiry_at_exactly_the_boundary_is_rejected() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let request = signed_request(&key, "fleet-1", 1, Some(2_000), ActionClass::Observe);

        assert!(verify_remote_execution_request(&request, &policy, &target(), 1_999).is_ok());
        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 2_000),
            Err(RemoteExecutionError::Expired)
        );
    }

    #[test]
    fn a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        // ActionClass::Delete needs AuthorityLevel::Govern (minimum_authority_for), above the
        // controller's Quarantine ceiling.
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Delete);

        assert_eq!(
            verify_remote_execution_request(&request, &policy, &target(), 1_500),
            Err(RemoteExecutionError::RequestedActionExceedsCeiling)
        );
    }

    #[test]
    fn a_request_naming_exactly_the_ceiling_is_accepted() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Quarantine);

        assert!(verify_remote_execution_request(&request, &policy, &target(), 1_500).is_ok());
    }

    #[test]
    fn no_action_class_ever_maps_to_recommend_or_autopilot() {
        // ADR-0032's own central safety claim: `ActionClass`'s wire vocabulary can only ever
        // resolve, via `minimum_authority_for`, to Observe/Quarantine/Govern - a remote
        // controller cannot request `Recommend` or `Autopilot` by construction, no matter how
        // permissive its ceiling is (checked here with the maximum possible ceiling, so no
        // ceiling check could ever be masking a reachable-but-refused case).
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);

        for action in [
            ActionClass::Observe,
            ActionClass::Quarantine,
            ActionClass::Archive,
            ActionClass::Restore,
            ActionClass::Delete,
        ] {
            let request = signed_request(&key, "fleet-1", 1, None, action);
            let verified = verify_remote_execution_request(&request, &policy, &target(), 1_500)
                .expect("verify");
            let resolved = minimum_authority_for(verified.requested_action);
            assert_ne!(
                resolved,
                AuthorityLevel::Recommend,
                "{action:?} must never resolve to Recommend"
            );
            assert_ne!(
                resolved,
                AuthorityLevel::Autopilot,
                "{action:?} must never resolve to Autopilot"
            );
        }
    }

    #[test]
    fn a_correctly_signed_request_for_a_different_target_is_rejected() {
        // Round 1 independent verifier review's exact reproduction: a request correctly signed
        // and valid for "ci-runner-1" must never verify against a caller checking on behalf of a
        // different machine - otherwise a verified request for one target could supply authority
        // for another.
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Quarantine);

        assert_eq!(
            verify_remote_execution_request(
                &request,
                &policy,
                &MachineId::new("another-machine"),
                1_500
            ),
            Err(RemoteExecutionError::TargetMismatch)
        );
    }

    #[test]
    fn a_first_request_from_a_new_controller_is_never_stale() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut log = RemoteExecutionLog::empty();
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);

        assert!(
            log.verify_and_record(&request, &policy, &target(), 1_500)
                .is_ok()
        );
    }

    #[test]
    fn a_replayed_equal_sequence_is_rejected() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut log = RemoteExecutionLog::empty();
        let first = signed_request(&key, "fleet-1", 5, None, ActionClass::Observe);
        log.verify_and_record(&first, &policy, &target(), 1_500)
            .expect("first accepted");

        let replayed = signed_request(&key, "fleet-1", 5, None, ActionClass::Observe);
        assert_eq!(
            log.verify_and_record(&replayed, &policy, &target(), 1_500),
            Err(RemoteExecutionError::Stale)
        );
    }

    #[test]
    fn an_out_of_order_lower_sequence_is_rejected_even_from_the_same_controller() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Autopilot, &key);
        let mut log = RemoteExecutionLog::empty();
        let newer = signed_request(&key, "fleet-1", 9, None, ActionClass::Observe);
        log.verify_and_record(&newer, &policy, &target(), 1_500)
            .expect("higher sequence accepted");

        let older = signed_request(&key, "fleet-1", 3, None, ActionClass::Observe);
        assert_eq!(
            log.verify_and_record(&older, &policy, &target(), 1_500),
            Err(RemoteExecutionError::Stale)
        );
    }

    #[test]
    fn a_rejected_request_never_advances_the_recorded_sequence() {
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let mut log = RemoteExecutionLog::empty();
        let accepted = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);
        log.verify_and_record(&accepted, &policy, &target(), 1_500)
            .expect("accepted");

        // Sequence 2 asks above the ceiling - rejected, but must not "use up" sequence 2.
        let over_ceiling = signed_request(&key, "fleet-1", 2, None, ActionClass::Delete);
        assert_eq!(
            log.verify_and_record(&over_ceiling, &policy, &target(), 1_500),
            Err(RemoteExecutionError::RequestedActionExceedsCeiling)
        );

        // Sequence 2, now correctly scoped, must still be accepted - proving the rejected
        // attempt above left the log's recorded sequence at 1, not 2.
        let retried = signed_request(&key, "fleet-1", 2, None, ActionClass::Observe);
        assert!(
            log.verify_and_record(&retried, &policy, &target(), 1_500)
                .is_ok()
        );
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
        let a_high = signed_request(&key_a, "fleet-a", 100, None, ActionClass::Observe);
        log.verify_and_record(&a_high, &policy, &target(), 1_500)
            .expect("fleet-a accepted at sequence 100");

        // fleet-b's own sequence 1 must not be treated as stale relative to fleet-a's 100.
        let b_low = signed_request(&key_b, "fleet-b", 1, None, ActionClass::Observe);
        assert!(
            log.verify_and_record(&b_low, &policy, &target(), 1_500)
                .is_ok()
        );
    }

    #[test]
    fn malformed_json_is_rejected_by_parse_request_without_panicking() {
        assert!(parse_request("{ not json").is_err());
        assert!(parse_request("{}").is_err());
    }

    #[test]
    fn an_unrecognized_field_is_rejected_before_verification_is_ever_reached() {
        let key = keypair();
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Observe);
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
        // Proves AC1's "no hidden second authority path": once extracted,
        // minimum_authority_for(a remote-originated requested_action) is fed into
        // AuthorityInputs exactly like a local AuthorityLevel value, and effective_authority
        // cannot tell the difference.
        let key = keypair();
        let policy = policy_with("fleet-1", AuthorityLevel::Quarantine, &key);
        let request = signed_request(&key, "fleet-1", 1, None, ActionClass::Quarantine);
        let verified =
            verify_remote_execution_request(&request, &policy, &target(), 1_500).expect("verify");

        let shared_inputs = |user_requested: AuthorityLevel| AuthorityInputs {
            user_requested,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: KnowledgeConfidence::Verified,
            activity: ActivityState::Idle,
            protection: ProtectionState::Normal,
            integrity: IntegrityState::Healthy,
            provider_trust: TrustedTier::untrusted(),
        };

        let from_remote = effective_authority(shared_inputs(minimum_authority_for(
            verified.requested_action,
        )));
        let from_local = effective_authority(shared_inputs(AuthorityLevel::Quarantine));
        assert_eq!(from_remote, from_local);
    }

    #[test]
    fn local_authority_is_unaffected_by_an_unconfigured_commercial_service() {
        // E18-S03 (docs/PRODUCT.md "Open-source and commercial boundary"), round 1 independent
        // verifier review's required repair: the prior version of this test constructed an empty
        // `TrustedRemoteControllers` and never passed it anywhere, which proved only that
        // `effective_authority`'s existing signature has no such parameter - true before this
        // story and unrelated to what AC1/AC3 actually require. This version exercises real data
        // flow: an empty policy (a fresh, single-machine install that has never heard of a
        // controller) must refuse a remote request cleanly through the real verification path
        // (AC2 - absence of commercial configuration never silently grants or corrupts
        // anything), and that refusal must have no bearing at all on a local authority decision
        // computed alongside it (AC1/AC3 - local functionality is not reduced or refused).
        let empty_policy = TrustedRemoteControllers::new(vec![]);
        let key = keypair();
        let unconfigured_remote_request =
            signed_request(&key, "any-controller", 1, None, ActionClass::Quarantine);
        let mut log = RemoteExecutionLog::empty();

        assert_eq!(
            log.verify_and_record(
                &unconfigured_remote_request,
                &empty_policy,
                &target(),
                1_500
            ),
            Err(RemoteExecutionError::UnknownController),
            "a remote request must be refused cleanly, never accepted or panicked on, when no \
             commercial/fleet controller is configured"
        );

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
        // local_inputs alone, independent of - and unreduced by - the refusal just above.
        assert_eq!(
            effective_authority(local_inputs).level,
            AuthorityLevel::Observe,
            "an unconfigured commercial service must never reduce or refuse local functionality"
        );
    }
}
