//! Guardian kill-switch (E15-S04, `docs/architecture/GUARDIAN_MODEL.md` "Kill switch"): an
//! immediate, local disable path that stops future autonomous mutations without corrupting
//! current state (AC1).
//!
//! **"Immediate local"** is deliberately a plain marker file, not the OS-service lifecycle
//! `cancellai_guardian::service` already provides - `service::ServiceRuntime::disable` shells out
//! to `launchctl`/`systemctl`/`schtasks`, which can fail (binary missing, no session bus) exactly
//! when a user most needs the disable to work. This module never depends on any of that: `engage`
//! is one local file write, checked by a plain read of the same file, with no external process
//! and no OS-service state to get out of sync with.
//!
//! **State lives in the file's content, never in its existence.** `engage`/`disengage` both
//! *write* the marker (`"engaged"`/`"disengaged"`) rather than creating/removing it -
//! `disengage` never calls `std::fs::remove_file`. `scripts/check_mutation_boundary.py` (SI-019:
//! "no crate but the safety executor deletes anything") refuses that call from any source file
//! but `cancellai-platform/src/mutation.rs`, unconditionally, with no exemption for cancellAI's
//! own local state - and correctly so: carving out "this deletion is only a byte of local state,
//! not provider data" is exactly the kind of judgment call a *structural* gate should not be
//! trusted to make file-by-file. A content-based marker satisfies the same requirement without
//! needing that judgment call at all.
//!
//! **"Without corrupting current state"** holds because [`apply_kill_switch`] is a pure,
//! in-memory transform over an already-computed [`GuardianPlanItem`] batch - no I/O, so there is
//! no partial/interrupted write it could ever leave behind - and because it never touches
//! execution at all: an in-flight destructive mutation still runs entirely inside
//! `cancellai_safety::mutation_executor`'s own transactional/TOCTOU-safe sequence, unaffected by
//! anything in this crate (`docs/architecture/GUARDIAN_MODEL.md`'s own text: "Any in-flight
//! destructive action still follows safety executor transaction semantics rather than being
//! killed mid-syscall unsafely"). This module has no ability to interrupt that even if it wanted
//! to - it only ever changes what a *future* planning call reports as actionable.
//!
//! **Fail-safe reads.** [`KillSwitch::is_engaged`] treats every outcome except a *confirmed*
//! absent marker as engaged - a permission error, or any other I/O failure, reads as engaged, not
//! disengaged. An unconfirmed kill-switch state must never be read as "safe to act," the same
//! "ambiguity never escalates privilege" principle (C-03) this codebase applies everywhere else
//! (`cancellai_platform::wsl`'s own "absence of a positive signal is the *weaker* classification"
//! - here, disengaged is the *stronger* one, so uncertainty resolves the other way).

use std::path::PathBuf;

use cancellai_model::{ActionClass, AuthorityLevel};

use crate::remediation::GuardianPlanItem;

#[derive(Debug)]
pub enum KillSwitchError {
    Io(String),
}

impl std::fmt::Display for KillSwitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KillSwitchError::Io(message) => write!(f, "kill-switch I/O failed: {message}"),
        }
    }
}

impl std::error::Error for KillSwitchError {}

/// A local, file-backed kill switch at a caller-supplied path - production callers point this
/// at a fixed location under Guardian's own local state directory; tests point it at a temporary
/// path.
pub struct KillSwitch {
    path: PathBuf,
}

const ENGAGED_MARKER: &str = "engaged";
const DISENGAGED_MARKER: &str = "disengaged";

