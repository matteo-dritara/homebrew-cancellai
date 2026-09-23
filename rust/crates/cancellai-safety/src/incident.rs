//! Safety incident containment by signed capability downgrade (E17-S07, SI-022, SI-029, SI-030,
//! `docs/security/INCIDENT_RESPONSE.md` "Kill-switch hierarchy", `docs/security/SUPPLY_CHAIN.md`
//! "Incident containment").
//!
//! A [`ContainmentNotice`] travels as the payload of an ordinary signed
//! [`crate::KnowledgeBundle`]. It names a provider (optionally narrowed to versions, action classes
//! and platforms) and a [`ContainmentCeiling`], and that is all it can say. The ceiling vocabulary
//! has exactly two members, `observe` and `recommend`, so a notice is structurally incapable of
//! expressing any level at which something could be mutated: the most a verified notice can do is
//! take authority away. There is no field for a command, a path, a tier, a restore or a lift.
//!
//! **The ledger is monotonic.** [`ContainmentLedger`] records every verified containment and only
//! ever grows from network input. A later bundle that omits an incident does not lift it, a
//! [`crate::KnowledgeStore::rollback`] to an older bundle does not lift it, and a bundle's expiry
//! does not lift it - each of those would turn a containment into something an attacker can undo by
//! replaying, withholding or waiting. Lifting is [`ContainmentLedger::lift_locally`], a call local
//! code makes after an owner-visible closure decision; nothing in the wire format reaches it
//! ("Central/federated systems can only reduce capability or inform a local node").
//!
//! **Offline is the installed kernel.** With no notice ingested the ledger contributes no
//! constraint at all, and [`ContainmentLedger::refresh`] treats an unreachable knowledge service,
//! an unparseable bundle, a bad signature, an expired bundle and a replay identically: the ledger is
//! left exactly as it was and authority keeps being computed from the local constraints. A failed
//! update never widens authority and never bricks inspection.
//!
//! **Evidence carries identifiers, not content.** [`IncidentEvidence`] records the knowledge
//! provenance (publisher, sequence, issue time, content digest), the installed release (version and
//! build channel), and the affected provider, versions, action classes, platforms and Safety
//! Invariants. Every one of those is validated against a bounded identifier alphabet on the way in,
//! so a notice cannot smuggle provider payload content - a transcript line, a path, a prompt - into
//! an evidence record under an innocuous field name.
//!
//! Nothing in the workspace consumes [`crate::effective_authority_under_containment`] on a live
//! mutation path yet, for the same reason `effective_authority_for_channel` has no caller: the
//! Rust CLI is still a beta artifact, and wiring its classification pipeline to real release and
//! knowledge state is E06-S04's cutover work.

use std::collections::BTreeMap;
use std::fmt;

use cancellai_model::{ActionClass, AuthorityLevel};
use sha2::{Digest, Sha256};

use crate::build_channel::BuildChannel;
use crate::knowledge_bundle::{
    KnowledgeBundle, KnowledgeBundleError, LocalTrustPolicy, parse_bundle, verify_bundle,
};

/// The payload schema version this build accepts inside a knowledge bundle.
pub const CONTAINMENT_SCHEMA_VERSION: u32 = 1;

/// The payload `kind` that marks a knowledge bundle as a containment notice rather than, say, a
/// provider manifest.
pub const CONTAINMENT_KIND: &str = "capability_containment";

const MAX_IDENTIFIER_LEN: usize = 64;
const MAX_LIST_LEN: usize = 32;
const MAX_ENTRIES: usize = 64;

/// The only ceilings a containment can impose. Both sit below
/// `minimum_authority_for(ActionClass::Quarantine)`, so no containment can leave anything able to
/// mutate - and there is deliberately no third member.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainmentCeiling {
    Observe,
    Recommend,
}

impl ContainmentCeiling {
    pub fn level(self) -> AuthorityLevel {
        match self {
            Self::Observe => AuthorityLevel::Observe,
            Self::Recommend => AuthorityLevel::Recommend,
        }
    }
}

/// `docs/security/INCIDENT_RESPONSE.md`'s incident classes that call for containment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IncidentSeverity {
    S0,
    S1,
}

/// The tier-1 platforms a containment can be scoped to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum IncidentPlatform {
    Macos,
    Linux,
    Windows,
}

/// One containment, as it appears inside a notice.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainmentEntry {
    pub incident_id: String,
    pub severity: IncidentSeverity,
    pub provider_id: String,
    /// `None` contains every version of the provider. An empty list is refused rather than read
    /// as either "all" or "none".
    pub provider_versions: Option<Vec<String>>,
    /// `None` contains every action class.
    pub action_classes: Option<Vec<ActionClass>>,
    /// `None` contains every platform.
    pub platforms: Option<Vec<IncidentPlatform>>,
    pub ceiling: ContainmentCeiling,
    /// Safety Invariant identifiers (`SI-NNN`) the incident involves. At least one.
    pub invariants: Vec<String>,
    /// Release versions known to be affected, if any.
    pub affected_releases: Vec<String>,
}

/// The payload of a containment knowledge bundle.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainmentNotice {
    pub schema_version: u32,
    pub kind: String,
    pub entries: Vec<ContainmentEntry>,
}

