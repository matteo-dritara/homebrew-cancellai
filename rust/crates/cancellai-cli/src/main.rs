//! Headless/scriptable CLI client (`docs/architecture/TARGET.md` - experience plane).
//!
//! Forbidden dependency direction: this crate may not access raw provider roots for
//! mutation directly - all mutation is routed through `cancellai-safety` (SI-019). It
//! specifically may never construct `cancellai_platform::mutation::SystemMutationExecutor`
//! or call `.mutate(` itself - `scripts/check_mutation_boundary.py` enforces this
//! structurally; the one production entry point this crate uses for a real mutation is
//! `cancellai_safety::execute_with_system_capabilities` (E06-S01 addition to the safety
//! kernel, `cancellai-safety/src/mutation_executor.rs`).
//!
//! E06-S01 adds the first real command surface: `status` (default, read-only),
//! `inspect` (read-only detail), `plan` (read-only, produces a `docs/architecture/
//! JSON_CONTRACTS.md` plan document), `clean` (the only mutating command - `--dry-run`
//! previews, otherwise requires `--yes` or an interactive confirmation; no flag or missing
//! subcommand ever implies `clean`, per SI-007), `configure` (Claude Code's own
//! `cleanupPeriodDays` retention setting), and `version`.

mod documents;
mod roots;
mod timestamp;

use std::collections::{BTreeSet, HashMap};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use cancellai_model::{
    Action, ActionClass, ArtifactId, AuthorityLevel, ErrorCategory, KnowledgeConfidence,
    Reversibility, RootFingerprint,
};
use cancellai_platform::{
    Clock, SystemClock, SystemIdentityObserver, SystemPathResolver, SystemProcessObserver,
};
use cancellai_policy::{
    ClassifiedArtifact, ProviderResolution, RetentionPolicy, ToolScope, build_actions,
    builtin_provider_trust, resolve_claude, resolve_codex,
};
use cancellai_provider_api::{RootConfidence, RootOrigin};
use cancellai_provider_claude::ClaudeProvider;
use cancellai_provider_codex::CodexProvider;
use cancellai_safety::{ApprovedRoot, SealedPlan, execute_with_system_capabilities};
use documents::{ActionResultDoc, ProviderRootDoc, ScanCompletenessDoc};

/// Provider process names each provider's root-authority process guard must confirm are not
/// running immediately before a real deletion (`cancellai-safety::mutation_executor::execute`'s
/// own TOCTOU re-check, E06 verifier review round 1 - see that module's docs).
const CLAUDE_PROCESS_NAMES: &[&str] = &["claude"];
const CODEX_PROCESS_NAMES: &[&str] = &["codex", "Codex"];

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COMMANDS: &[&str] = &["status", "inspect", "plan", "clean", "configure", "version"];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(run(&args));
}

fn run(args: &[String]) -> i32 {
    let (command, rest) = match split_command(args) {
        Ok(v) => v,
        Err(message) => return invalid_input(&message),
    };
    match command.as_str() {
        "status" => cmd_read_only(rest, RunMode::Status),
        "inspect" => cmd_read_only(rest, RunMode::Inspect),
        "plan" => cmd_read_only(rest, RunMode::Plan),
        "clean" => cmd_clean(rest),
        "configure" => cmd_configure(rest),
        "version" => cmd_version(rest),
        _ => unreachable!("split_command only returns a name from COMMANDS or \"status\""),
    }
}

/// No subcommand, or a leading flag with no subcommand, always means `status` - the read-only
/// default (SI-007: ambiguity never escalates toward mutation). An unrecognized leading token
/// that is not a flag is refused outright rather than guessed at.
fn split_command(args: &[String]) -> Result<(String, &[String]), String> {
    match args.first() {
        None => Ok(("status".to_string(), &args[0..0])),
        Some(first) if COMMANDS.contains(&first.as_str()) => Ok((first.clone(), &args[1..])),
        Some(first) if first.starts_with('-') => Ok(("status".to_string(), args)),
        Some(other) => Err(format!(
            "unrecognized command '{other}' - expected one of {COMMANDS:?}, or a flag for the default 'status' command"
        )),
    }
}

fn invalid_input(message: &str) -> i32 {
    eprintln!("[{}] {message}", ErrorCategory::InvalidInput.code());
    ErrorCategory::InvalidInput.exit_code()
}

#[derive(Debug, Clone)]
struct CommonFlags {
    days: u32,
    keep_latest: u32,
    tool: ToolScope,
    json: bool,
    allow_running: bool,
    dry_run: bool,
    yes: bool,
}