impl KillSwitch {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn write(&self, content: &str) -> Result<(), KillSwitchError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| KillSwitchError::Io(err.to_string()))?;
        }
        std::fs::write(&self.path, content).map_err(|err| KillSwitchError::Io(err.to_string()))
    }

    /// Engages the switch. Idempotent - engaging an already-engaged switch is not an error.
    pub fn engage(&self) -> Result<(), KillSwitchError> {
        self.write(ENGAGED_MARKER)
    }

    /// Disengages the switch. Idempotent - disengaging a switch that was never engaged is not an
    /// error (the marker is simply written for the first time, already holding the disengaged
    /// content the postcondition needs).
    pub fn disengage(&self) -> Result<(), KillSwitchError> {
        self.write(DISENGAGED_MARKER)
    }

    /// See the module docs' "Fail-safe reads" section: only a marker that is either absent
    /// (never engaged) or reads **byte-for-byte exactly** [`DISENGAGED_MARKER`] counts as
    /// disengaged - not a trimmed/normalized comparison. Round-1 independent review found the
    /// original implementation compared `content.trim()` instead, which let content this crate
    /// never itself writes (a trailing newline, surrounding whitespace, any other modification)
    /// still read as a confirmed disengage; `disengage()` never appends a newline or any other
    /// byte beyond the marker itself, so an exact comparison rejects nothing legitimate. Present
    /// but unreadable, present with any other content, or any other I/O failure, all read as
    /// engaged. Never mutates.
    pub fn is_engaged(&self) -> bool {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => content != DISENGAGED_MARKER,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => true,
        }
    }
}