/// Why a containment update was not applied. Every variant leaves the ledger unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainmentError {
    /// The bundle text did not parse as a knowledge bundle.
    MalformedBundle,
    /// The bundle failed signature, digest, publisher, schema or expiry verification.
    Bundle(KnowledgeBundleError),
    /// The bundle's sequence does not exceed the last containment ingested from that publisher.
    Replayed,
    /// The verified payload is not a well-formed containment notice.
    MalformedNotice(String),
}

impl fmt::Display for ContainmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedBundle => f.write_str("containment update is not a knowledge bundle"),
            Self::Bundle(error) => write!(f, "containment update refused: {error}"),
            Self::Replayed => {
                f.write_str("containment update is not newer than the last one from this publisher")
            }
            Self::MalformedNotice(reason) => write!(f, "malformed containment notice: {reason}"),
        }
    }
}

/// A knowledge service that could not be reached. Carries nothing: the reason is not evidence
/// about anything the ledger needs, and an error body is not something to parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnowledgeUnavailable;

/// What [`ContainmentLedger::refresh`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// The knowledge service was unreachable; the installed kernel and the existing ledger apply.
    Offline,
    /// The update was refused; the ledger is unchanged.
    Refused(ContainmentError),
    /// The update was verified and its containments recorded.
    Applied(Vec<IncidentEvidence>),
}

/// The build a containment was ingested under.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ReleaseProvenance {
    pub version: String,
    pub channel: cancellai_model::ReleaseChannel,
}

impl ReleaseProvenance {
    /// The running build: its compiled version and compiled channel, never values a caller or a
    /// notice supplies.
    pub fn of_this_build(channel: BuildChannel) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            channel: channel.level(),
        }
    }
}

/// Where a containment came from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct KnowledgeProvenance {
    pub publisher_id: String,
    pub sequence: u64,
    pub issued_at: u64,
    pub content_digest: String,
}

/// The incident record for one containment. Identifiers only - see the module doc.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IncidentEvidence {
    pub incident_id: String,
    pub severity: IncidentSeverity,
    pub provider_id: String,
    pub provider_versions: Option<Vec<String>>,
    pub action_classes: Option<Vec<ActionClass>>,
    pub platforms: Option<Vec<IncidentPlatform>>,
    pub ceiling: ContainmentCeiling,
    pub invariants: Vec<String>,
    pub affected_releases: Vec<String>,
    pub knowledge: KnowledgeProvenance,
    pub release: ReleaseProvenance,
}

/// What an authority decision is about, for the purpose of matching containments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContainmentTarget<'a> {
    pub provider_id: &'a str,
    /// `None` when the installed provider version is not known. An unknown version is treated as
    /// affected by a version-scoped containment: not knowing is not evidence of being safe.
    pub provider_version: Option<&'a str>,
    pub action_class: ActionClass,
    pub platform: IncidentPlatform,
}

/// The ceiling containment imposes on one target, and which incidents imposed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainmentBinding {
    pub ceiling: AuthorityLevel,
    pub incidents: Vec<String>,
}

/// Every containment this installation has verified and not locally lifted.
#[derive(Debug, Clone, Default)]
pub struct ContainmentLedger {
    active: BTreeMap<String, IncidentEvidence>,
    last_sequence: BTreeMap<String, u64>,
}

impl ContainmentLedger {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Every active containment, ordered by incident id.
    pub fn active(&self) -> impl Iterator<Item = &IncidentEvidence> {
        self.active.values()
    }

    /// One knowledge-service poll. `fetched` is the raw bundle text, or the fact that the service
    /// could not be reached; either way nothing but a verified, newer notice changes the ledger.
    pub fn refresh(
        &mut self,
        fetched: Result<&str, KnowledgeUnavailable>,
        policy: &LocalTrustPolicy,
        now_unix: u64,
        release: &ReleaseProvenance,
    ) -> RefreshOutcome {
        let Ok(text) = fetched else {
            return RefreshOutcome::Offline;
        };
        let applied = parse_bundle(text)
            .map_err(|_| ContainmentError::MalformedBundle)
            .and_then(|bundle| self.ingest(&bundle, policy, now_unix, release));
        match applied {
            Ok(evidence) => RefreshOutcome::Applied(evidence),
            Err(error) => RefreshOutcome::Refused(error),
        }
    }

