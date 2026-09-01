//! The retention resolver: CLASSIFY + RESOLVE + PLAN for cancellAI's two built-in providers
//! (`docs/architecture/TARGET.md`'s core loop), ported from `cancellai.py`'s `build_plan`/
//! `choose_old_sessions`/`choose_codex_old_sessions` (E06-S01).
//!
//! This crate was an empty skeleton before E06 because, per `cancellai-model::agent_artifact`'s
//! module docs, deriving `RiskClass`/lifecycle axes/`AuthorityCeiling` for a discovered session
//! is a classification decision no earlier story had the provider/policy knowledge to make.
//! `docs/adrs/0016-rust-artifact-risk-classification.md` records that decision; this module
//! implements it.
//!
//! Scope note on what this does *not* port from `cancellai.py`: `--aggressive` (legacy/cache
//! category widening) is not implemented here - it would require porting a second, separate
//! discovery surface (`CLAUDE_LEGACY_PATHS`/`CLAUDE_SAFE_CACHE_FILES`) this story does not need
//! to touch the core CLASSIFY/PLAN pipeline. Omitting a *widening* flag is fail-closed (the
//! Rust CLI simply finds fewer candidates than Python's `--aggressive` would, never more), so
//! this is a tracked parity gap, not a safety gap - see the E06-S01 evidence packet.

use cancellai_model::{
    Action, ActionClass, ActivityState, AgentArtifact, ArtifactId, AuthorityLevel, Evidence,
    EvidenceId, IntegrityState, KnowledgeConfidence, Precondition, ProtectionState, ResidencyState,
    Reversibility, RiskClass,
};
use cancellai_platform::{Clock, FsObserver, Observation, ProcessObserver, Timestamp};
use cancellai_provider_api::ProtectionOutcome;
use cancellai_provider_claude::{ClaudeSession, SessionDiscoveryScope, discover_claude_sessions};
use cancellai_provider_codex::{CodexSession, discover_codex_sessions, group_into_subagent_trees};
use cancellai_safety::authority::minimum_authority_for;
use cancellai_safety::{AuthorityInputs, TrustedTier, effective_authority};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Which tool(s) a run applies to (`cancellai.py`'s `--tool {all,codex,claude}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolScope {
    All,
    Codex,
    Claude,
}

impl ToolScope {
    fn includes_codex(self) -> bool {
        matches!(self, ToolScope::All | ToolScope::Codex)
    }

    fn includes_claude(self) -> bool {
        matches!(self, ToolScope::All | ToolScope::Claude)
    }
}

/// The retention policy a `status`/`plan`/`clean` invocation resolves against
/// (`cancellai.py`'s `--days`/`--keep-latest`/`--tool`).
#[derive(Debug, Clone, Copy)]
pub struct RetentionPolicy {
    pub days: u32,
    pub keep_latest: u32,
    pub tool: ToolScope,
    /// Mirrors `cancellai.py`'s `--allow-running`: proceed even though provider-process
    /// liveness could not be established, or was established as running. Never implied by any
    /// other flag (SI-007) - a caller must set this explicitly.
    pub allow_running: bool,
}

/// One fully classified session, plus the working data (path, size, group key) `AgentArtifact`
/// itself does not carry (see `cancellai-model::agent_artifact`'s module docs for why).
#[derive(Debug, Clone)]
pub struct ClassifiedArtifact {
    pub artifact: AgentArtifact,
    pub path: PathBuf,
    pub size_bytes: u64,
    /// The [`AuthorityLevel`] this artifact could reach if the caller requested the maximum
    /// (`Autopilot`) - i.e. the ceiling every *other* constraint (confidence, lifecycle,
    /// provider trust, the artifact's own ceiling) already agrees on, before `user_requested`
    /// is even considered. `plan`/`clean` compare this against
    /// `cancellai_safety::minimum_authority_for(ActionClass::Delete)` to decide whether an
    /// artifact is a real deletion candidate or an observation-only entry.
    pub reachable_authority: AuthorityLevel,
    /// Why `reachable_authority` is what it is - the same binding-constraint names
    /// `cancellai_safety::EffectiveAuthority::binding_constraints` reports, carried through so
    /// a caller can explain a withheld action instead of just stating the fact.
    pub binding_constraints: Vec<&'static str>,
}

