//! Remote target vocabulary (E18-S01, `docs/architecture/TARGET.md`, `docs/architecture/
//! DOMAIN_MODEL.md`'s `MachineId` sketch).
//!
//! This module models an SSH/dev-container/CI-runner target as an explicit machine with its
//! own identity, capabilities, and trust - the first real occupant of the `MachineId` slot
//! `DOMAIN_MODEL.md` has sketched since before this crate had any domain types at all
//! (`views::by_machine`'s own doc: "`MachineId` does not exist on `AgentArtifact` because
//! E18/E19's remote-target work has not landed"). It stays true after this story too: nothing
//! here touches `AgentArtifact`, `ArtifactId`, or any local-authority type. Wiring a real
//! `MachineId` onto observed artifacts is inventory-pipeline integration work for a later
//! story - inventing that wiring now, with no adapter yet producing remote facts, would repeat
//! exactly the "payload type nothing produces yet" case `cancellai-provider-api::capability`'s
//! own module doc already declines for the identical reason.
//!
//! Like [`crate::ProviderTrust`], the types here are plain, freely-constructible vocabulary,
//! not a gated authority. A remote target's `RemoteTargetTrust` is not, by itself, an
//! authorization to do anything - the same split `ProviderTrust`/`cancellai_safety::
//! TrustedTier` already draws: this crate stays free of policy/decision logic, and a future
//! CR4 story (E18-S02, "Local-agent remote execution boundary") is where a real execution
//! grant would have to route through a safety-crate gate, not this one.
//!
//! AC1 ("remote inventory never masquerades as local state") is enforced by shape, not
//! convention: [`InventoryOrigin`] has exactly two variants, and `Local` carries no
//! [`MachineId`] at all - there is no value of any kind that both variants can agree on, so no
//! remote target's id, however it is named, can ever compare equal to `Local`. [`RemoteTarget::
//! origin`] is the only way this module hands out an `InventoryOrigin` for a remote target, and
//! it can only ever produce `Remote`.
//!
//! AC2 ("disconnected targets preserve last-seen state as stale, not current") is
//! [`RemoteTargetConnection`]: a target is `Connected` or it is not, and [`RemoteTarget::
//! state_is_current`] answers `false` for anything but `Connected` - never a time-based guess
//! about how stale is "stale enough" to matter. A freshly constructed target
//! ([`RemoteTarget::new`]) starts `Disconnected`, matching C-02 ("unknown is protected"): a
//! target this module has not yet confirmed live is treated the same as one that dropped.

use serde::{Deserialize, Serialize};

/// Opaque, stable identity for one machine cancellAI can observe. Freely constructible pure
/// vocabulary, like `ProviderTrust` - it names a target, it does not authorize anything about
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MachineId(String);

impl MachineId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where an inventory observation came from. `Local` is the machine cancellAI itself runs on;
/// `Remote` names exactly which target it came from instead. See the module doc for why this
/// pair, and not a nullable/optional `MachineId`, is what actually discharges AC1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryOrigin {
    Local,
    Remote(MachineId),
}

impl InventoryOrigin {
    pub fn is_local(&self) -> bool {
        matches!(self, InventoryOrigin::Local)
    }
}

/// The three remote target shapes this epic scopes (`docs/architecture/TARGET.md`, E18
/// objective: "SSH/dev-container/CI-runner targets").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTargetKind {
    Ssh,
    DevContainer,
    CiRunner,
}

/// The independent capability questions a remote target answers for itself - "independent"
/// meaning per-instance, not inferred from `RemoteTargetKind`: two SSH targets can disagree on
/// either field. Bare booleans are appropriate here, unlike `cancellai_provider_api::
/// capability`'s evidenced `CapabilityOutcome`: that contract exists to make an adapter justify
/// *why* a provider capability holds, which matters once real capability results drive
/// planning; nothing here drives planning yet, and inventing evidence fields no acceptance
/// criterion asks for would be the same "placeholder payload" this module's own doc already
/// declines.
///
/// Deliberately `Serialize`, not `Deserialize`: `ProviderTrust`'s own history (E05 round-1,
/// `cancellai_safety::trust_promotion`'s module doc) is a document deserialized straight into a
/// bare `ProviderTrust`/capability value reaching an authority computation with no promotion
/// gate at all. Nothing here feeds an authority computation yet, but this type is a capability
/// grant in shape, and C-05 names capability/trust as authority inputs - so it stays
/// unconstructible from external data from day one, the same way `TrustedTier` closes that gap
/// for providers, rather than leaving a `Deserialize` a later story could wire straight into
/// `AuthorityInputs` and repeat E05's exact mistake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RemoteCapabilities {
    pub can_observe: bool,
    pub can_execute_mutations: bool,
}

/// Bare trust vocabulary for a remote target, mirroring `ProviderTrust`'s "declaration order is
/// the meaning" convention. `Untrusted` is the default for exactly the reason `ProviderTrust`'s
/// own tests fix: a freshly modeled target must not default to anything stronger.
///
/// `Serialize` only, not `Deserialize` - see [`RemoteCapabilities`]'s doc for why a bare trust
/// grant stays unconstructible from external data until a real gate (E18-S02 or later) exists.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTargetTrust {
    #[default]
    Untrusted,
    Trusted,
}

/// Whether a target's last-known facts are live or stale. See the module doc for why this is a
/// plain two-state fact rather than a time-window computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTargetConnection {
    Connected,
    Disconnected,
}

