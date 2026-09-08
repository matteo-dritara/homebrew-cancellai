//! Signed provider knowledge bundles (E16-S02, SI-022, SI-029, `docs/security/SUPPLY_CHAIN.md`
//! "Knowledge updates", [ADR-0024](../../../../docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md)).
//!
//! A [`KnowledgeBundle`] packages provider/version/layout intelligence (for example, a
//! `cancellai-provider-api::ProviderManifest` serialized into [`KnowledgeBundle::payload`])
//! separately from the binary, so it can be updated without a full release. This module is
//! deliberately *not* what interprets `payload` - it only answers "did a locally-trusted
//! publisher really sign exactly this content, is it still current, and is it newer than what
//! is already installed?". What the payload means is entirely the calling crate's concern
//! (`cancellai-provider-api::parse_manifest`, for a manifest payload) - the same separation
//! `manifest.rs`'s own module doc draws between "this shape cannot express a capability/trust/
//! authority claim" (enforced structurally, by the payload's own type) and "trust/authority is
//! computed the same way regardless of where the data came from" (enforced here, by never
//! reading a tier, capability, or authority claim out of the bundle itself).
//!
//! **AC1 ("unsigned/invalid bundles are ignored with diagnostics") is enforced at three
//! independent layers**: `serde(deny_unknown_fields)` plus every field being required means a
//! bundle missing `signature` entirely (the literal "unsigned" case) fails to parse at all, with
//! a diagnostic from `serde_json` naming the missing field; [`parse_bundle`] never returns a
//! `KnowledgeBundle` from malformed JSON; and [`verify_bundle`] rejects a syntactically valid
//! but cryptographically wrong bundle (bad hex, bad signature, wrong content digest) with a
//! specific [`KnowledgeBundleError`] variant rather than a generic failure.
//!
//! **AC2 ("a knowledge update cannot raise authority beyond local trust policy") is enforced by
//! [`VerifiedKnowledgeBundle::tier`] never being derived from bundle content**: [`verify_bundle`]
//! looks up the tier for `bundle.publisher_id` in the caller-supplied [`LocalTrustPolicy`] -
//! itself built only from [`crate::TrustedTier`] values, which (SI-021) can only be
//! [`crate::TrustedTier::untrusted`] or the result of a real, evidenced
//! [`crate::TrustedTier::promote`] call - and returns exactly that tier. There is no field on
//! [`KnowledgeBundle`] a publisher could set to claim a higher tier than local policy already
//! assigned them, mirroring [`crate::manifest`]'s own structural approach in the sibling
//! `cancellai-provider-api` crate (that module cannot depend on this one - `cancellai-safety`
//! sits below the provider layer, per `docs/architecture/TARGET.md`'s dependency direction -
//! so the two independently reach the same shape rather than sharing a type).
//!
//! **AC3 ("rollback to prior trusted bundle is supported") is [`KnowledgeStore::rollback`]**:
//! applying a new bundle over a current one keeps the bundle it replaced as `previous`, and
//! rollback reinstates it as `current` after re-checking expiry against the caller's clock (a
//! bundle that was valid when replaced may have since expired; rollback must not silently
//! reinstall an expired bundle - SI-029 "a bad update cannot brick basic offline inspection"
//! is about *not losing access to the last good state*, not about reviving a stale one).
//!
//! Anti-replay (SI-029 "replayed... knowledge updates are rejected"): [`KnowledgeStore::apply`]
//! refuses a bundle from the *same* publisher whose `sequence` does not strictly exceed the
//! currently installed bundle's sequence from that publisher - an attacker who captures an
//! old, validly-signed bundle cannot silently reintroduce it as if it were a fresh update.
//! This check is scoped per-publisher (a first bundle from a *different*, newly trusted
//! publisher is never "stale" relative to an unrelated publisher's sequence numbering).

use std::fmt;

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::trust_promotion::TrustedTier;

/// The schema version range this build accepts. A single build accepts exactly one version
/// today (mirrors [`crate::manifest`]-adjacent `CURRENT_SCHEMA_VERSION`-style constants
/// elsewhere in this workspace); the range shape leaves room for a future story to accept more
/// than one version during a migration without changing [`verify_bundle`]'s signature.
pub const SUPPORTED_SCHEMA_VERSIONS: (u32, u32) = (1, 1);