/// Everything one provider's resolution produced: every discovered artifact (for `status`/
/// `inspect`), and whether the scan was complete enough to authorize any destructive action at
/// all (SI-008, SI-009: a structurally incomplete scan withholds, it never silently proceeds
/// with what little it saw).
#[derive(Debug, Clone)]
pub struct ProviderResolution {
    pub provider_id: &'static str,
    pub artifacts: Vec<ClassifiedArtifact>,
    pub scan_complete: bool,
    pub scan_incomplete_reason: Option<String>,
}

fn stable_id(prefix: &str, parts: &[&str]) -> String {
    let mut hasher = DefaultHasher::new();
    for part in parts {
        part.hash(&mut hasher);
    }
    format!("{prefix}-{:016x}", hasher.finish())
}

fn observe_mtime(fs: &dyn FsObserver, path: &Path) -> Option<Timestamp> {
    match fs.observe(path) {
        Observation::Metadata(meta) => Some(meta.modified),
        Observation::Absent | Observation::Unreadable { .. } => None,
    }
}

/// Every constraint besides `user_requested`, evaluated as if the caller asked for
/// [`AuthorityLevel::Autopilot`] - i.e. "what is the highest authority every other independent
/// safety constraint would allow here?" `plan`/`clean` then compare this to the real minimum a
/// given `ActionClass` requires (`cancellai_safety::minimum_authority_for`), and `clean` itself
/// still calls `cancellai_safety::effective_authority`/`execute` again with the real
/// `user_requested` immediately before mutating (this function never substitutes for that -
/// see `retention.rs`'s callers).
fn reachable_authority(
    artifact_ceiling: AuthorityLevel,
    confidence: KnowledgeConfidence,
    activity: ActivityState,
    protection: ProtectionState,
    integrity: IntegrityState,
    provider_trust: TrustedTier,
) -> (AuthorityLevel, Vec<&'static str>) {
    let result = effective_authority(AuthorityInputs {
        user_requested: AuthorityLevel::Autopilot,
        artifact_ceiling,
        confidence,
        activity,
        protection,
        integrity,
        provider_trust,
    });
    (result.level, result.binding_constraints)
}