/// Parses every flag this CLI recognizes across all commands. Flags irrelevant to a given
/// command (e.g. `--dry-run` on `status`) are accepted but have no effect - `status`/`inspect`/
/// `plan` are categorically incapable of mutation regardless, so accepting an unused token
/// there cannot itself escalate authority (SI-007's concern is never-guess-toward-mutation, not
/// strict per-command flag hygiene). An unrecognized flag, or a value-taking flag with a
/// missing/malformed value, is always refused.
fn parse_flags(args: &[String]) -> Result<CommonFlags, String> {
    let mut flags = CommonFlags {
        days: 7,
        keep_latest: 2,
        tool: ToolScope::All,
        json: false,
        allow_running: false,
        dry_run: false,
        yes: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--days" => {
                i += 1;
                flags.days = parse_u32(args.get(i), "--days")?;
            }
            "--keep-latest" => {
                i += 1;
                flags.keep_latest = parse_u32(args.get(i), "--keep-latest")?;
            }
            "--tool" => {
                i += 1;
                flags.tool = match args.get(i).map(String::as_str) {
                    Some("all") => ToolScope::All,
                    Some("codex") => ToolScope::Codex,
                    Some("claude") => ToolScope::Claude,
                    other => {
                        return Err(format!(
                            "--tool expects one of all|codex|claude, got {other:?}"
                        ));
                    }
                };
            }
            "--json" => flags.json = true,
            "--allow-running" => flags.allow_running = true,
            "--dry-run" => flags.dry_run = true,
            "--yes" | "-y" => flags.yes = true,
            other => return Err(format!("unrecognized flag '{other}'")),
        }
        i += 1;
    }
    Ok(flags)
}

fn parse_u32(value: Option<&String>, flag: &str) -> Result<u32, String> {
    value
        .ok_or_else(|| format!("{flag} requires a value"))?
        .parse::<u32>()
        .map_err(|_| format!("{flag} expects a non-negative integer"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Status,
    Inspect,
    Plan,
}

/// Every provider's classified inventory, resolved once and reused by whichever read-only or
/// mutating command called this - `docs/architecture/TARGET.md`'s OBSERVE+CLASSIFY+RESOLVE
/// stages, shared by every command surface so `status`/`inspect`/`plan`/`clean` never
/// re-derive it differently from one another.
struct Resolved {
    resolutions: Vec<ProviderResolution>,
    claude_root: PathBuf,
    codex_root: PathBuf,
    /// The real, independently-derived fingerprint for each root - computed exactly once here
    /// and reused by every caller (`provider_root_docs`, the root-authority gate in
    /// `execute_clean`/`delete_one`), never recomputed with a hard-coded `is_default_root`
    /// (E06 verifier review round 1: two separate call sites each independently hard-coded
    /// `true`, so a `CLAUDE_CONFIG_DIR`/`CODEX_HOME` override was always reported - and
    /// therefore always mutation-eligible - as the default root regardless of where it actually
    /// pointed).
    claude_fingerprint: cancellai_provider_api::RootFingerprint,
    codex_fingerprint: cancellai_provider_api::RootFingerprint,
}

/// Resolve every provider's root, classify its inventory, and fingerprint its root authority.
/// `Err` when a root cannot be positively resolved at all (no override and no usable `$HOME`) -
/// a caller must refuse rather than guess a fallback (`roots::claude_home`/`codex_home`'s own
/// docs: an earlier version of this function silently fell back to `"."`, the current working
/// directory, which is not a positively-identified provider root).
fn resolve_all(flags: &CommonFlags) -> Result<Resolved, String> {
    let claude_resolved = roots::claude_home().ok_or_else(|| {
        "could not resolve the Claude Code home directory ($HOME is not set and \
         $CLAUDE_CONFIG_DIR is not set)"
            .to_string()
    })?;
    let codex_resolved = roots::codex_home().ok_or_else(|| {
        "could not resolve the Codex CLI home directory ($HOME is not set and $CODEX_HOME is \
         not set)"
            .to_string()
    })?;
    let claude_root = claude_resolved.path;
    let codex_root = codex_resolved.path;
    let policy = RetentionPolicy {
        days: flags.days,
        keep_latest: flags.keep_latest,
        tool: flags.tool,
        allow_running: flags.allow_running,
    };
    let process = SystemProcessObserver;
    let fs = cancellai_platform::SystemFsObserver;
    let clock = SystemClock;
    let trust = builtin_provider_trust();

    let claude_provider = ClaudeProvider::new(&claude_root, claude_resolved.is_default);
    let codex_provider = CodexProvider::new(&codex_root, codex_resolved.is_default);
    let claude_fingerprint = claude_provider.fingerprint();
    let codex_fingerprint = codex_provider.fingerprint();

    let claude_resolution = resolve_claude(
        &claude_root,
        |p: &Path| claude_provider.protection(p),
        &policy,
        &process,
        &clock,
        trust,
    );
    let codex_resolution = resolve_codex(
        &codex_root,
        |p: &Path| codex_provider.protection(p),
        &policy,
        &fs,
        &process,
        &clock,
        trust,
    );

    Ok(Resolved {
        resolutions: vec![claude_resolution, codex_resolution],
        claude_root,
        codex_root,
        claude_fingerprint,
        codex_fingerprint,
    })
}

fn provider_root_docs(resolved: &Resolved) -> Vec<ProviderRootDoc> {
    vec![
        ProviderRootDoc::new(
            "root-claude-code".to_string(),
            "claude-code".to_string(),
            resolved.claude_fingerprint.origin,
            resolved.claude_fingerprint.confidence,
        ),
        ProviderRootDoc::new(
            "root-codex-cli".to_string(),
            "codex-cli".to_string(),
            resolved.codex_fingerprint.origin,
            resolved.codex_fingerprint.confidence,
        ),
    ]
}

/// The provider id (matching `cancellai_model::AgentArtifact::provider_id`) that owns a given
/// root fingerprint - `"codex-cli"`/`"claude-code"`, used to key process-guard names and to look
/// an artifact's root fingerprint up by its `provider_id`.
fn root_fingerprint_for<'a>(
    resolved: &'a Resolved,
    provider_id: &str,
) -> &'a cancellai_provider_api::RootFingerprint {
    if provider_id == "codex-cli" {
        &resolved.codex_fingerprint
    } else {
        &resolved.claude_fingerprint
    }
}