/// A signed provider knowledge bundle, as received (not yet verified). See the module doc for
/// what this type deliberately cannot express.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeBundle {
    pub schema_version: u32,
    /// Identifies which [`TrustedPublisher`] in the caller's [`LocalTrustPolicy`] to verify
    /// against. Carries no trust claim of its own - it is a lookup key, not an assertion.
    pub publisher_id: String,
    /// Strictly increasing per publisher. [`KnowledgeStore::apply`] uses this to refuse a
    /// replayed or out-of-order update from the same publisher.
    pub sequence: u64,
    /// Unix seconds.
    pub issued_at: u64,
    /// Unix seconds; `None` means the bundle never expires on its own (still subject to
    /// [`KnowledgeStore::apply`]'s sequence check when superseded).
    pub expires_at: Option<u64>,
    /// Lowercase hex SHA-256 of `payload`, computed by the publisher before signing. Verified
    /// independently of the signature (`verify_bundle` checks both) so a bug or attack that
    /// targeted only one of the two checks is still caught by the other.
    pub content_digest: String,
    /// Opaque to this module - see the module doc. Typically JSON text a calling crate parses
    /// with its own, separately-versioned schema (for example a `ProviderManifest`).
    pub payload: String,
    /// Lowercase hex Ed25519 signature (64 bytes) over [`KnowledgeBundle::signing_bytes`].
    pub signature: String,
}

impl KnowledgeBundle {
    /// The exact bytes [`TrustedPublisher::public_key`] must have signed. Length-prefixes every
    /// field (big-endian `u64` length, then the field's own bytes) rather than joining them with
    /// a delimiter - a delimiter inside `publisher_id` or `payload` could otherwise let two
    /// different logical bundles serialize to the same byte string, letting a signature meant
    /// for one be replayed as if it covered the other. `signature` itself is never included -
    /// it is what is being computed against this output, not part of it.
    fn signing_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut push_field = |field: &[u8]| {
            buf.extend_from_slice(&(field.len() as u64).to_be_bytes());
            buf.extend_from_slice(field);
        };
        push_field(&self.schema_version.to_be_bytes());
        push_field(self.publisher_id.as_bytes());
        push_field(&self.sequence.to_be_bytes());
        push_field(&self.issued_at.to_be_bytes());
        push_field(&self.expires_at.unwrap_or(0).to_be_bytes());
        push_field(&[u8::from(self.expires_at.is_some())]);
        push_field(self.content_digest.as_bytes());
        push_field(self.payload.as_bytes());
        buf
    }
}

/// Parses `text` as a [`KnowledgeBundle`]. The sole entry point from untrusted bytes - a bundle
/// missing any required field (including a wholly unsigned bundle, which is missing
/// `signature`) or carrying an unrecognized field fails here, before [`verify_bundle`] is ever
/// reached, with a diagnostic naming what was wrong.
pub fn parse_bundle(text: &str) -> Result<KnowledgeBundle, String> {
    serde_json::from_str(text).map_err(|error| format!("invalid knowledge bundle: {error}"))
}

/// One publisher this installation trusts, and the [`TrustedTier`] their bundles are allowed to
/// carry - assigned by local policy (a human decision, recorded the same way any other
/// [`TrustedTier::promote`] call is), never read from a bundle itself.
#[derive(Debug, Clone)]
pub struct TrustedPublisher {
    pub publisher_id: String,
    pub public_key: [u8; 32],
    pub tier: TrustedTier,
}

/// The set of publishers this installation trusts. Not `pub` fields - constructed once and
/// looked up by publisher id, so a caller cannot mutate it into containing a
/// publisher/tier pairing the local operator never actually chose.
#[derive(Debug, Clone, Default)]
pub struct LocalTrustPolicy {
    publishers: Vec<TrustedPublisher>,
}

impl LocalTrustPolicy {
    pub fn new(publishers: Vec<TrustedPublisher>) -> Self {
        Self { publishers }
    }