/// Resolve Claude Code sessions under `root` against `policy`. `root` is the already-probed
/// [`cancellai_provider_claude::ClaudeProvider`]'s own root path, kept as a bare `&Path` here
/// so this module stays free of a hard dependency on the provider struct's own API surface
/// beyond the free functions it re-exports (`discover_claude_sessions`, protected-name check).
pub fn resolve_claude(
    root: &Path,
    protection: impl Fn(&Path) -> ProtectionOutcome,
    policy: &RetentionPolicy,
    process: &dyn ProcessObserver,
    clock: &dyn Clock,
    provider_trust: TrustedTier,
) -> ProviderResolution {
    let empty = || ProviderResolution {
        provider_id: "claude-code",
        artifacts: Vec::new(),
        scan_complete: true,
        scan_incomplete_reason: None,
    };
    if !policy.tool.includes_claude() {
        return empty();
    }
    if !root.exists() {
        // A provider simply not installed on this machine is a known-empty state, not missing
        // evidence (SI-009 distinguishes the two): report it as complete.
        return empty();
    }

    let discovered = discover_claude_sessions(root);
    if discovered.scope == SessionDiscoveryScope::Unavailable {
        // A `claude_home` that exists but has no `projects/` (or a symlinked one) is a
        // structurally empty install, not a failed observation - `cancellai.py`'s own
        // `build_plan` does not withhold the tool in this case either (E06-S02 differential
        // gate finding: an earlier version of this function treated `Unavailable` the same as
        // a mid-scan companion-listing failure, which withheld the whole tool for a fixture
        // Python reports zero-candidates-but-not-withheld for). `discover_claude_sessions`
        // already refuses to follow a symlinked `projects/` (SI-003), so there is no
        // discoverable evidence being silently dropped here - only "nothing to report."
        return empty();
    }

    let liveness = process.observe(&["claude"]);
    let process_active = liveness.is_running("claude") && !policy.allow_running;
    let cutoff = clock
        .now()
        .0
        .saturating_sub(u64::from(policy.days) * 86_400);

    // Protect the `keep_latest` most-recently-modified unique sessions, matching
    // `cancellai.py::choose_old_sessions` (mtime desc, unique by session_id).
    let mut ordered: Vec<&ClaudeSession> = discovered.sessions.iter().collect();
    ordered.sort_by(|a, b| {
        let ka = a.modified.map(observed_secs);
        let kb = b.modified.map(observed_secs);
        kb.cmp(&ka)
    });
    let protected_ids: std::collections::HashSet<&str> = ordered
        .iter()
        .take(policy.keep_latest as usize)
        .map(|s| s.session_id.as_str())
        .collect();

    let degraded: std::collections::HashSet<&Path> = discovered
        .degraded_companions
        .iter()
        .map(PathBuf::as_path)
        .collect();

    let mut artifacts: Vec<ClassifiedArtifact> = discovered
        .sessions
        .iter()
        .map(|session| {
            let mtime = session.modified.map(|m| {
                Timestamp(
                    m.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                )
            });
            let is_protected = protection(&session.path).is_protected();
            let is_pinned = protected_ids.contains(session.session_id.as_str());
            let degraded_companion = session
                .companion_payload
                .as_deref()
                .is_some_and(|p| degraded.contains(p));
            classify(
                "claude-code",
                &session.path,
                session.size_bytes,
                mtime,
                &format!(
                    "claude:projects/{}/{}.jsonl",
                    session.project, session.session_id
                ),
                &session.session_id,
                is_protected,
                is_pinned,
                degraded_companion,
                process_active,
                cutoff,
                provider_trust,
            )
        })
        .collect();

    // SI-008/SI-009: a companion payload directory that could not be fully listed makes the
    // *whole tool's* scan partial, not merely that one session's evidence - `cancellai.py`'s
    // own `build_plan` withholds every candidate for a tool the instant any of its scan scopes
    // is incomplete (`Plan.withheld`), never just the specific artifact whose evidence was
    // degraded. Without this, two other, perfectly ordinary sessions in the same fixture would
    // still be proposed for deletion even though this run could not prove the tool's directory
    // tree was fully observed - exactly the "absence of evidence read as absence of active/
    // protected data" mistake SI-009 exists to prevent. Found via the E06-S02 differential
    // parity gate diverging from the committed `claude-partial-tree` characterization.
    if !discovered.degraded_companions.is_empty() {
        // `docs/architecture/JSON_CONTRACTS.md`: "An artifact produced from a PARTIAL or
        // UNKNOWN scan_completeness scope must carry knowledge_confidence no higher than
        // LOW/UNKNOWN for that scope" - this applies to *every* artifact this scope produced,
        // not only the one session whose own companion payload was degraded (E06 verifier
        // review round 1: the other, perfectly-readable sessions in the same partial scan kept
        // reporting `Verified`, overstating what this run actually proved).
        for classified in &mut artifacts {
            classified.artifact.knowledge_confidence = KnowledgeConfidence::LowUnknown;
        }
        return ProviderResolution {
            provider_id: "claude-code",
            artifacts,
            scan_complete: false,
            scan_incomplete_reason: Some(format!(
                "{} companion payload director{} could not be fully listed; every action for \
                 this tool is withheld until the scan can be re-observed complete",
                discovered.degraded_companions.len(),
                if discovered.degraded_companions.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            )),
        };
    }

    ProviderResolution {
        provider_id: "claude-code",
        artifacts,
        scan_complete: true,
        scan_incomplete_reason: None,
    }
}