/// `ArtifactId -> provider_id`, built once and reused wherever a caller needs to know which
/// provider's root authority governs a given action's target (root-authority withholding,
/// `execute_clean`'s per-action provider dispatch).
fn provider_id_by_target(resolved: &Resolved) -> HashMap<ArtifactId, &'static str> {
    resolved
        .resolutions
        .iter()
        .flat_map(|r| {
            r.artifacts
                .iter()
                .map(move |c| (c.artifact.artifact_id.clone(), r.provider_id))
        })
        .collect()
}

/// ADR-0013: only the provider's own default root may be mutated - a custom root, however
/// convincing its structural markers look, is inspection-only (SI-002: structural evidence is
/// cheap to fabricate and is therefore never proof of ownership). This downgrades every
/// `Delete` action whose target's provider root is not `Default` to an `Observe`, explaining
/// why, and returns the set of providers this withheld - `plan`/`clean` both call this so a
/// preview always matches what a real run would actually do (E06 verifier review round 1: an
/// earlier version had no such gate at all; a stale session under a `CLAUDE_CONFIG_DIR` custom
/// root was reported `origin=default` by a separate bug and then genuinely deleted).
fn withhold_for_root_authority(
    mut actions: Vec<Action>,
    resolved: &Resolved,
    provider_by_target: &HashMap<ArtifactId, &'static str>,
) -> (Vec<Action>, BTreeSet<&'static str>) {
    let mut withheld = BTreeSet::new();
    for action in &mut actions {
        if action.action_class != ActionClass::Delete {
            continue;
        }
        let Some(target) = action.target_artifact_ids.first() else {
            continue;
        };
        let Some(&provider_id) = provider_by_target.get(target) else {
            continue;
        };
        let fingerprint = root_fingerprint_for(resolved, provider_id);
        if fingerprint.origin != RootOrigin::Default {
            withheld.insert(provider_id);
            action.action_class = ActionClass::Observe;
            action.authority = AuthorityLevel::Observe;
            action.reversibility = Reversibility::Rebuildable;
            action.execution_preconditions.clear();
            action.reason = format!(
                "refusing destructive work on this {provider_id} root: it is not the default \
                 root (origin={origin:?}, confidence={confidence:?}); looking right structurally \
                 is not proof of ownership (SI-002), so no deletion is proposed here - unset the \
                 configured override to clean the default root",
                origin = fingerprint.origin,
                confidence = fingerprint.confidence,
            );
        }
    }
    (actions, withheld)
}

fn scan_completeness_docs(resolved: &Resolved) -> Vec<ScanCompletenessDoc> {
    resolved
        .resolutions
        .iter()
        .map(|r| ScanCompletenessDoc {
            scope: r.provider_id,
            complete: r.scan_complete,
            error_count: u32::from(!r.scan_complete),
        })
        .collect()
}

fn any_incomplete(resolved: &Resolved) -> bool {
    resolved.resolutions.iter().any(|r| !r.scan_complete)
}