    fn find(&self, publisher_id: &str) -> Option<&TrustedPublisher> {
        self.publishers
            .iter()
            .find(|p| p.publisher_id == publisher_id)
    }
}

/// Why [`verify_bundle`] refused a bundle. Every variant is fail-closed: no partial trust is
/// ever granted on the way to one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnowledgeBundleError {
    UnsupportedSchemaVersion,
    /// `publisher_id` names no [`TrustedPublisher`] in the [`LocalTrustPolicy`] consulted.
    UnknownSigner,
    /// `signature` was not exactly 64 bytes of valid hex.
    MalformedSignature,
    /// SHA-256 of `payload` does not match `content_digest`.
    ContentDigestMismatch,
    /// The Ed25519 signature does not verify against the publisher's public key.
    InvalidSignature,
    /// `expires_at` is at or before the verification clock.
    Expired,
    /// [`KnowledgeStore::apply`] only: `sequence` does not strictly exceed the currently
    /// installed bundle's sequence from the same publisher.
    Stale,
    /// [`KnowledgeStore::rollback`] only: there is no prior bundle to roll back to.
    NoPriorBundle,
}

impl fmt::Display for KnowledgeBundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::UnsupportedSchemaVersion => "unsupported knowledge bundle schema version",
            Self::UnknownSigner => "unknown knowledge bundle publisher",
            Self::MalformedSignature => "malformed knowledge bundle signature",
            Self::ContentDigestMismatch => "knowledge bundle content digest mismatch",
            Self::InvalidSignature => "invalid knowledge bundle signature",
            Self::Expired => "knowledge bundle has expired",
            Self::Stale => "knowledge bundle is not newer than the installed one",
            Self::NoPriorBundle => "no prior knowledge bundle to roll back to",
        };
        f.write_str(text)
    }
}

/// A [`KnowledgeBundle`] that has passed every check in [`verify_bundle`]. `tier` is the
/// [`LocalTrustPolicy`]'s own assignment for `publisher_id`, never anything read from the
/// bundle - see the module doc's AC2 note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedKnowledgeBundle {
    pub publisher_id: String,
    pub tier: TrustedTier,
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: Option<u64>,
    pub payload: String,
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
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

/// Verifies `bundle` against `policy` at `now_unix` (Unix seconds - threaded in explicitly
/// rather than read from the system clock, so tests can exercise expiry deterministically).
/// Never returns a [`VerifiedKnowledgeBundle`] for a bundle that failed any check; never raises
/// `tier` above what `policy` already assigned `bundle.publisher_id`.
pub fn verify_bundle(
    bundle: &KnowledgeBundle,
    policy: &LocalTrustPolicy,
    now_unix: u64,
) -> Result<VerifiedKnowledgeBundle, KnowledgeBundleError> {
    if bundle.schema_version < SUPPORTED_SCHEMA_VERSIONS.0
        || bundle.schema_version > SUPPORTED_SCHEMA_VERSIONS.1
    {
        return Err(KnowledgeBundleError::UnsupportedSchemaVersion);
    }

    let publisher = policy
        .find(&bundle.publisher_id)
        .ok_or(KnowledgeBundleError::UnknownSigner)?;

    let expected_digest = encode_hex(&Sha256::digest(bundle.payload.as_bytes()));
    if expected_digest != bundle.content_digest {
        return Err(KnowledgeBundleError::ContentDigestMismatch);
    }

    let signature_bytes = decode_hex(&bundle.signature)
        .and_then(|bytes| <[u8; 64]>::try_from(bytes).ok())
        .ok_or(KnowledgeBundleError::MalformedSignature)?;
    let signature = Signature::from_bytes(&signature_bytes);
    let verifying_key = VerifyingKey::from_bytes(&publisher.public_key)
        .map_err(|_| KnowledgeBundleError::UnknownSigner)?;
    verifying_key
        .verify(&bundle.signing_bytes(), &signature)
        .map_err(|_| KnowledgeBundleError::InvalidSignature)?;

    if let Some(expires_at) = bundle.expires_at
        && now_unix >= expires_at
    {
        return Err(KnowledgeBundleError::Expired);
    }

    Ok(VerifiedKnowledgeBundle {
        publisher_id: bundle.publisher_id.clone(),
        tier: publisher.tier,
        sequence: bundle.sequence,
        issued_at: bundle.issued_at,
        expires_at: bundle.expires_at,
        payload: bundle.payload.clone(),
    })
}