/// Applies the kill switch on top of an already-planned batch - never touches
/// `cancellai_guardian::remediation::plan_remediation` itself, only narrows its output further,
/// the same "only ever narrow, never grant" shape that module's own authority `min` already
/// uses. When engaged, every item is forced to [`AuthorityLevel::Observe`]/
/// [`ActionClass::Observe`] regardless of what pressure/policy would otherwise have granted -
/// read-only inspection of the plan itself is never withheld (the items are still returned, only
/// their granted authority is), matching `docs/architecture/GUARDIAN_MODEL.md`'s "Disabling
/// automation never prevents manual read-only inspection or recovery."
///
/// `pub(crate)`, matching `GuardianPlanItem`'s own visibility (`remediation.rs`'s module docs) -
/// this crate's remediation output is not externally reachable at all yet. `#[allow(dead_code)]`
/// because no production orchestrator wires this yet (this crate's own "primitive delivered, no
/// orchestrator yet" pattern) - exercised directly by this module's own tests.
#[allow(dead_code)]
pub(crate) fn apply_kill_switch(
    kill_switch: &KillSwitch,
    plan: Vec<GuardianPlanItem>,
) -> Vec<GuardianPlanItem> {
    if !kill_switch.is_engaged() {
        return plan;
    }
    plan.into_iter()
        .map(|item| GuardianPlanItem {
            granted_authority: AuthorityLevel::Observe,
            action_class: ActionClass::Observe,
            ..item
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pressure::PressureState;
    use cancellai_model::ArtifactId;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-guardian-killswitch-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn item(action_class: ActionClass, granted_authority: AuthorityLevel) -> GuardianPlanItem {
        GuardianPlanItem {
            artifact_id: ArtifactId::new("artifact-0001"),
            action_class,
            granted_authority,
            pressure_state: PressureState::Red,
            binding_constraints: vec!["test_constraint"],
        }
    }

    #[test]
    fn a_never_engaged_switch_reads_as_disengaged() {
        let dir = TempDir::new("never-engaged");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        assert!(!switch.is_engaged());
    }

    #[test]
    fn engage_then_is_engaged_reads_true() {
        let dir = TempDir::new("engage");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        switch.engage().expect("engage must succeed");
        assert!(switch.is_engaged());
    }

    #[test]
    fn engage_then_disengage_round_trips_to_false() {
        let dir = TempDir::new("round-trip");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        switch.engage().unwrap();
        switch.disengage().expect("disengage must succeed");
        assert!(!switch.is_engaged());
    }

    #[test]
    fn a_marker_that_merely_trims_to_disengaged_still_reads_as_engaged() {
        // Round-1 independent review: a trailing newline (content this crate's own disengage()
        // never writes) must not be treated as an honest disengage just because it trims down to
        // the right string - only a byte-for-byte exact match counts.
        let dir = TempDir::new("newline-marker");
        let marker = dir.0.join("killswitch");
        std::fs::write(&marker, "disengaged\n").unwrap();
        let switch = KillSwitch::at(marker);
        assert!(switch.is_engaged());
    }

    #[test]
    fn a_marker_with_leading_or_trailing_whitespace_still_reads_as_engaged() {
        let dir = TempDir::new("whitespace-marker");
        let marker = dir.0.join("killswitch");
        std::fs::write(&marker, "  disengaged  ").unwrap();
        let switch = KillSwitch::at(marker);
        assert!(switch.is_engaged());
    }

    #[test]
    fn disengage_when_never_engaged_is_not_an_error() {
        let dir = TempDir::new("disengage-absent");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        assert!(switch.disengage().is_ok());
    }

    #[test]
    fn engage_is_idempotent() {
        let dir = TempDir::new("engage-idempotent");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        switch.engage().unwrap();
        assert!(switch.engage().is_ok());
        assert!(switch.is_engaged());
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_marker_directory_reads_as_engaged_not_disengaged() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new("unreadable");
        let locked_subdir = dir.0.join("locked");
        std::fs::create_dir_all(&locked_subdir).unwrap();
        let marker = locked_subdir.join("killswitch");
        // A real, confirmable I/O failure (not "not found") reading the marker's own content -
        // must read as engaged (fail-safe), never as disengaged.
        std::fs::set_permissions(&locked_subdir, std::fs::Permissions::from_mode(0o000)).unwrap();
        let switch = KillSwitch::at(marker);
        assert!(switch.is_engaged());
        // Restore permissions so the TempDir guard can clean up.
        std::fs::set_permissions(&locked_subdir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn engage_failure_is_surfaced_and_never_reads_back_as_a_confirmed_disengage() {
        // A path whose parent cannot exist (a regular file standing where a directory is
        // required) makes create_dir_all fail - engage() must report that, not silently claim
        // success while the marker was never written.
        let dir = TempDir::new("engage-failure");
        let blocking_file = dir.0.join("not-a-directory");
        std::fs::write(&blocking_file, b"x").unwrap();
        let switch = KillSwitch::at(blocking_file.join("killswitch"));
        assert!(switch.engage().is_err());
        // The marker itself was never written, but reading a path whose parent is not a
        // directory is itself an unresolvable I/O error, not a confirmed "not found" - the
        // module docs' fail-safe rule reads that as engaged, not disengaged. This is the
        // conservative direction, not a gap: a caller who cannot tell whether the switch is off
        // must never proceed as if it were.
        assert!(switch.is_engaged());
    }

    #[test]
    fn disengaged_switch_leaves_the_plan_unchanged() {
        let dir = TempDir::new("apply-disengaged");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        let plan = vec![item(ActionClass::Quarantine, AuthorityLevel::Quarantine)];
        let result = apply_kill_switch(&switch, plan.clone());
        assert_eq!(result, plan);
    }

    #[test]
    fn engaged_switch_forces_every_item_to_observe() {
        let dir = TempDir::new("apply-engaged");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        switch.engage().unwrap();
        let plan = vec![
            item(ActionClass::Quarantine, AuthorityLevel::Quarantine),
            item(ActionClass::Observe, AuthorityLevel::Recommend),
        ];
        let result = apply_kill_switch(&switch, plan);
        for entry in &result {
            assert_eq!(entry.action_class, ActionClass::Observe);
            assert_eq!(entry.granted_authority, AuthorityLevel::Observe);
        }
    }

    #[test]
    fn engaged_switch_still_returns_every_item_read_only_inspection_is_not_withheld() {
        let dir = TempDir::new("apply-engaged-count");
        let switch = KillSwitch::at(dir.0.join("killswitch"));
        switch.engage().unwrap();
        let plan = vec![
            item(ActionClass::Quarantine, AuthorityLevel::Quarantine),
            item(ActionClass::Observe, AuthorityLevel::Recommend),
        ];
        let result = apply_kill_switch(&switch, plan);
        assert_eq!(result.len(), 2);
    }
}