fn cmd_read_only(args: &[String], mode: RunMode) -> i32 {
    let flags = match parse_flags(args) {
        Ok(f) => f,
        Err(e) => return invalid_input(&e),
    };
    let resolved = match resolve_all(&flags) {
        Ok(r) => r,
        Err(e) => return invalid_input(&e),
    };
    let now = SystemClock.now();
    let mut withheld_by_root_authority = BTreeSet::new();

    match mode {
        RunMode::Status | RunMode::Inspect => {
            let artifacts: Vec<_> = resolved
                .resolutions
                .iter()
                .flat_map(|r| r.artifacts.iter())
                .map(|c| c.artifact.clone())
                .collect();
            if flags.json || mode == RunMode::Inspect {
                let doc = documents::inventory_document(
                    "inventory-1".to_string(),
                    now,
                    provider_root_docs(&resolved),
                    scan_completeness_docs(&resolved),
                    artifacts,
                );
                println!("{}", serde_json::to_string_pretty(&doc).unwrap());
            } else {
                print_status_summary(&resolved);
            }
        }
        RunMode::Plan => {
            // `plan` previews what a real `clean` would do, so it must apply the same
            // root-authority withholding `clean` does (SI-007: a preview that disagrees with
            // the real run is itself a safety defect) - see `withhold_for_root_authority`'s docs.
            let provider_by_target = provider_id_by_target(&resolved);
            let (actions, withheld) = withhold_for_root_authority(
                build_actions(&resolved.resolutions),
                &resolved,
                &provider_by_target,
            );
            withheld_by_root_authority = withheld;
            if flags.json {
                let doc = documents::plan_document(
                    "plan-1".to_string(),
                    "inventory-1".to_string(),
                    now,
                    provider_root_docs(&resolved),
                    actions,
                    Vec::new(),
                );
                println!("{}", serde_json::to_string_pretty(&doc).unwrap());
            } else {
                print_plan_summary(&actions);
            }
        }
    }

    if any_incomplete(&resolved) {
        eprintln!(
            "[{}] one or more provider scans were incomplete; no conclusion about missing data was assumed",
            ErrorCategory::IncompleteInventory.code()
        );
        ErrorCategory::IncompleteInventory.exit_code()
    } else if !withheld_by_root_authority.is_empty() {
        eprintln!(
            "[{}] destructive work was withheld for: {} (not the default root)",
            ErrorCategory::SafetyBlock.code(),
            withheld_by_root_authority
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        );
        ErrorCategory::SafetyBlock.exit_code()
    } else {
        0
    }
}

fn print_status_summary(resolved: &Resolved) {
    for resolution in &resolved.resolutions {
        let total_bytes: u64 = resolution.artifacts.iter().map(|c| c.size_bytes).sum();
        println!(
            "{}: {} artifact(s), {} bytes, scan_complete={}",
            resolution.provider_id,
            resolution.artifacts.len(),
            total_bytes,
            resolution.scan_complete
        );
    }
}

fn print_plan_summary(actions: &[Action]) {
    let delete_count = actions
        .iter()
        .filter(|a| a.action_class == ActionClass::Delete)
        .count();
    println!(
        "{} action(s) proposed: {} delete candidate(s), {} observation(s)",
        actions.len(),
        delete_count,
        actions.len() - delete_count
    );
    for action in actions {
        println!(
            "  [{:?}] {:?}: {}",
            action.action_class, action.target_artifact_ids, action.reason
        );
    }
}

fn cmd_clean(args: &[String]) -> i32 {
    let flags = match parse_flags(args) {
        Ok(f) => f,
        Err(e) => return invalid_input(&e),
    };
    // Automation safety: `--json` requesting a mutating run must state its intent explicitly
    // (`--yes` or `--dry-run`) - never inferred, matching `cancellai.py`'s own gate.
    if flags.json && !flags.yes && !flags.dry_run {
        return invalid_input(
            "clean --json requires --yes or --dry-run: a machine-readable destructive run must state its intent explicitly",
        );
    }

    let resolved = match resolve_all(&flags) {
        Ok(r) => r,
        Err(e) => return invalid_input(&e),
    };
    let provider_by_target = provider_id_by_target(&resolved);
    let (actions, withheld) = withhold_for_root_authority(
        build_actions(&resolved.resolutions),
        &resolved,
        &provider_by_target,
    );
    // SI-008/SI-009/SI-002: absence-of-evidence and absence-of-ownership both withhold real
    // work, and that must be visible in the exit code every time this command can report it -
    // including `--dry-run` and the "nothing to clean" short-circuit, not only a real run (E06
    // verifier review round 1: an earlier version always exited 0 on those two paths regardless
    // of whether something was actually withheld).
    let safety_withheld = any_incomplete(&resolved) || !withheld.is_empty();
    let delete_count = actions
        .iter()
        .filter(|a| a.action_class == ActionClass::Delete)
        .count();

    if delete_count == 0 {
        println!(
            "{}",
            if safety_withheld {
                "Nothing was cleaned: safety withheld the requested work."
            } else {
                "Nothing to clean: no artifact is both stale and unblocked."
            }
        );
        return if safety_withheld {
            ErrorCategory::SafetyBlock.exit_code()
        } else {
            0
        };
    }

    if flags.dry_run {
        println!("Dry-run only. No files were changed.");
        print_plan_summary(&actions);
        return if safety_withheld {
            ErrorCategory::SafetyBlock.exit_code()
        } else {
            0
        };
    }

    if !flags.yes {
        print_plan_summary(&actions);
        print!("Proceed with deleting {delete_count} artifact(s)? [y/N] ");
        std::io::stdout().flush().ok();
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err()
            || !matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
        {
            println!("Cancelled. No files were changed.");
            return 1;
        }
    }

    let exit_code = execute_clean(&resolved, &actions, flags.json, flags.allow_running);
    if exit_code == 0 && safety_withheld {
        ErrorCategory::SafetyBlock.exit_code()
    } else {
        exit_code
    }
}