/// Holds at most a current and a previous verified bundle. `apply`/`rollback` are the only ways
/// to change either - there is no field mutation available to a caller outside this crate.
#[derive(Debug, Clone, Default)]
pub struct KnowledgeStore {
    current: Option<VerifiedKnowledgeBundle>,
    previous: Option<VerifiedKnowledgeBundle>,
}

impl KnowledgeStore {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Option<&VerifiedKnowledgeBundle> {
        self.current.as_ref()
    }

    /// Verifies `bundle`, then installs it as current if it is not stale relative to the
    /// currently installed bundle from the *same* publisher. The bundle it replaces (if any)
    /// becomes `previous`, available to [`KnowledgeStore::rollback`].
    pub fn apply(
        &mut self,
        bundle: &KnowledgeBundle,
        policy: &LocalTrustPolicy,
        now_unix: u64,
    ) -> Result<&VerifiedKnowledgeBundle, KnowledgeBundleError> {
        let verified = verify_bundle(bundle, policy, now_unix)?;
        if let Some(current) = &self.current
            && current.publisher_id == verified.publisher_id
            && verified.sequence <= current.sequence
        {
            return Err(KnowledgeBundleError::Stale);
        }
        self.previous = self.current.take();
        self.current = Some(verified);
        Ok(self.current.as_ref().expect("just inserted"))
    }

    /// Reinstates `previous` as `current`, after re-checking it has not since expired. Leaves
    /// the store unchanged (returns [`KnowledgeBundleError::NoPriorBundle`] /
    /// [`KnowledgeBundleError::Expired`]) rather than clearing `current` on failure - a failed
    /// rollback must not itself brick offline inspection (SI-029).
    pub fn rollback(
        &mut self,
        now_unix: u64,
    ) -> Result<&VerifiedKnowledgeBundle, KnowledgeBundleError> {
        let candidate = self
            .previous
            .as_ref()
            .ok_or(KnowledgeBundleError::NoPriorBundle)?;
        if let Some(expires_at) = candidate.expires_at
            && now_unix >= expires_at
        {
            return Err(KnowledgeBundleError::Expired);
        }
        self.current = self.previous.take();
        Ok(self.current.as_ref().expect("just reinstated"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_model::ProviderTrust;
    use ed25519_dalek::{Signer, SigningKey};

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn policy_for(seed: u8, publisher_id: &str, tier: TrustedTier) -> LocalTrustPolicy {
        let key = signing_key(seed);
        LocalTrustPolicy::new(vec![TrustedPublisher {
            publisher_id: publisher_id.to_string(),
            public_key: key.verifying_key().to_bytes(),
            tier,
        }])
    }

    fn signed_bundle(
        seed: u8,
        publisher_id: &str,
        sequence: u64,
        issued_at: u64,
        expires_at: Option<u64>,
        payload: &str,
    ) -> KnowledgeBundle {
        let content_digest = encode_hex(&Sha256::digest(payload.as_bytes()));
        let mut bundle = KnowledgeBundle {
            schema_version: 1,
            publisher_id: publisher_id.to_string(),
            sequence,
            issued_at,
            expires_at,
            content_digest,
            payload: payload.to_string(),
            signature: String::new(),
        };
        let key = signing_key(seed);
        let signature = key.sign(&bundle.signing_bytes());
        bundle.signature = encode_hex(&signature.to_bytes());
        bundle
    }

    fn promoted_tier() -> TrustedTier {
        TrustedTier::for_tests(ProviderTrust::CommunityVerified)
    }

    #[test]
    fn ac1_a_bundle_missing_the_signature_field_entirely_fails_to_parse() {
        let text = r#"{
            "schema_version": 1, "publisher_id": "acme", "sequence": 1,
            "issued_at": 0, "expires_at": null,
            "content_digest": "deadbeef", "payload": "{}"
        }"#;
        let error = parse_bundle(text).expect_err("unsigned bundle must not parse");
        assert!(error.contains("signature"), "{error}");
    }

    #[test]
    fn ac1_an_unparseable_bundle_never_reaches_verification() {
        assert!(parse_bundle("not json").is_err());
    }

    #[test]
    fn a_correctly_signed_bundle_verifies() {
        let policy = policy_for(1, "acme", promoted_tier());
        let bundle = signed_bundle(1, "acme", 1, 1_000, None, "{\"root\":\"data\"}");
        let verified = verify_bundle(&bundle, &policy, 1_001).expect("must verify");
        assert_eq!(verified.publisher_id, "acme");
        assert_eq!(verified.payload, "{\"root\":\"data\"}");
    }

    #[test]
    fn tamper_a_payload_changed_after_signing_fails_content_digest_and_signature_checks() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut bundle = signed_bundle(1, "acme", 1, 1_000, None, "original");
        bundle.payload = "tampered".to_string();
        let error = verify_bundle(&bundle, &policy, 1_001).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::ContentDigestMismatch);
    }