    /// Verifies `bundle` against `policy`, then records every containment its payload names.
    /// All-or-nothing: a notice with one malformed entry records none of them.
    pub fn ingest(
        &mut self,
        bundle: &KnowledgeBundle,
        policy: &LocalTrustPolicy,
        now_unix: u64,
        release: &ReleaseProvenance,
    ) -> Result<Vec<IncidentEvidence>, ContainmentError> {
        let verified = verify_bundle(bundle, policy, now_unix).map_err(ContainmentError::Bundle)?;
        if self
            .last_sequence
            .get(&verified.publisher_id)
            .is_some_and(|last| verified.sequence <= *last)
        {
            return Err(ContainmentError::Replayed);
        }
        let notice = parse_notice(&verified.payload)?;
        let knowledge = KnowledgeProvenance {
            publisher_id: verified.publisher_id.clone(),
            sequence: verified.sequence,
            issued_at: verified.issued_at,
            content_digest: hex_digest(&verified.payload),
        };
        let evidence: Vec<IncidentEvidence> = notice
            .entries
            .into_iter()
            .map(|entry| IncidentEvidence {
                incident_id: entry.incident_id,
                severity: entry.severity,
                provider_id: entry.provider_id,
                provider_versions: entry.provider_versions,
                action_classes: entry.action_classes,
                platforms: entry.platforms,
                ceiling: entry.ceiling,
                invariants: entry.invariants,
                affected_releases: entry.affected_releases,
                knowledge: knowledge.clone(),
                release: release.clone(),
            })
            .collect();
        self.last_sequence
            .insert(verified.publisher_id, verified.sequence);
        for record in &evidence {
            // A re-issued incident keeps the stricter of the two ceilings: the network may
            // narrow a containment, never relax one.
            let merged = match self.active.get(&record.incident_id) {
                Some(existing) if existing.ceiling.level() < record.ceiling.level() => {
                    existing.clone()
                }
                _ => record.clone(),
            };
            self.active.insert(record.incident_id.clone(), merged);
        }
        Ok(evidence)
    }

    /// Removes a containment. Local code calls this after the owner-visible closure decision
    /// `docs/security/INCIDENT_RESPONSE.md` requires; no notice field can reach it.
    pub fn lift_locally(&mut self, incident_id: &str) -> Option<IncidentEvidence> {
        self.active.remove(incident_id)
    }

    /// Whether a trust promotion for `provider_id` must wait: while any containment names the
    /// provider, raising its trust tier is refused ("stop promotion").
    pub fn freezes_promotion_of(&self, provider_id: &str) -> bool {
        self.active
            .values()
            .any(|record| record.provider_id == provider_id)
    }

    /// The strictest ceiling any active containment imposes on `target`, or `None` when no
    /// containment applies.
    pub fn binding_for(&self, target: &ContainmentTarget<'_>) -> Option<ContainmentBinding> {
        let matching: Vec<&IncidentEvidence> = self
            .active
            .values()
            .filter(|record| applies_to(record, target))
            .collect();
        let ceiling = matching.iter().map(|record| record.ceiling.level()).min()?;
        Some(ContainmentBinding {
            ceiling,
            incidents: matching
                .iter()
                .filter(|record| record.ceiling.level() == ceiling)
                .map(|record| record.incident_id.clone())
                .collect(),
        })
    }
}

fn applies_to(record: &IncidentEvidence, target: &ContainmentTarget<'_>) -> bool {
    if record.provider_id != target.provider_id {
        return false;
    }
    let version_matches = match (&record.provider_versions, target.provider_version) {
        (None, _) | (Some(_), None) => true,
        (Some(versions), Some(version)) => versions.iter().any(|v| v == version),
    };
    let action_matches = record
        .action_classes
        .as_ref()
        .is_none_or(|classes| classes.contains(&target.action_class));
    let platform_matches = record
        .platforms
        .as_ref()
        .is_none_or(|platforms| platforms.contains(&target.platform));
    version_matches && action_matches && platform_matches
}