/// [`ApprovedRoot::establish`] alone verifies that `path` resolves to *some* real, identifiable
/// object (SI-002) - it has no notion of "default vs custom" and correctly does not refuse a
/// symlinked custom root (a custom root is already refused elsewhere by origin, symlinked or
/// not). This wrapper adds the one thing `establish` cannot know on its own: a root this run
/// classified as `Default` must, immediately before establishing it for real mutation, be
/// re-confirmed to not be a symlink/reparse point at all (E06 verifier review round 2 - `roots
/// ::is_symlink`'s own docs explain why authority must never rest on the lexical `$HOME/.claude`
/// name alone). `fingerprint` is the classification already computed for this run;
/// [`roots::is_symlink`] itself is evaluated fresh here, not read from that cached value.
fn establish_verified_root(
    path: &Path,
    fingerprint: &cancellai_provider_api::RootFingerprint,
    resolver: &SystemPathResolver,
    observer: &SystemIdentityObserver,
) -> Result<ApprovedRoot, cancellai_safety::BoundaryError> {
    if fingerprint.origin == RootOrigin::Default {
        if roots::is_symlink(path) {
            return Err(cancellai_safety::BoundaryError::RootIdentityUnavailable(
                format!(
                    "{} is a symlink; a default-named root must be a real directory, not a \
                 link, to carry destructive authority (SI-002/ADR-0013)",
                    path.display()
                ),
            ));
        }
        // E07-S09 round-1 independent verifier review: the leaf-only check above missed a
        // default root reached through an *intermediate* symlink (e.g. `$HOME` itself being a
        // link to a real, non-symlink `.claude` leaf) - `ApprovedRoot::establish` below
        // canonicalizes `path`, which silently resolves through exactly that. `SealedRoot`'s
        // handle-relative walk (already built for `configure`, `cancellai-sealedfs`, ADR-0017)
        // is reused here purely to prove no component of `path` - not only the leaf - is a
        // link, before `establish` gets a chance to canonicalize through one.
        if let Err(e) = cancellai_sealedfs::verify_no_intermediate_links(path) {
            return Err(cancellai_safety::BoundaryError::RootIdentityUnavailable(
                format!(
                    "{} is not a safely-establishable default root: {e} \
                 (SI-002/SI-003/SI-013/ADR-0013)",
                    path.display()
                ),
            ));
        }
    }
    ApprovedRoot::establish(path, resolver, observer)
}

fn execute_clean(resolved: &Resolved, actions: &[Action], json: bool, allow_running: bool) -> i32 {
    let by_id: HashMap<ArtifactId, &ClassifiedArtifact> = resolved
        .resolutions
        .iter()
        .flat_map(|r| r.artifacts.iter())
        .map(|c| (c.artifact.artifact_id.clone(), c))
        .collect();

    let resolver = SystemPathResolver;
    let observer = SystemIdentityObserver;
    // Fresh, execution-time symlink re-check (E06 verifier review round 2), independent of
    // `resolved`'s cached fingerprint - `resolved` was computed at the top of `cmd_clean`,
    // before the interactive confirmation prompt a `clean --yes`-less run waits on; a root
    // named `$HOME/.claude` could be swapped for a symlink during that pause. See
    // `establish_verified_root`'s own docs.
    let claude_approved_root = establish_verified_root(
        &resolved.claude_root,
        &resolved.claude_fingerprint,
        &resolver,
        &observer,
    );
    let codex_approved_root = establish_verified_root(
        &resolved.codex_root,
        &resolved.codex_fingerprint,
        &resolver,
        &observer,
    );

    let mut results = Vec::new();
    let mut any_failed = false;
    let mut any_blocked = false;
    let mut reclaimed_bytes = 0u64;

    for action in actions {
        let target_id = &action.target_artifact_ids[0];
        let doc = match action.action_class {
            ActionClass::Delete => match by_id.get(target_id) {
                None => {
                    any_failed = true;
                    ActionResultDoc {
                        action_id: action.action_id.0.clone(),
                        status: "failed",
                        reason_code: "INTERNAL_FAULT: plan referenced an artifact not present in \
                                      this run's own inventory"
                            .to_string(),
                        reclaimed_bytes: 0,
                        post_action_state: "hot",
                    }
                }
                Some(classified) => {
                    let (approved_root, provider_id, process_names) =
                        if classified.artifact.provider_id == "codex-cli" {
                            (&codex_approved_root, "codex-cli", CODEX_PROCESS_NAMES)
                        } else {
                            (&claude_approved_root, "claude-code", CLAUDE_PROCESS_NAMES)
                        };
                    // `--allow-running` must apply consistently to both the plan-build-time
                    // liveness check (`cancellai-policy::retention`) and this execution-time
                    // re-check (`cancellai-safety::mutation_executor`'s `process_guard`) - an
                    // explicit, SI-007-compliant override the operator stated once must not be
                    // silently overridden by a *second*, un-opt-out-able check at delete time.
                    let process_guard = if allow_running {
                        None
                    } else {
                        Some(process_names)
                    };
                    delete_one(
                        approved_root,
                        &resolver,
                        &observer,
                        classified,
                        provider_id,
                        root_fingerprint_for(resolved, provider_id),
                        process_guard,
                        action,
                        &mut any_failed,
                        &mut any_blocked,
                        &mut reclaimed_bytes,
                    )
                }
            },
            ActionClass::Observe | ActionClass::Quarantine | ActionClass::Archive => {
                ActionResultDoc {
                    action_id: action.action_id.0.clone(),
                    status: "safely_skipped",
                    reason_code: "NOT_ELIGIBLE".to_string(),
                    reclaimed_bytes: 0,
                    post_action_state: "hot",
                }
            }
        };
        results.push(doc);
    }

    let now = SystemClock.now();
    if json {
        let doc = documents::result_document("plan-1".to_string(), now, results);
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    } else {
        let succeeded = results.iter().filter(|r| r.status == "succeeded").count();
        println!("{succeeded} artifact(s) deleted, {reclaimed_bytes} bytes reclaimed.");
    }

    if any_failed {
        ErrorCategory::MutationFailure.exit_code()
    } else if any_blocked {
        ErrorCategory::SafetyBlock.exit_code()
    } else {
        0
    }
}