    #[test]
    fn tamper_a_digest_updated_to_match_tampered_payload_still_fails_signature_verification() {
        // Simulates an attacker who edits both payload and content_digest consistently, but
        // cannot produce a valid signature over the new signing_bytes without the private key.
        let policy = policy_for(1, "acme", promoted_tier());
        let mut bundle = signed_bundle(1, "acme", 1, 1_000, None, "original");
        bundle.payload = "tampered".to_string();
        bundle.content_digest = encode_hex(&Sha256::digest(bundle.payload.as_bytes()));
        let error = verify_bundle(&bundle, &policy, 1_001).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::InvalidSignature);
    }

    #[test]
    fn tamper_a_bit_flipped_signature_is_rejected() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut bundle = signed_bundle(1, "acme", 1, 1_000, None, "payload");
        let mut sig_bytes = decode_hex(&bundle.signature).unwrap();
        sig_bytes[0] ^= 0xFF;
        bundle.signature = encode_hex(&sig_bytes);
        let error = verify_bundle(&bundle, &policy, 1_001).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::InvalidSignature);
    }

    #[test]
    fn unknown_signer_a_publisher_id_absent_from_local_trust_policy_is_rejected() {
        let policy = policy_for(1, "acme", promoted_tier());
        let bundle = signed_bundle(2, "not-acme", 1, 1_000, None, "payload");
        let error = verify_bundle(&bundle, &policy, 1_001).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::UnknownSigner);
    }

    #[test]
    fn unknown_signer_a_correct_publisher_id_with_a_different_signing_key_is_rejected() {
        // publisher_id matches, but the bundle was actually signed by a different key than the
        // one local policy trusts for that id - must not verify against the claimed identity
        // alone.
        let policy = policy_for(1, "acme", promoted_tier());
        let bundle = signed_bundle(99, "acme", 1, 1_000, None, "payload");
        let error = verify_bundle(&bundle, &policy, 1_001).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::InvalidSignature);
    }

    #[test]
    fn ac2_tier_comes_from_local_policy_never_from_the_bundle() {
        let policy = policy_for(1, "acme", promoted_tier());
        let bundle = signed_bundle(1, "acme", 1, 1_000, None, "payload");
        let verified = verify_bundle(&bundle, &policy, 1_001).unwrap();
        assert_eq!(verified.tier.level(), ProviderTrust::CommunityVerified);
        // KnowledgeBundle has no field that could have asserted this - see its own definition.
    }

    #[test]
    fn expiry_a_bundle_past_its_expires_at_is_rejected() {
        let policy = policy_for(1, "acme", promoted_tier());
        let bundle = signed_bundle(1, "acme", 1, 1_000, Some(2_000), "payload");
        let error = verify_bundle(&bundle, &policy, 2_000).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::Expired);
    }

    #[test]
    fn expiry_a_bundle_before_its_expires_at_still_verifies() {
        let policy = policy_for(1, "acme", promoted_tier());
        let bundle = signed_bundle(1, "acme", 1, 1_000, Some(2_000), "payload");
        assert!(verify_bundle(&bundle, &policy, 1_999).is_ok());
    }

    #[test]
    fn unsupported_schema_version_is_rejected_before_any_crypto_check() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut bundle = signed_bundle(1, "acme", 1, 1_000, None, "payload");
        bundle.schema_version = 2;
        let error = verify_bundle(&bundle, &policy, 1_001).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::UnsupportedSchemaVersion);
    }

    #[test]
    fn rollback_ac3_a_replaced_bundle_can_be_restored() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut store = KnowledgeStore::empty();
        let first = signed_bundle(1, "acme", 1, 1_000, None, "first");
        let second = signed_bundle(1, "acme", 2, 2_000, None, "second");
        store.apply(&first, &policy, 1_001).unwrap();
        store.apply(&second, &policy, 2_001).unwrap();
        assert_eq!(store.current().unwrap().payload, "second");
        store.rollback(2_002).unwrap();
        assert_eq!(store.current().unwrap().payload, "first");
    }

    #[test]
    fn rollback_fails_closed_with_no_prior_bundle() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut store = KnowledgeStore::empty();
        let only = signed_bundle(1, "acme", 1, 1_000, None, "only");
        store.apply(&only, &policy, 1_001).unwrap();
        let error = store.rollback(1_002).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::NoPriorBundle);
        // Failed rollback must not disturb the current bundle (SI-029).
        assert_eq!(store.current().unwrap().payload, "only");
    }

    #[test]
    fn rollback_refuses_to_reinstate_a_bundle_that_has_since_expired() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut store = KnowledgeStore::empty();
        let first = signed_bundle(1, "acme", 1, 1_000, Some(1_500), "first");
        let second = signed_bundle(1, "acme", 2, 2_000, None, "second");
        store.apply(&first, &policy, 1_001).unwrap();
        store.apply(&second, &policy, 2_001).unwrap();
        let error = store.rollback(1_600).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::Expired);
        // Current stays "second" - a failed rollback never leaves the store empty or reverted.
        assert_eq!(store.current().unwrap().payload, "second");
    }

    #[test]
    fn replay_a_stale_sequence_from_the_same_publisher_is_rejected() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut store = KnowledgeStore::empty();
        let newer = signed_bundle(1, "acme", 5, 5_000, None, "newer");
        let older = signed_bundle(1, "acme", 3, 3_000, None, "older");
        store.apply(&newer, &policy, 5_001).unwrap();
        let error = store.apply(&older, &policy, 5_002).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::Stale);
        assert_eq!(store.current().unwrap().payload, "newer");
    }

    #[test]
    fn replay_an_equal_sequence_from_the_same_publisher_is_also_rejected() {
        let policy = policy_for(1, "acme", promoted_tier());
        let mut store = KnowledgeStore::empty();
        let first = signed_bundle(1, "acme", 5, 5_000, None, "first");
        let replay = signed_bundle(1, "acme", 5, 5_000, None, "first");
        store.apply(&first, &policy, 5_001).unwrap();
        let error = store.apply(&replay, &policy, 5_002).unwrap_err();
        assert_eq!(error, KnowledgeBundleError::Stale);
    }

    #[test]
    fn a_first_bundle_from_a_different_publisher_is_never_stale() {
        let key_a = signing_key(1);
        let key_b = signing_key(2);
        let policy = LocalTrustPolicy::new(vec![
            TrustedPublisher {
                publisher_id: "acme".to_string(),
                public_key: key_a.verifying_key().to_bytes(),
                tier: promoted_tier(),
            },
            TrustedPublisher {
                publisher_id: "other".to_string(),
                public_key: key_b.verifying_key().to_bytes(),
                tier: TrustedTier::untrusted(),
            },
        ]);
        let mut store = KnowledgeStore::empty();
        let acme_bundle = signed_bundle(1, "acme", 100, 5_000, None, "acme-payload");
        store.apply(&acme_bundle, &policy, 5_001).unwrap();
        let other_bundle = signed_bundle(2, "other", 1, 1_000, None, "other-payload");
        store.apply(&other_bundle, &policy, 5_002).unwrap();
        assert_eq!(store.current().unwrap().publisher_id, "other");
    }
}