/// Resolve Codex CLI rollouts under `root` against `policy`, grouping into subagent trees
/// first (`cancellai_provider_codex::group_into_subagent_trees`, already ported at E05-S04) so
/// `keep_latest`/cutoff apply per *tree*, not per rollout file - an old-looking parent whose
/// child was touched a minute ago must stay protected in its entirety, matching
/// `cancellai.py::choose_codex_old_sessions`'s own "a recent subagent protects the whole tree"
/// rule.
pub fn resolve_codex(
    root: &Path,
    protection: impl Fn(&Path) -> ProtectionOutcome,
    policy: &RetentionPolicy,
    fs: &dyn FsObserver,
    process: &dyn ProcessObserver,
    clock: &dyn Clock,
    provider_trust: TrustedTier,
) -> ProviderResolution {
    let empty = || ProviderResolution {
        provider_id: "codex-cli",
        artifacts: Vec::new(),
        scan_complete: true,
        scan_incomplete_reason: None,
    };
    if !policy.tool.includes_codex() {
        return empty();
    }
    if !root.exists() {
        return empty();
    }

    let sessions = discover_codex_sessions(root);
    let trees = group_into_subagent_trees(&sessions);
    let liveness = process.observe(&["codex", "Codex"]);
    let process_active =
        (liveness.is_running("codex") || liveness.is_running("Codex")) && !policy.allow_running;
    let cutoff = clock
        .now()
        .0
        .saturating_sub(u64::from(policy.days) * 86_400);

    // Per-tree effective mtime: the max observed mtime across every member, `None` if any
    // member's mtime could not be observed (a partial fact about the tree as a whole, not
    // silently treated as "very old").
    struct TreeFacts {
        effective_mtime: Option<u64>,
        member_mtimes: std::collections::HashMap<String, Option<u64>>,
    }
    let tree_facts: Vec<(String, TreeFacts, &[CodexSession])> = trees
        .iter()
        .map(|tree| {
            let mut effective: Option<u64> = None;
            let mut any_unknown = false;
            let mut member_mtimes = std::collections::HashMap::new();
            for member in &tree.members {
                let mtime = observe_mtime(fs, &member.path).map(|t| t.0);
                if mtime.is_none() {
                    any_unknown = true;
                }
                effective = match (effective, mtime) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (Some(a), None) => Some(a),
                    (None, m) => m,
                };
                member_mtimes.insert(member.session_id.clone(), mtime);
            }
            (
                tree.root_id.clone(),
                TreeFacts {
                    effective_mtime: if any_unknown { None } else { effective },
                    member_mtimes,
                },
                tree.members.as_slice(),
            )
        })
        .collect();

    let mut ordered: Vec<&(String, TreeFacts, &[CodexSession])> = tree_facts.iter().collect();
    ordered.sort_by(|a, b| b.1.effective_mtime.cmp(&a.1.effective_mtime));
    let protected_roots: std::collections::HashSet<&str> = ordered
        .iter()
        .take(policy.keep_latest as usize)
        .map(|(root_id, _, _)| root_id.as_str())
        .collect();

    let mut artifacts = Vec::new();
    for (root_id, facts, members) in &tree_facts {
        let tree_pinned = protected_roots.contains(root_id.as_str());
        let tree_integrity_unknown = facts.effective_mtime.is_none();
        for member in *members {
            let mtime = facts
                .member_mtimes
                .get(&member.session_id)
                .copied()
                .flatten();
            let is_protected = protection(&member.path).is_protected();
            let identity = format!(
                "codex:{}/{}.jsonl",
                member.category.label(),
                member.session_id
            );
            artifacts.push(classify(
                "codex-cli",
                &member.path,
                member.size_bytes,
                mtime.map(Timestamp),
                &identity,
                root_id,
                is_protected,
                tree_pinned,
                tree_integrity_unknown && mtime.is_none(),
                process_active,
                cutoff,
                provider_trust,
            ));
        }
    }

    ProviderResolution {
        provider_id: "codex-cli",
        artifacts,
        scan_complete: true,
        scan_incomplete_reason: None,
    }
}