/// Maps a root's structural [`RootConfidence`] onto the [`KnowledgeConfidence`] vocabulary a
/// [`SealedPlan`] carries - the same mapping `ClaudeProvider`/`CodexProvider`'s own
/// `capability()` already use for `CapabilityKind::FingerprintRoot`. This does not itself grant
/// or withhold mutation eligibility (`cancellai_model::RootFingerprint`'s own docs: "nothing in
/// E03-S02 grants mutation eligibility from a fingerprint alone") - that gate is
/// `withhold_for_root_authority`'s origin check, applied before any action reaches this
/// function. This only makes the sealed plan's own record honest instead of a hard-coded
/// `Verified` regardless of what was actually observed (E06 verifier review round 1).
fn knowledge_confidence_from_root(confidence: RootConfidence) -> KnowledgeConfidence {
    match confidence {
        RootConfidence::Default => KnowledgeConfidence::Verified,
        RootConfidence::High => KnowledgeConfidence::Observed,
        RootConfidence::Low => KnowledgeConfidence::Inferred,
        RootConfidence::Unknown => KnowledgeConfidence::LowUnknown,
    }
}

#[allow(clippy::too_many_arguments)]
fn delete_one(
    approved_root: &Result<ApprovedRoot, cancellai_safety::BoundaryError>,
    resolver: &SystemPathResolver,
    observer: &SystemIdentityObserver,
    classified: &ClassifiedArtifact,
    provider_id: &'static str,
    root_fp: &cancellai_provider_api::RootFingerprint,
    process_guard: Option<&'static [&'static str]>,
    action: &Action,
    any_failed: &mut bool,
    any_blocked: &mut bool,
    reclaimed_bytes: &mut u64,
) -> ActionResultDoc {
    // Independent, execution-time re-check (defense in depth alongside
    // `withhold_for_root_authority`'s plan-time gate, E06 verifier review round 1's explicit
    // ask: "independently refuse mutation of custom/unverified roots at execution"). This is
    // the *only* place a real, filesystem-mutating deletion is requested, so it must never
    // trust an upstream decision alone.
    if root_fp.origin != RootOrigin::Default {
        *any_blocked = true;
        return ActionResultDoc {
            action_id: action.action_id.0.clone(),
            status: "safely_skipped",
            reason_code: format!(
                "ROOT_AUTHORITY_DENIED: {provider_id} root is not the default root \
                 (origin={:?}, confidence={:?}); refusing destructive work immediately before \
                 mutation",
                root_fp.origin, root_fp.confidence
            ),
            reclaimed_bytes: 0,
            post_action_state: "hot",
        };
    }
    let root = match approved_root {
        Ok(root) => root,
        Err(e) => {
            *any_blocked = true;
            return ActionResultDoc {
                action_id: action.action_id.0.clone(),
                status: "safely_skipped",
                reason_code: format!("ROOT_UNAVAILABLE: {e:?}"),
                reclaimed_bytes: 0,
                post_action_state: "hot",
            };
        }
    };
    let bound = match root.bind(&classified.path, resolver, observer) {
        Ok(b) => b,
        Err(e) => {
            *any_blocked = true;
            return ActionResultDoc {
                action_id: action.action_id.0.clone(),
                status: "safely_skipped",
                reason_code: format!("STALE_PLAN: {e:?}"),
                reclaimed_bytes: 0,
                post_action_state: "hot",
            };
        }
    };
    let root_fingerprint = RootFingerprint {
        root_id: format!("root-{provider_id}"),
        provider_id: provider_id.to_string(),
        confidence: knowledge_confidence_from_root(root_fp.confidence),
    };
    let sealed = SealedPlan::seal_with_process_guard(
        root,
        root_fingerprint,
        &bound,
        ActionClass::Delete,
        AuthorityLevel::Govern,
        Reversibility::Irreversible,
        process_guard,
    );
    match execute_with_system_capabilities(&sealed, &bound) {
        cancellai_safety::ActionResult::Succeeded => {
            *reclaimed_bytes += classified.size_bytes;
            ActionResultDoc {
                action_id: action.action_id.0.clone(),
                status: "succeeded",
                reason_code: "OK".to_string(),
                reclaimed_bytes: classified.size_bytes,
                post_action_state: "purged",
            }
        }
        cancellai_safety::ActionResult::SafelyBlocked { reason } => {
            *any_blocked = true;
            ActionResultDoc {
                action_id: action.action_id.0.clone(),
                status: "safely_skipped",
                reason_code: reason,
                reclaimed_bytes: 0,
                post_action_state: "hot",
            }
        }
        cancellai_safety::ActionResult::Failed { reason } => {
            *any_failed = true;
            ActionResultDoc {
                action_id: action.action_id.0.clone(),
                status: "failed",
                reason_code: reason,
                reclaimed_bytes: 0,
                post_action_state: "hot",
            }
        }
    }
}