fn hex_digest(payload: &str) -> String {
    Sha256::digest(payload.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Parses and validates a containment notice from a verified payload.
pub fn parse_notice(payload: &str) -> Result<ContainmentNotice, ContainmentError> {
    let notice: ContainmentNotice = serde_json::from_str(payload)
        .map_err(|error| ContainmentError::MalformedNotice(error.to_string()))?;
    validate_notice(&notice).map_err(ContainmentError::MalformedNotice)?;
    Ok(notice)
}

fn validate_notice(notice: &ContainmentNotice) -> Result<(), String> {
    if notice.schema_version != CONTAINMENT_SCHEMA_VERSION {
        return Err(format!(
            "unsupported schema_version {}",
            notice.schema_version
        ));
    }
    if notice.kind != CONTAINMENT_KIND {
        return Err("kind is not a capability containment".to_string());
    }
    if notice.entries.is_empty() || notice.entries.len() > MAX_ENTRIES {
        return Err(format!("entries must number 1..={MAX_ENTRIES}"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for entry in &notice.entries {
        validate_entry(entry)?;
        if !seen.insert(entry.incident_id.as_str()) {
            return Err(format!("incident {} appears twice", entry.incident_id));
        }
    }
    Ok(())
}

fn validate_entry(entry: &ContainmentEntry) -> Result<(), String> {
    identifier(&entry.incident_id, "incident_id")?;
    identifier(&entry.provider_id, "provider_id")?;
    if let Some(versions) = &entry.provider_versions {
        bounded_list(versions.len(), "provider_versions")?;
        for version in versions {
            identifier(version, "provider_versions")?;
        }
    }
    if let Some(classes) = &entry.action_classes {
        bounded_list(classes.len(), "action_classes")?;
    }
    if let Some(platforms) = &entry.platforms {
        bounded_list(platforms.len(), "platforms")?;
    }
    bounded_list(entry.invariants.len(), "invariants")?;
    for invariant in &entry.invariants {
        invariant_id(invariant)?;
    }
    if entry.affected_releases.len() > MAX_LIST_LEN {
        return Err(format!("affected_releases exceeds {MAX_LIST_LEN} entries"));
    }
    for release in &entry.affected_releases {
        identifier(release, "affected_releases")?;
    }
    Ok(())
}

fn bounded_list(len: usize, field: &str) -> Result<(), String> {
    if len == 0 || len > MAX_LIST_LEN {
        return Err(format!("{field} must hold 1..={MAX_LIST_LEN} entries"));
    }
    Ok(())
}

/// An identifier is 1-64 characters of `[A-Za-z0-9._+-]`, starting with an alphanumeric. No
/// whitespace, separators or path characters: nothing that could carry content.
fn identifier(value: &str, field: &str) -> Result<(), String> {
    let well_formed = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_LEN
        && value
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'));
    if well_formed {
        Ok(())
    } else {
        Err(format!("{field} is not a bounded identifier"))
    }
}

fn invariant_id(value: &str) -> Result<(), String> {
    let well_formed = value.len() == 6
        && value.starts_with("SI-")
        && value.bytes().skip(3).all(|b| b.is_ascii_digit());
    if well_formed {
        Ok(())
    } else {
        Err("invariants must be SI-NNN identifiers".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_bundle::{KnowledgeStore, TrustedPublisher};
    use crate::trust_promotion::TrustedTier;
    use cancellai_model::{ProviderTrust, ReleaseChannel};
    use ed25519_dalek::{Signer, SigningKey};

    const NOW: u64 = 10_000;

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn policy(seed: u8, publisher: &str) -> LocalTrustPolicy {
        LocalTrustPolicy::new(vec![TrustedPublisher {
            publisher_id: publisher.to_string(),
            public_key: key(seed).verifying_key().to_bytes(),
            tier: TrustedTier::for_tests(ProviderTrust::BuiltinVerified),
        }])
    }

    fn release() -> ReleaseProvenance {
        ReleaseProvenance {
            version: "1.18.0".to_string(),
            channel: ReleaseChannel::Stable,
        }
    }

    fn signing_bytes(bundle: &KnowledgeBundle) -> Vec<u8> {
        // Mirrors `KnowledgeBundle::signing_bytes`, which is private to its module; a mismatch
        // would surface as every test's bundle failing verification, not as a silent pass.
        let mut buf = Vec::new();
        let mut push = |field: &[u8]| {
            buf.extend_from_slice(&(field.len() as u64).to_be_bytes());
            buf.extend_from_slice(field);
        };
        push(&bundle.schema_version.to_be_bytes());
        push(bundle.publisher_id.as_bytes());
        push(&bundle.sequence.to_be_bytes());
        push(&bundle.issued_at.to_be_bytes());
        push(&bundle.expires_at.unwrap_or(0).to_be_bytes());
        push(&[u8::from(bundle.expires_at.is_some())]);
        push(bundle.content_digest.as_bytes());
        push(bundle.payload.as_bytes());
        buf
    }

    fn bundle(
        seed: u8,
        publisher: &str,
        sequence: u64,
        expires: Option<u64>,
        payload: &str,
    ) -> KnowledgeBundle {
        let mut bundle = KnowledgeBundle {
            schema_version: 1,
            publisher_id: publisher.to_string(),
            sequence,
            issued_at: NOW - 10,
            expires_at: expires,
            content_digest: hex_digest(payload),
            payload: payload.to_string(),
            signature: String::new(),
        };
        let signature = key(seed).sign(&signing_bytes(&bundle));
        bundle.signature = signature
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        bundle
    }

    fn entry(incident: &str, provider: &str, ceiling: &str) -> serde_json::Value {
        serde_json::json!({
            "incident_id": incident,
            "severity": "S0",
            "provider_id": provider,
            "provider_versions": null,
            "action_classes": null,
            "platforms": null,
            "ceiling": ceiling,
            "invariants": ["SI-022"],
            "affected_releases": ["1.17.4"]
        })
    }

    fn notice(entries: Vec<serde_json::Value>) -> String {
        serde_json::json!({
            "schema_version": 1,
            "kind": CONTAINMENT_KIND,
            "entries": entries
        })
        .to_string()
    }

    fn target(provider: &str, class: ActionClass) -> ContainmentTarget<'_> {
        ContainmentTarget {
            provider_id: provider,
            provider_version: Some("2.0.0"),
            action_class: class,
            platform: IncidentPlatform::Linux,
        }
    }

    fn contained(ledger: &mut ContainmentLedger, payload: &str, sequence: u64) {
        ledger
            .ingest(
                &bundle(1, "acme", sequence, None, payload),
                &policy(1, "acme"),
                NOW,
                &release(),
            )
            .expect("valid containment must apply");
    }

    // --- AC1: downgrade through trusted signed knowledge, never an elevation ---------------

    #[test]
    fn a_signed_containment_caps_the_provider_at_its_ceiling() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "recommend")]),
            1,
        );
        let binding = ledger
            .binding_for(&target("codex", ActionClass::Delete))
            .expect("contained");
        assert_eq!(binding.ceiling, AuthorityLevel::Recommend);
        assert_eq!(binding.incidents, vec!["INC-1".to_string()]);
        assert!(
            ledger
                .binding_for(&target("claude", ActionClass::Delete))
                .is_none()
        );
    }

    #[test]
    fn the_ceiling_vocabulary_cannot_express_a_mutating_level() {
        for level in ["quarantine", "govern", "autopilot", "delete", "restore"] {
            let payload = notice(vec![entry("INC-1", "codex", level)]);
            assert!(
                matches!(
                    parse_notice(&payload),
                    Err(ContainmentError::MalformedNotice(_))
                ),
                "{level} must not parse as a ceiling"
            );
        }
        for ceiling in [ContainmentCeiling::Observe, ContainmentCeiling::Recommend] {
            assert!(
                ceiling.level() < crate::authority::minimum_authority_for(ActionClass::Quarantine)
            );
        }
    }

    #[test]
    fn a_notice_cannot_smuggle_an_authority_restore_or_lift_field() {
        for field in ["authority", "tier", "lift", "restore", "command"] {
            let mut value = entry("INC-1", "codex", "observe");
            value[field] = serde_json::json!("autopilot");
            assert!(parse_notice(&notice(vec![value])).is_err(), "{field}");
        }
        let top = serde_json::json!({
            "schema_version": 1, "kind": CONTAINMENT_KIND,
            "entries": [entry("INC-1", "codex", "observe")], "lift": ["INC-0"]
        });
        assert!(parse_notice(&top.to_string()).is_err());
    }

    #[test]
    fn a_reissued_incident_can_narrow_but_never_relax_its_ceiling() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "observe")]),
            1,
        );
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "recommend")]),
            2,
        );
        let binding = ledger.binding_for(&target("codex", ActionClass::Observe));
        assert_eq!(binding.map(|b| b.ceiling), Some(AuthorityLevel::Observe));

        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "recommend")]),
            1,
        );
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "observe")]),
            2,
        );
        let binding = ledger.binding_for(&target("codex", ActionClass::Observe));
        assert_eq!(binding.map(|b| b.ceiling), Some(AuthorityLevel::Observe));
    }

    #[test]
    fn the_strictest_of_several_matching_containments_binds_and_all_ties_are_named() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![
                entry("INC-1", "codex", "recommend"),
                entry("INC-2", "codex", "observe"),
                entry("INC-3", "codex", "observe"),
            ]),
            1,
        );
        let binding = ledger
            .binding_for(&target("codex", ActionClass::Quarantine))
            .expect("contained");
        assert_eq!(binding.ceiling, AuthorityLevel::Observe);
        assert_eq!(
            binding.incidents,
            vec!["INC-2".to_string(), "INC-3".to_string()]
        );
    }

    #[test]
    fn a_containment_freezes_trust_promotion_for_its_provider_only() {
        let mut ledger = ContainmentLedger::empty();
        assert!(!ledger.freezes_promotion_of("codex"));
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "recommend")]),
            1,
        );
        assert!(ledger.freezes_promotion_of("codex"));
        assert!(!ledger.freezes_promotion_of("claude"));
    }

    // --- scoping: version, action class, platform -------------------------------------------

    #[test]
    fn scoping_narrows_to_versions_actions_and_platforms() {
        let mut scoped = entry("INC-1", "codex", "observe");
        scoped["provider_versions"] = serde_json::json!(["2.0.0", "2.0.1"]);
        scoped["action_classes"] = serde_json::json!(["delete"]);
        scoped["platforms"] = serde_json::json!(["linux"]);
        let mut ledger = ContainmentLedger::empty();
        contained(&mut ledger, &notice(vec![scoped]), 1);

        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Delete))
                .is_some()
        );
        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Quarantine))
                .is_none()
        );
        let other_version = ContainmentTarget {
            provider_version: Some("3.0.0"),
            ..target("codex", ActionClass::Delete)
        };
        assert!(ledger.binding_for(&other_version).is_none());
        let other_platform = ContainmentTarget {
            platform: IncidentPlatform::Windows,
            ..target("codex", ActionClass::Delete)
        };
        assert!(ledger.binding_for(&other_platform).is_none());
    }

    #[test]
    fn an_unknown_installed_version_is_treated_as_affected() {
        let mut scoped = entry("INC-1", "codex", "observe");
        scoped["provider_versions"] = serde_json::json!(["2.0.0"]);
        let mut ledger = ContainmentLedger::empty();
        contained(&mut ledger, &notice(vec![scoped]), 1);
        let unknown = ContainmentTarget {
            provider_version: None,
            ..target("codex", ActionClass::Delete)
        };
        assert!(ledger.binding_for(&unknown).is_some());
    }

    // --- replay, rollback, expiry: none of them lifts a containment -------------------------

    #[test]
    fn a_replayed_bundle_is_refused_and_changes_nothing() {
        let mut ledger = ContainmentLedger::empty();
        let payload = notice(vec![entry("INC-1", "codex", "recommend")]);
        contained(&mut ledger, &payload, 5);
        let replay = bundle(1, "acme", 5, None, &payload);
        let older = bundle(
            1,
            "acme",
            4,
            None,
            &notice(vec![entry("INC-9", "claude", "observe")]),
        );
        for stale in [replay, older] {
            let before: Vec<IncidentEvidence> = ledger.active().cloned().collect();
            assert_eq!(
                ledger.ingest(&stale, &policy(1, "acme"), NOW, &release()),
                Err(ContainmentError::Replayed)
            );
            assert_eq!(before, ledger.active().cloned().collect::<Vec<_>>());
        }
    }

    #[test]
    fn a_later_bundle_that_omits_an_incident_does_not_lift_it() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "recommend")]),
            1,
        );
        contained(
            &mut ledger,
            &notice(vec![entry("INC-2", "claude", "recommend")]),
            2,
        );
        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Delete))
                .is_some()
        );
        assert!(
            ledger
                .binding_for(&target("claude", ActionClass::Delete))
                .is_some()
        );
    }

    #[test]
    fn rolling_the_knowledge_store_back_does_not_lift_a_containment() {
        let policy = policy(1, "acme");
        let first = bundle(1, "acme", 1, None, "{\"manifest\":\"v1\"}");
        let payload = notice(vec![entry("INC-1", "codex", "observe")]);
        let second = bundle(1, "acme", 2, None, &payload);
        let mut store = KnowledgeStore::empty();
        let mut ledger = ContainmentLedger::empty();
        store.apply(&first, &policy, NOW).expect("first applies");
        store.apply(&second, &policy, NOW).expect("second applies");
        ledger
            .ingest(&second, &policy, NOW, &release())
            .expect("containment applies");
        store
            .rollback(NOW)
            .expect("rollback to the pre-incident bundle");
        assert_eq!(store.current().map(|b| b.sequence), Some(1));
        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Delete))
                .is_some()
        );
    }

    #[test]
    fn an_expired_containment_bundle_is_refused_but_an_ingested_one_outlives_its_expiry() {
        let mut ledger = ContainmentLedger::empty();
        let payload = notice(vec![entry("INC-1", "codex", "observe")]);
        let expired = bundle(1, "acme", 1, Some(NOW), &payload);
        assert_eq!(
            ledger.ingest(&expired, &policy(1, "acme"), NOW, &release()),
            Err(ContainmentError::Bundle(KnowledgeBundleError::Expired))
        );
        assert_eq!(ledger.active().count(), 0);

        let expiring = bundle(1, "acme", 2, Some(NOW + 1), &payload);
        ledger
            .ingest(&expiring, &policy(1, "acme"), NOW, &release())
            .expect("valid until NOW + 1");
        // Time passes beyond the bundle's expiry; the containment stays.
        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Delete))
                .is_some()
        );
    }

    #[test]
    fn only_a_local_lift_removes_a_containment() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "observe")]),
            1,
        );
        assert!(ledger.lift_locally("INC-404").is_none());
        let lifted = ledger.lift_locally("INC-1").expect("was active");
        assert_eq!(lifted.incident_id, "INC-1");
        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Delete))
                .is_none()
        );
        assert!(!ledger.freezes_promotion_of("codex"));
    }

    // --- invalid signature, unknown publisher, tamper ----------------------------------------

    #[test]
    fn an_invalid_signature_unknown_publisher_or_tampered_payload_changes_nothing() {
        let payload = notice(vec![entry("INC-1", "codex", "observe")]);
        let forged = bundle(2, "acme", 1, None, &payload);
        let stranger = bundle(1, "mallory", 1, None, &payload);
        let mut tampered = bundle(1, "acme", 1, None, &payload);
        tampered.payload = notice(vec![entry("INC-1", "claude", "observe")]);
        let cases = [
            (forged, KnowledgeBundleError::InvalidSignature),
            (stranger, KnowledgeBundleError::UnknownSigner),
            (tampered, KnowledgeBundleError::ContentDigestMismatch),
        ];
        for (candidate, expected) in cases {
            let mut ledger = ContainmentLedger::empty();
            assert_eq!(
                ledger.ingest(&candidate, &policy(1, "acme"), NOW, &release()),
                Err(ContainmentError::Bundle(expected))
            );
            assert_eq!(ledger.active().count(), 0);
        }
    }

    #[test]
    fn a_refused_bundle_does_not_advance_the_replay_counter() {
        let mut ledger = ContainmentLedger::empty();
        let forged = bundle(
            2,
            "acme",
            9,
            None,
            &notice(vec![entry("INC-1", "codex", "observe")]),
        );
        assert!(
            ledger
                .ingest(&forged, &policy(1, "acme"), NOW, &release())
                .is_err()
        );
        let malformed = bundle(1, "acme", 8, None, "{\"schema_version\":1}");
        assert!(
            ledger
                .ingest(&malformed, &policy(1, "acme"), NOW, &release())
                .is_err()
        );
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "observe")]),
            1,
        );
        assert_eq!(ledger.active().count(), 1);
    }

    // --- AC2: offline and failed updates keep the installed kernel ---------------------------

    #[test]
    fn an_unreachable_service_is_offline_and_keeps_the_existing_ledger() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "observe")]),
            1,
        );
        let outcome = ledger.refresh(
            Err(KnowledgeUnavailable),
            &policy(1, "acme"),
            NOW,
            &release(),
        );
        assert_eq!(outcome, RefreshOutcome::Offline);
        assert!(
            ledger
                .binding_for(&target("codex", ActionClass::Delete))
                .is_some()
        );

        let mut fresh = ContainmentLedger::empty();
        let outcome = fresh.refresh(
            Err(KnowledgeUnavailable),
            &policy(1, "acme"),
            NOW,
            &release(),
        );
        assert_eq!(outcome, RefreshOutcome::Offline);
        assert!(
            fresh
                .binding_for(&target("codex", ActionClass::Delete))
                .is_none()
        );
    }

    #[test]
    fn refresh_refuses_garbage_and_applies_a_valid_bundle() {
        let mut ledger = ContainmentLedger::empty();
        assert_eq!(
            ledger.refresh(
                Ok("<html>captive portal</html>"),
                &policy(1, "acme"),
                NOW,
                &release()
            ),
            RefreshOutcome::Refused(ContainmentError::MalformedBundle)
        );
        let payload = notice(vec![entry("INC-1", "codex", "observe")]);
        let text =
            serde_json::to_string(&bundle(1, "acme", 1, None, &payload)).expect("serializes");
        match ledger.refresh(Ok(&text), &policy(1, "acme"), NOW, &release()) {
            RefreshOutcome::Applied(evidence) => assert_eq!(evidence.len(), 1),
            other => panic!("expected Applied, got {other:?}"),
        }
        assert!(matches!(
            ledger.refresh(Ok(&text), &policy(1, "acme"), NOW, &release()),
            RefreshOutcome::Refused(ContainmentError::Replayed)
        ));
    }

    // --- AC3: evidence records provenance and identifiers, never content ---------------------

    #[test]
    fn evidence_records_provenance_capability_provider_platform_and_invariants() {
        let mut scoped = entry("INC-7", "codex", "recommend");
        scoped["provider_versions"] = serde_json::json!(["2.0.0"]);
        scoped["action_classes"] = serde_json::json!(["delete", "archive"]);
        scoped["platforms"] = serde_json::json!(["windows"]);
        scoped["invariants"] = serde_json::json!(["SI-022", "SI-029"]);
        let payload = notice(vec![scoped]);
        let mut ledger = ContainmentLedger::empty();
        let evidence = ledger
            .ingest(
                &bundle(1, "acme", 3, None, &payload),
                &policy(1, "acme"),
                NOW,
                &release(),
            )
            .expect("applies");
        let record = evidence.first().expect("one record");
        assert_eq!(record.knowledge.publisher_id, "acme");
        assert_eq!(record.knowledge.sequence, 3);
        assert_eq!(record.knowledge.issued_at, NOW - 10);
        assert_eq!(record.knowledge.content_digest, hex_digest(&payload));
        assert_eq!(record.release, release());
        assert_eq!(record.provider_versions, Some(vec!["2.0.0".to_string()]));
        assert_eq!(
            record.action_classes,
            Some(vec![ActionClass::Delete, ActionClass::Archive])
        );
        assert_eq!(record.platforms, Some(vec![IncidentPlatform::Windows]));
        assert_eq!(
            record.invariants,
            vec!["SI-022".to_string(), "SI-029".to_string()]
        );
        assert_eq!(record.affected_releases, vec!["1.17.4".to_string()]);

        let json = serde_json::to_value(record).expect("serializes");
        let keys: Vec<&str> = json
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        for forbidden in ["payload", "content", "path", "message", "reason"] {
            assert!(!keys.contains(&forbidden), "{forbidden} in {keys:?}");
        }
        assert!(!json.to_string().contains("\"entries\""));
    }

    #[test]
    fn identifiers_that_could_carry_content_are_refused() {
        let cases: [(&str, serde_json::Value); 9] = [
            ("incident_id", serde_json::json!("")),
            ("incident_id", serde_json::json!("INC 1")),
            ("provider_id", serde_json::json!("../../.codex/sessions")),
            ("provider_id", serde_json::json!("-codex")),
            ("provider_id", serde_json::json!("x".repeat(65))),
            (
                "provider_versions",
                serde_json::json!(["user said: rm -rf"]),
            ),
            ("affected_releases", serde_json::json!(["C:\\Users\\me"])),
            ("invariants", serde_json::json!(["SI-22"])),
            ("invariants", serde_json::json!(["XX-022"])),
        ];
        for (field, value) in cases {
            let mut bad = entry("INC-1", "codex", "observe");
            bad[field] = value.clone();
            assert!(
                matches!(
                    parse_notice(&notice(vec![bad])),
                    Err(ContainmentError::MalformedNotice(_))
                ),
                "{field}={value}"
            );
        }
    }

    #[test]
    fn empty_or_oversized_scopes_and_lists_are_refused_rather_than_guessed() {
        for (field, value) in [
            ("provider_versions", serde_json::json!([])),
            ("action_classes", serde_json::json!([])),
            ("platforms", serde_json::json!([])),
            ("invariants", serde_json::json!([])),
            ("affected_releases", serde_json::json!(vec!["1.0.0"; 33])),
            ("invariants", serde_json::json!(vec!["SI-001"; 33])),
        ] {
            let mut bad = entry("INC-1", "codex", "observe");
            bad[field] = value;
            assert!(parse_notice(&notice(vec![bad])).is_err(), "{field}");
        }
    }

    #[test]
    fn malformed_notice_envelopes_are_refused_and_nothing_is_recorded() {
        let valid = entry("INC-1", "codex", "observe");
        let cases = [
            "not json".to_string(),
            notice(vec![]),
            notice(vec![valid.clone(); 65]),
            notice(vec![valid.clone(), valid.clone()]),
            serde_json::json!({"schema_version": 2, "kind": CONTAINMENT_KIND, "entries": [valid]})
                .to_string(),
            serde_json::json!({"schema_version": 1, "kind": "provider_manifest", "entries": [valid]})
                .to_string(),
        ];
        for (sequence, payload) in (1..).zip(cases) {
            let mut ledger = ContainmentLedger::empty();
            let result = ledger.ingest(
                &bundle(1, "acme", sequence, None, &payload),
                &policy(1, "acme"),
                NOW,
                &release(),
            );
            assert!(
                matches!(result, Err(ContainmentError::MalformedNotice(_))),
                "{payload}"
            );
            assert_eq!(ledger.active().count(), 0);
        }
    }

    #[test]
    fn one_bad_entry_rejects_the_whole_notice() {
        let mut bad = entry("INC-2", "codex", "observe");
        bad["invariants"] = serde_json::json!(["nope"]);
        let payload = notice(vec![entry("INC-1", "codex", "observe"), bad]);
        let mut ledger = ContainmentLedger::empty();
        assert!(
            ledger
                .ingest(
                    &bundle(1, "acme", 1, None, &payload),
                    &policy(1, "acme"),
                    NOW,
                    &release()
                )
                .is_err()
        );
        assert_eq!(ledger.active().count(), 0);
    }

    #[test]
    fn errors_render_a_diagnostic() {
        for error in [
            ContainmentError::MalformedBundle,
            ContainmentError::Bundle(KnowledgeBundleError::Expired),
            ContainmentError::Replayed,
            ContainmentError::MalformedNotice("x".to_string()),
        ] {
            assert!(!error.to_string().is_empty());
        }
    }

    // --- authority: containment only ever narrows -----------------------------------------

    fn permissive(user: AuthorityLevel) -> crate::AuthorityInputs {
        crate::AuthorityInputs {
            user_requested: user,
            artifact_ceiling: AuthorityLevel::Autopilot,
            confidence: cancellai_model::KnowledgeConfidence::Verified,
            activity: cancellai_model::ActivityState::Idle,
            protection: cancellai_model::ProtectionState::Normal,
            integrity: cancellai_model::IntegrityState::Healthy,
            provider_trust: TrustedTier::for_tests(ProviderTrust::BuiltinVerified),
        }
    }

    const LEVELS: [AuthorityLevel; 5] = [
        AuthorityLevel::Observe,
        AuthorityLevel::Recommend,
        AuthorityLevel::Quarantine,
        AuthorityLevel::Govern,
        AuthorityLevel::Autopilot,
    ];
    const CHANNELS: [ReleaseChannel; 3] = [
        ReleaseChannel::Nightly,
        ReleaseChannel::Beta,
        ReleaseChannel::Stable,
    ];

    #[test]
    fn an_empty_ledger_is_exactly_the_installed_kernel() {
        let ledger = ContainmentLedger::empty();
        for user in LEVELS {
            for channel in CHANNELS {
                let build = BuildChannel::for_tests(channel);
                let kernel = crate::effective_authority_for_channel(permissive(user), build);
                let contained = crate::effective_authority_under_containment(
                    permissive(user),
                    build,
                    &ledger,
                    &target("codex", ActionClass::Delete),
                );
                assert_eq!(kernel, contained, "{user:?} {channel:?}");
            }
        }
    }

    #[test]
    fn containment_never_raises_and_always_caps_below_any_mutation() {
        for ceiling in ["observe", "recommend"] {
            let mut ledger = ContainmentLedger::empty();
            contained(
                &mut ledger,
                &notice(vec![entry("INC-1", "codex", ceiling)]),
                1,
            );
            for user in LEVELS {
                for channel in CHANNELS {
                    let build = BuildChannel::for_tests(channel);
                    let kernel = crate::effective_authority_for_channel(permissive(user), build);
                    let contained = crate::effective_authority_under_containment(
                        permissive(user),
                        build,
                        &ledger,
                        &target("codex", ActionClass::Delete),
                    );
                    assert!(contained.level <= kernel.level);
                    assert!(
                        contained.level
                            < crate::authority::minimum_authority_for(ActionClass::Quarantine)
                    );
                }
            }
        }
    }

    #[test]
    fn containment_is_named_when_it_binds() {
        let mut ledger = ContainmentLedger::empty();
        contained(
            &mut ledger,
            &notice(vec![entry("INC-1", "codex", "observe")]),
            1,
        );
        let result = crate::effective_authority_under_containment(
            permissive(AuthorityLevel::Autopilot),
            BuildChannel::for_tests(ReleaseChannel::Stable),
            &ledger,
            &target("codex", ActionClass::Delete),
        );
        assert_eq!(result.level, AuthorityLevel::Observe);
        assert_eq!(
            result.binding_constraints,
            vec!["incident_containment_authority"]
        );
    }

    #[test]
    fn release_provenance_comes_from_the_build_not_the_caller() {
        let provenance = ReleaseProvenance::of_this_build(BuildChannel::default());
        assert_eq!(provenance.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(provenance.channel, ReleaseChannel::Nightly);
    }
}