fn observed_secs(t: std::time::SystemTime) -> u64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Classify one session-shaped artifact along every lifecycle axis and compute the authority
/// it could reach (`docs/adrs/0016-rust-artifact-risk-classification.md` is the record of the
/// mapping this function implements):
///
/// - a protected-name match is `R5_PROTECTED`, ceiling `Observe` - never destructive,
///   regardless of any other fact (SI-006 defense in depth);
/// - an ordinary discovered session is `R3_RESUMABLE` ("removal can destroy session/history/
///   resume value," DOMAIN_MODEL.md's own definition), ceiling `Govern` - the minimum this
///   build's mutation executor actually requires for the one destructive operation it
///   implements today (`ActionClass::Delete`; `cancellai-safety::mutation_executor` does not
///   implement `Quarantine` yet, see that module's own docs, so a `Quarantine` ceiling here
///   would make `clean` permanently unable to do anything, which is not a more conservative
///   choice, just a differently-broken one). `Reversibility::Irreversible` follows the same
///   real-capability constraint.
///
/// Everything else this codebase already treats as an independent authority-reducing fact
/// (`cancellai_safety::authority::effective_authority`'s existing constraints) is expressed
/// through the other lifecycle fields, not through a second special case here: an unreadable/
/// missing mtime is `IntegrityState::Unknown`; a currently-running provider process is
/// `ActivityState::Active`; low provider-root confidence is a lower `KnowledgeConfidence`.
/// Each collapses `reachable_authority` toward `Recommend` on its own via the constraints
/// already wired in `cancellai-safety`, so this function does not need to special-case their
/// combinations - the monotonic minimum does that once, generically, for every caller.
#[allow(clippy::too_many_arguments)]
fn classify(
    provider_id: &'static str,
    path: &Path,
    size_bytes: u64,
    mtime: Option<Timestamp>,
    identity_token: &str,
    group_key: &str,
    is_protected: bool,
    is_pinned: bool,
    degraded_evidence: bool,
    process_active: bool,
    cutoff_secs: u64,
    provider_trust: TrustedTier,
) -> ClassifiedArtifact {
    let confidence = if degraded_evidence {
        KnowledgeConfidence::Observed
    } else {
        KnowledgeConfidence::Verified
    };
    let integrity = if mtime.is_none() {
        IntegrityState::Unknown
    } else if degraded_evidence {
        IntegrityState::Partial
    } else {
        IntegrityState::Healthy
    };
    let activity = if process_active {
        ActivityState::Active
    } else {
        match mtime {
            Some(t) if t.0 < cutoff_secs => ActivityState::Stale,
            Some(_) => ActivityState::Idle,
            None => ActivityState::Unknown,
        }
    };
    let protection = if is_protected {
        ProtectionState::Protected
    } else if is_pinned {
        ProtectionState::Pinned
    } else {
        ProtectionState::Normal
    };
    let (risk_class, ceiling, reversibility) = if is_protected {
        (
            RiskClass::R5Protected,
            AuthorityLevel::Observe,
            Reversibility::Unknown,
        )
    } else {
        (
            RiskClass::R3Resumable,
            AuthorityLevel::Govern,
            Reversibility::Irreversible,
        )
    };

    let (reachable, binding_constraints) = reachable_authority(
        ceiling,
        confidence,
        activity,
        protection,
        integrity,
        provider_trust,
    );

    let evidence_id = EvidenceId::new(stable_id("evidence", &[provider_id, identity_token]));
    let evidence_description = format!(
        "{provider_id} artifact at {} observed via filesystem scan (mtime {})",
        path.display(),
        mtime
            .map(|t| t.0.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );

    let artifact = AgentArtifact {
        artifact_id: ArtifactId::new(stable_id("artifact", &[provider_id, identity_token])),
        identity_token: identity_token.to_string(),
        provider_id: provider_id.to_string(),
        artifact_type: "session".to_string(),
        risk_class,
        reversibility,
        knowledge_confidence: confidence,
        activity_state: activity,
        residency_state: ResidencyState::Hot,
        protection_state: protection,
        integrity_state: integrity,
        authority_ceiling: ceiling,
        evidence_ids: vec![evidence_id.clone()],
    };

    let _ = Evidence::new(evidence_id.0.clone(), evidence_description);
    let _ = group_key;

    ClassifiedArtifact {
        artifact,
        path: path.to_path_buf(),
        size_bytes,
        reachable_authority: reachable,
        binding_constraints,
    }
}

/// Turn every classified artifact from one or more [`ProviderResolution`]s into the
/// `docs/architecture/JSON_CONTRACTS.md` plan document's `Action` list: eligible, stale,
/// unprotected artifacts that can reach `Delete`'s required authority become `Delete`
/// candidates with real execution preconditions; everything else is reported as `Observe`,
/// with a reason naming why (SI-007: never silently omitted, always explained).
pub fn build_actions(resolutions: &[ProviderResolution]) -> Vec<Action> {
    let delete_minimum = minimum_authority_for(ActionClass::Delete);
    resolutions
        .iter()
        .flat_map(|resolution| {
            resolution
                .artifacts
                .iter()
                .map(move |classified| (resolution, classified))
        })
        .map(|(resolution, classified)| {
            let target = classified.artifact.artifact_id.clone();
            // SI-008/SI-009: an incomplete scan withholds every action for the whole tool, not
            // only the specific artifact whose own evidence was degraded - see
            // `resolve_claude`'s own module-doc note on why (E06-S02 differential gate finding).
            if !resolution.scan_complete {
                let reason = resolution
                    .scan_incomplete_reason
                    .clone()
                    .unwrap_or_else(|| "provider scan was incomplete".to_string());
                return observe_action(target, &classified.artifact.evidence_ids, &reason);
            }
            if classified.artifact.activity_state != ActivityState::Stale {
                return observe_action(
                    target,
                    &classified.artifact.evidence_ids,
                    "artifact is inside the retention window or its activity could not be \
                     confirmed as stale; reported for visibility only",
                );
            }
            if classified.reachable_authority < delete_minimum {
                let reason = format!(
                    "artifact is stale but blocked from deletion by: {}",
                    classified.binding_constraints.join(", ")
                );
                return observe_action(target, &classified.artifact.evidence_ids, &reason);
            }
            Action {
                action_id: cancellai_model::ActionId::new(stable_id(
                    "action",
                    &["delete", &classified.artifact.identity_token],
                )),
                target_artifact_ids: vec![target],
                action_class: ActionClass::Delete,
                reason: "artifact is past the retention cutoff, not protected, not among the \
                         kept-latest sessions, and no provider process appears to be using it"
                    .to_string(),
                authority: delete_minimum,
                reversibility: classified.artifact.reversibility,
                evidence_ids: classified.artifact.evidence_ids.clone(),
                execution_preconditions: vec![
                    Precondition::new("identity_token", classified.artifact.identity_token.clone()),
                    Precondition::new("process_not_running", true),
                ],
            }
        })
        .collect()
}

fn observe_action(target: ArtifactId, evidence_ids: &[EvidenceId], reason: &str) -> Action {
    Action {
        action_id: cancellai_model::ActionId::new(stable_id("action", &["observe", &target.0])),
        target_artifact_ids: vec![target],
        action_class: ActionClass::Observe,
        reason: reason.to_string(),
        authority: AuthorityLevel::Observe,
        reversibility: Reversibility::Rebuildable,
        evidence_ids: evidence_ids.to_vec(),
        execution_preconditions: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cancellai_platform::{FrozenClock, SyntheticProcessObserver, SystemFsObserver};
    use cancellai_provider_api::ProtectionOutcome;

    fn clear(_: &Path) -> ProtectionOutcome {
        ProtectionOutcome::Clear
    }

    fn tree(dir: &Path, label: &str) -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "cancellai-policy-retention-test-{label}-{}",
                std::process::id()
            ))
            .join(dir)
    }

    /// Every mtime `filetime_set` below writes is near the Unix epoch, so a clock frozen a
    /// modest, fixed distance past it (rather than real wall-clock "now," which would make
    /// every test's staleness outcome depend on when it happens to run) gives a stable,
    /// reproducible cutoff well past any of this module's fixture mtimes.
    fn frozen_now() -> FrozenClock {
        FrozenClock::at(10 * 86_400)
    }

    fn filetime_set(path: &Path, seconds_since_epoch: u64) {
        // std has no portable mtime setter without a new dependency; `File::set_modified` is
        // stable since 1.75, well within this workspace's MSRV 1.85.
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds_since_epoch),
        )
        .unwrap();
    }

    #[test]
    fn a_missing_provider_root_is_a_known_empty_scan_not_a_withheld_one() {
        let root = tree(Path::new("nonexistent"), "missing-root");
        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 2,
            tool: ToolScope::All,
            allow_running: false,
        };
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(&root, clear, &policy, &process, &clock, trust);
        assert!(resolution.scan_complete);
        assert!(resolution.artifacts.is_empty());
    }

    #[test]
    fn a_stale_unprotected_session_reaches_delete_authority_when_everything_else_is_clean() {
        let dir = tree(Path::new(""), "stale-eligible");
        std::fs::create_dir_all(dir.join("projects/proj-a")).unwrap();
        let session_path = dir.join("projects/proj-a/11111111-1111-4111-8111-111111111111.jsonl");
        std::fs::write(&session_path, "{}").unwrap();
        filetime_set(&session_path, 0);

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 0,
            tool: ToolScope::Claude,
            allow_running: false,
        };
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(&dir, clear, &policy, &process, &clock, trust);
        assert_eq!(resolution.artifacts.len(), 1);
        let classified = &resolution.artifacts[0];
        assert_eq!(classified.artifact.activity_state, ActivityState::Stale);
        assert_eq!(
            classified.reachable_authority,
            AuthorityLevel::Govern,
            "binding constraints were: {:?}",
            classified.binding_constraints
        );

        let actions = build_actions(std::slice::from_ref(&resolution));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_class, ActionClass::Delete);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_running_provider_process_blocks_every_action_for_that_tool_even_when_stale() {
        let dir = tree(Path::new(""), "process-active");
        std::fs::create_dir_all(dir.join("projects/proj-a")).unwrap();
        let session_path = dir.join("projects/proj-a/22222222-2222-4222-8222-222222222222.jsonl");
        std::fs::write(&session_path, "{}").unwrap();
        filetime_set(&session_path, 0);

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 0,
            tool: ToolScope::Claude,
            allow_running: false,
        };
        // "Running" - process observer reports claude as live.
        let process = SyntheticProcessObserver::complete(vec!["claude".to_string()]);
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(&dir, clear, &policy, &process, &clock, trust);
        let actions = build_actions(std::slice::from_ref(&resolution));
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].action_class,
            ActionClass::Observe,
            "a live provider process must block deletion even of an otherwise-eligible session"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_incomplete_process_probe_fails_closed_exactly_like_a_running_process() {
        let dir = tree(Path::new(""), "process-unknown");
        std::fs::create_dir_all(dir.join("projects/proj-a")).unwrap();
        let session_path = dir.join("projects/proj-a/33333333-3333-4333-8333-333333333333.jsonl");
        std::fs::write(&session_path, "{}").unwrap();
        filetime_set(&session_path, 0);

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 0,
            tool: ToolScope::Claude,
            allow_running: false,
        };
        let process = SyntheticProcessObserver::incomplete();
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(&dir, clear, &policy, &process, &clock, trust);
        let actions = build_actions(std::slice::from_ref(&resolution));
        assert_eq!(actions[0].action_class, ActionClass::Observe);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_protected_name_is_never_a_deletion_candidate_even_if_stale_and_unpinned() {
        let dir = tree(Path::new(""), "protected");
        std::fs::create_dir_all(dir.join("projects/proj-a")).unwrap();
        let session_path = dir.join("projects/proj-a/44444444-4444-4444-8444-444444444444.jsonl");
        std::fs::write(&session_path, "{}").unwrap();
        filetime_set(&session_path, 0);

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 0,
            tool: ToolScope::Claude,
            allow_running: false,
        };
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(
            &dir,
            |_| ProtectionOutcome::Protected {
                matched_name: "settings.json".to_string(),
            },
            &policy,
            &process,
            &clock,
            trust,
        );
        assert_eq!(
            resolution.artifacts[0].artifact.authority_ceiling,
            AuthorityLevel::Observe
        );
        let actions = build_actions(std::slice::from_ref(&resolution));
        assert_eq!(actions[0].action_class, ActionClass::Observe);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn keep_latest_protects_the_most_recently_modified_sessions_from_deletion() {
        let dir = tree(Path::new(""), "keep-latest");
        std::fs::create_dir_all(dir.join("projects/proj-a")).unwrap();
        let old_path = dir.join("projects/proj-a/55555555-5555-4555-8555-555555555555.jsonl");
        let new_path = dir.join("projects/proj-a/66666666-6666-4666-8666-666666666666.jsonl");
        std::fs::write(&old_path, "{}").unwrap();
        std::fs::write(&new_path, "{}").unwrap();
        filetime_set(&old_path, 0);
        filetime_set(&new_path, 1);

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 1,
            tool: ToolScope::Claude,
            allow_running: false,
        };
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(&dir, clear, &policy, &process, &clock, trust);
        let actions = build_actions(std::slice::from_ref(&resolution));
        let delete_targets: Vec<_> = actions
            .iter()
            .filter(|a| a.action_class == ActionClass::Delete)
            .flat_map(|a| a.target_artifact_ids.iter())
            .cloned()
            .collect();
        assert_eq!(
            delete_targets.len(),
            1,
            "exactly one of the two sessions must be a delete candidate: {actions:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn codex_keep_latest_protects_a_whole_subagent_tree_even_when_the_root_looks_old() {
        let dir = tree(Path::new(""), "codex-tree");
        let root_path = dir.join("sessions/rollout-33333333-3333-4333-8333-333333333333.jsonl");
        let child_path = dir.join("sessions/rollout-33333333-3333-4333-8333-333333333334.jsonl");
        std::fs::create_dir_all(root_path.parent().unwrap()).unwrap();
        std::fs::write(
            &root_path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "session_meta",
                    "payload": {"meta": {"id": "33333333-3333-4333-8333-333333333333"}}
                })
            ),
        )
        .unwrap();
        std::fs::write(
            &child_path,
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "session_meta",
                    "payload": {"meta": {
                        "id": "33333333-3333-4333-8333-333333333334",
                        "parent_thread_id": "33333333-3333-4333-8333-333333333333"
                    }}
                })
            ),
        )
        .unwrap();
        // The root file itself looks old, but its child was touched recently - the whole tree
        // must stay protected (`cancellai.py::choose_codex_old_sessions`'s own rule).
        filetime_set(&root_path, 0);
        filetime_set(&child_path, 9 * 86_400);

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 1,
            tool: ToolScope::Codex,
            allow_running: false,
        };
        let fs = SystemFsObserver;
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_codex(&dir, clear, &policy, &fs, &process, &clock, trust);
        let actions = build_actions(std::slice::from_ref(&resolution));
        assert!(
            actions
                .iter()
                .all(|a| a.action_class == ActionClass::Observe),
            "a recently-touched child must protect its whole tree from deletion: {actions:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn a_degraded_companion_withholds_every_action_for_the_whole_tool_not_only_its_own_session() {
        use std::os::unix::fs::PermissionsExt;

        // Mirrors `tests/fixtures/recipes.py::build_claude_partial_tree`: two ordinary, fully
        // readable sessions plus a third whose companion payload directory cannot be listed.
        // E06-S02's differential parity gate found this scenario diverging from
        // `claude-partial-tree`'s committed characterization (Python withholds the whole tool;
        // an earlier version of this function only downgraded the one degraded session).
        let dir = tree(Path::new(""), "degraded-companion");
        let project = dir.join("projects/proj-c");
        std::fs::create_dir_all(&project).unwrap();
        let ok_a = project.join("11111111-1111-4111-8111-111111111111.jsonl");
        let ok_b = project.join("22222222-2222-4222-8222-222222222222.jsonl");
        std::fs::write(&ok_a, "{}").unwrap();
        std::fs::write(&ok_b, "{}").unwrap();
        filetime_set(&ok_a, 0);
        filetime_set(&ok_b, 0);

        let degraded_id = "33333333-3333-4333-8333-333333333333";
        let degraded_session = project.join(format!("{degraded_id}.jsonl"));
        std::fs::write(&degraded_session, "{}").unwrap();
        filetime_set(&degraded_session, 0);
        let companion = project.join(degraded_id);
        std::fs::create_dir_all(companion.join("tool-results")).unwrap();
        std::fs::write(companion.join("tool-results/large.txt"), "x").unwrap();
        std::fs::set_permissions(&companion, std::fs::Permissions::from_mode(0o000)).unwrap();

        let policy = RetentionPolicy {
            days: 7,
            keep_latest: 0,
            tool: ToolScope::Claude,
            allow_running: false,
        };
        let process = SyntheticProcessObserver::complete(Vec::<String>::new());
        let clock = frozen_now();
        let trust = crate::trust::builtin_provider_trust();

        let resolution = resolve_claude(&dir, clear, &policy, &process, &clock, trust);

        std::fs::set_permissions(&companion, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            !resolution.scan_complete,
            "a companion directory that could not be listed must mark the whole scan incomplete"
        );
        assert!(
            resolution
                .artifacts
                .iter()
                .all(|c| c.artifact.knowledge_confidence == KnowledgeConfidence::LowUnknown),
            "JSON_CONTRACTS.md: every artifact from a PARTIAL/UNKNOWN scope must report \
             knowledge_confidence no higher than LOW/UNKNOWN, including the two ordinary \
             sessions whose own evidence was perfectly readable: {:?}",
            resolution
                .artifacts
                .iter()
                .map(|c| c.artifact.knowledge_confidence)
                .collect::<Vec<_>>()
        );
        let actions = build_actions(std::slice::from_ref(&resolution));
        assert!(
            actions
                .iter()
                .all(|a| a.action_class == ActionClass::Observe),
            "every action for the tool must be withheld, including the two ordinary sessions \
             whose own evidence was perfectly readable: {actions:?}"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