fn cmd_configure(args: &[String]) -> i32 {
    let mut retention_days: Option<u32> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--claude-retention" => {
                i += 1;
                match parse_u32(args.get(i), "--claude-retention") {
                    Ok(v) if v >= 1 => retention_days = Some(v),
                    Ok(_) => return invalid_input("--claude-retention must be at least 1"),
                    Err(e) => return invalid_input(&e),
                }
            }
            other => return invalid_input(&format!("unrecognized flag '{other}'")),
        }
        i += 1;
    }
    let Some(days) = retention_days else {
        return invalid_input("configure requires --claude-retention DAYS");
    };

    let claude_resolved = match roots::claude_home() {
        Some(r) => r,
        None => {
            return invalid_input(
                "could not resolve the Claude Code home directory ($HOME is not set)",
            );
        }
    };
    // `configure` is a real provider-configuration mutation (SI-019), so it uses the same root
    // boundary `clean` does: only the default Claude root may be written to (ADR-0013) - a
    // custom root, however plausible it looks, is refused (E06 verifier review round 1).
    let claude_provider = ClaudeProvider::new(&claude_resolved.path, claude_resolved.is_default);
    let fingerprint = claude_provider.fingerprint();
    if fingerprint.origin != RootOrigin::Default {
        return safety_block(&format!(
            "refusing to configure {}: it is not the default Claude Code root \
             (origin={:?}, confidence={:?}); looking right structurally is not proof of \
             ownership - unset $CLAUDE_CONFIG_DIR to configure the default root",
            claude_resolved.path.display(),
            fingerprint.origin,
            fingerprint.confidence,
        ));
    }
    // Fresh, execution-time re-check (E06 verifier review round 2), independent of
    // `claude_resolved.is_default` above - see `establish_verified_root`'s identical rationale.
    // This is a fast, legible diagnostic only: it does not by itself close the TOCTOU between
    // this check and the write below (E07-S07 round-1 verifier rejection found exactly that
    // gap - a root swapped to a symlink *after* this check and before the raw path-based write
    // operations that used to follow it). `configure_claude_retention`'s `SealedRoot` is the
    // capability that actually closes it, unconditionally, regardless of what this check saw.
    if roots::is_symlink(&claude_resolved.path) {
        return safety_block(&format!(
            "refusing to configure {}: it is a symlink, not a real directory - a default-named \
             root must not carry destructive/configuration authority through a link (SI-002/ADR-0013)",
            claude_resolved.path.display(),
        ));
    }
    match configure_claude_retention(&claude_resolved.path, days) {
        Ok(()) => {
            println!("Set Claude Code cleanupPeriodDays to {days}.");
            0
        }
        Err(ConfigureError::MalformedSettings(msg)) => {
            safety_block(&format!("refusing to modify invalid settings.json: {msg}"))
        }
        Err(ConfigureError::Sealed(e)) => safety_block(&format!(
            "refusing to configure {}: {e} (SI-002/SI-003/SI-013)",
            claude_resolved.path.display(),
        )),
        Err(ConfigureError::Io(e)) => {
            eprintln!("[{}] {e}", ErrorCategory::InternalFault.code());
            ErrorCategory::InternalFault.exit_code()
        }
    }
}