/// One SSH/dev-container/CI-runner machine, modeled as an explicit target with its own
/// identity, capabilities, trust, and connection state.
///
/// `Serialize` only - see [`RemoteCapabilities`]'s doc; a whole `RemoteTarget` carries a trust
/// grant, so it inherits the same "not constructible from external data yet" constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteTarget {
    pub id: MachineId,
    pub kind: RemoteTargetKind,
    pub capabilities: RemoteCapabilities,
    pub trust: RemoteTargetTrust,
    connection: RemoteTargetConnection,
}

impl RemoteTarget {
    /// A newly modeled target starts `Disconnected` - it has not yet been confirmed live, so
    /// its state is not current until something explicitly calls `mark_connected`.
    pub fn new(
        id: MachineId,
        kind: RemoteTargetKind,
        capabilities: RemoteCapabilities,
        trust: RemoteTargetTrust,
    ) -> Self {
        Self {
            id,
            kind,
            capabilities,
            trust,
            connection: RemoteTargetConnection::Disconnected,
        }
    }

    pub fn connection(&self) -> RemoteTargetConnection {
        self.connection
    }

    pub fn mark_connected(&mut self) {
        self.connection = RemoteTargetConnection::Connected;
    }

    pub fn mark_disconnected(&mut self) {
        self.connection = RemoteTargetConnection::Disconnected;
    }

    /// AC2: whether this target's last-known facts may be treated as current. `false` for
    /// anything but `Connected` - a target that was live a second ago is exactly as stale as
    /// one that has been gone for a week until it reconnects.
    pub fn state_is_current(&self) -> bool {
        matches!(self.connection, RemoteTargetConnection::Connected)
    }

    /// AC1: the only `InventoryOrigin` this module will ever hand out for a remote target -
    /// always `Remote`, regardless of how the target's id is spelled.
    pub fn origin(&self) -> InventoryOrigin {
        InventoryOrigin::Remote(self.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ssh_target(id: &str) -> RemoteTarget {
        RemoteTarget::new(
            MachineId::new(id),
            RemoteTargetKind::Ssh,
            RemoteCapabilities {
                can_observe: true,
                can_execute_mutations: false,
            },
            RemoteTargetTrust::Untrusted,
        )
    }

    #[test]
    fn a_fresh_target_defaults_to_untrusted_and_disconnected() {
        let target = ssh_target("build-box");
        assert_eq!(target.trust, RemoteTargetTrust::Untrusted);
        assert_eq!(target.connection(), RemoteTargetConnection::Disconnected);
        assert!(
            !target.state_is_current(),
            "an unconfirmed target must never be treated as current"
        );
    }

    #[test]
    fn mock_remote_lifecycle_connect_disconnect_reconnect() {
        let mut target = ssh_target("ci-runner-1");

        target.mark_connected();
        assert!(target.state_is_current());

        target.mark_disconnected();
        assert!(
            !target.state_is_current(),
            "a disconnected target's prior state must go stale, not stay current"
        );

        target.mark_connected();
        assert!(
            target.state_is_current(),
            "reconnecting must restore current state"
        );
    }

    #[test]
    fn ac1_no_machine_id_however_spelled_ever_compares_equal_to_local() {
        for id in ["", "local", "LOCAL", "localhost", "this-machine"] {
            let origin = InventoryOrigin::Remote(MachineId::new(id));
            assert_ne!(
                origin,
                InventoryOrigin::Local,
                "a remote id spelled {id:?} must never masquerade as local state"
            );
            assert!(!origin.is_local());
        }
        assert!(InventoryOrigin::Local.is_local());
    }

    #[test]
    fn ac1_a_remote_targets_own_origin_is_never_local() {
        let target = ssh_target("dev-container-7");
        assert_eq!(
            target.origin(),
            InventoryOrigin::Remote(MachineId::new("dev-container-7"))
        );
        assert!(!target.origin().is_local());
    }

    #[test]
    fn capabilities_and_trust_are_independent_per_instance_not_inferred_from_kind() {
        let observer_only = RemoteTarget::new(
            MachineId::new("a"),
            RemoteTargetKind::Ssh,
            RemoteCapabilities {
                can_observe: true,
                can_execute_mutations: false,
            },
            RemoteTargetTrust::Untrusted,
        );
        let trusted_executor = RemoteTarget::new(
            MachineId::new("b"),
            RemoteTargetKind::Ssh,
            RemoteCapabilities {
                can_observe: true,
                can_execute_mutations: true,
            },
            RemoteTargetTrust::Trusted,
        );

        assert_eq!(observer_only.kind, trusted_executor.kind);
        assert_ne!(observer_only.capabilities, trusted_executor.capabilities);
        assert_ne!(observer_only.trust, trusted_executor.trust);
    }

    #[test]
    fn remote_target_trust_defaults_to_untrusted() {
        assert_eq!(RemoteTargetTrust::default(), RemoteTargetTrust::Untrusted);
    }

    #[test]
    fn machine_id_round_trips_through_json() {
        let id = MachineId::new("ci-runner-42");
        let json = serde_json::to_string(&id).expect("serialize");
        let round_tripped: MachineId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, round_tripped);
    }

    #[test]
    fn machine_id_survives_unicode_and_control_characters() {
        // MachineId is opaque and never parsed - a hostile or merely exotic id must round-trip
        // byte-for-byte rather than panicking or being silently normalized.
        for raw in ["café-runner", "🖥️-box", "a\u{0}b", "a\nb\tc"] {
            let id = MachineId::new(raw);
            assert_eq!(id.as_str(), raw);
            let json = serde_json::to_string(&id).expect("serialize");
            let round_tripped: MachineId = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(id, round_tripped);
        }
    }
}