fn safety_block(message: &str) -> i32 {
    eprintln!("[{}] {message}", ErrorCategory::SafetyBlock.code());
    ErrorCategory::SafetyBlock.exit_code()
}

enum ConfigureError {
    /// `settings.json` exists but is not valid JSON, or its root is not a JSON object - refused
    /// outright rather than silently discarded and replaced (E06 verifier review round 1: an
    /// earlier version treated a parse failure exactly like a missing file, silently overwriting
    /// whatever the operator had there with a fresh `{}`).
    MalformedSettings(String),
    /// A `cancellai_sealedfs::SealError` other than a bare I/O failure - the root turned out to
    /// be a link/reparse point, not a directory, or carried an invalid child name. Kept
    /// separate from `Io` so the caller reports it as a safety block (SI-002/SI-003/SI-013),
    /// not `InternalFault`.
    Sealed(cancellai_sealedfs::SealError),
    Io(std::io::Error),
}

impl From<std::io::Error> for ConfigureError {
    fn from(e: std::io::Error) -> Self {
        ConfigureError::Io(e)
    }
}

impl From<cancellai_sealedfs::SealError> for ConfigureError {
    fn from(e: cancellai_sealedfs::SealError) -> Self {
        match e {
            cancellai_sealedfs::SealError::Io(io_err) => ConfigureError::Io(io_err),
            other => ConfigureError::Sealed(other),
        }
    }
}

/// Sets Claude Code's own `cleanupPeriodDays` setting - a vendor configuration value, not a
/// cancellAI-tracked artifact, so this does not go through the mutation-executor safety
/// boundary (SI-019 is about deleting *provider artifacts*; writing one JSON key to Claude
/// Code's own settings file is the same category of operation `cancellai.py`'s
/// `configure_claude_retention` already performs outside its own deletion path). Root-origin
/// gating happens in the caller (`cmd_configure`), before this is ever reached.
///
/// Every read/write below is issued through `cancellai_sealedfs::SealedRoot`, not a raw path -
/// E07-S07 round-1 independent verifier review found that the previous path-based version (a
/// `create_dir_all`/`read_to_string`/`OpenOptions`/`rename` sequence against
/// `claude_home.join(...)`) let a root swapped to a symlink *after* the caller's own
/// `is_symlink` check and *before* these operations redirect every one of them outside the
/// approved root (SI-002/SI-003/SI-013). `SealedRoot::establish` opens the root exactly once
/// with `O_NOFOLLOW` and retains that descriptor; every operation below is relative to it, so a
/// later rename/symlink-swap of `claude_home` itself cannot redirect any of them - see
/// `cancellai-sealedfs`'s own module docs for the full mechanism and rationale. The temp-name +
/// atomic-rename shape, and the `O_CREAT | O_EXCL` refusal of anything already present at the
/// temp name, match `cancellai.py::atomic_write_json`'s `tempfile.mkstemp` + `os.replace` shape,
/// as the pre-existing path-based version already did.
fn configure_claude_retention(claude_home: &Path, days: u32) -> Result<(), ConfigureError> {
    let root = cancellai_sealedfs::SealedRoot::establish(claude_home)?;

    let value: serde_json::Value = match root.read_child_to_string("settings.json")? {
        Some(text) => {
            let parsed: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| ConfigureError::MalformedSettings(e.to_string()))?;
            if !parsed.is_object() {
                return Err(ConfigureError::MalformedSettings(
                    "settings.json root must be a JSON object".to_string(),
                ));
            }
            parsed
        }
        None => serde_json::json!({}),
    };
    let mut value = value;
    value["cleanupPeriodDays"] = serde_json::json!(days);
    let serialized = serde_json::to_string_pretty(&value).expect("value is always representable");

    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(
        ".settings.json.{}.{unique_suffix}.cancellai-tmp",
        std::process::id()
    );

    // On a failure after the temp child was created, this leaves it behind rather than
    // cleaning it up - the same accepted tradeoff the prior path-based version documented:
    // `scripts/check_mutation_boundary.py` (SI-019) permits raw filesystem removal only inside
    // `cancellai-platform`'s mutation executor, and this uniquely-named, non-provider-artifact
    // temp file is simply orphaned on this rare error path rather than adding a second,
    // narrower deletion primitive to a crate that boundary exists to keep clean.
    root.write_new_child_atomically(&tmp_name, "settings.json", serialized.as_bytes())?;
    Ok(())
}

fn cmd_version(args: &[String]) -> i32 {
    if !args.is_empty() {
        return invalid_input(&format!("version accepts no arguments, got {args:?}"));
    }
    println!("cancellai-cli {VERSION}");
    0
}
