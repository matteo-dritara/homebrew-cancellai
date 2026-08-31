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

use std::collections::HashMap;
use std::io::Write;
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
use cancellai_provider_claude::ClaudeProvider;
use cancellai_provider_codex::CodexProvider;
use cancellai_safety::{ApprovedRoot, SealedPlan, execute_with_system_capabilities};
use documents::{ActionResultDoc, ProviderRootDoc, ScanCompletenessDoc};

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
}

fn resolve_all(flags: &CommonFlags) -> Resolved {
    let claude_root = roots::claude_home().unwrap_or_else(|| PathBuf::from("."));
    let codex_root = roots::codex_home().unwrap_or_else(|| PathBuf::from("."));
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

    let claude_provider = ClaudeProvider::new(&claude_root, true);
    let codex_provider = CodexProvider::new(&codex_root, true);

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

    Resolved {
        resolutions: vec![claude_resolution, codex_resolution],
        claude_root,
        codex_root,
    }
}

fn provider_root_docs(resolved: &Resolved) -> Vec<ProviderRootDoc> {
    let claude_provider = ClaudeProvider::new(&resolved.claude_root, true);
    let codex_provider = CodexProvider::new(&resolved.codex_root, true);
    let claude_fp = claude_provider.fingerprint();
    let codex_fp = codex_provider.fingerprint();
    vec![
        ProviderRootDoc::new(
            "root-claude-code".to_string(),
            "claude-code".to_string(),
            claude_fp.origin,
            claude_fp.confidence,
        ),
        ProviderRootDoc::new(
            "root-codex-cli".to_string(),
            "codex-cli".to_string(),
            codex_fp.origin,
            codex_fp.confidence,
        ),
    ]
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
    let resolved = resolve_all(&flags);
    let now = SystemClock.now();

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
            let actions = build_actions(&resolved.resolutions);
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

    let resolved = resolve_all(&flags);
    let actions = build_actions(&resolved.resolutions);
    let delete_count = actions
        .iter()
        .filter(|a| a.action_class == ActionClass::Delete)
        .count();

    if delete_count == 0 {
        println!("Nothing to clean: no artifact is both stale and unblocked.");
        return if any_incomplete(&resolved) {
            ErrorCategory::IncompleteInventory.exit_code()
        } else {
            0
        };
    }

    if flags.dry_run {
        println!("Dry-run only. No files were changed.");
        print_plan_summary(&actions);
        return 0;
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

    execute_clean(&resolved, &actions, flags.json)
}

fn execute_clean(resolved: &Resolved, actions: &[Action], json: bool) -> i32 {
    let by_id: HashMap<ArtifactId, &ClassifiedArtifact> = resolved
        .resolutions
        .iter()
        .flat_map(|r| r.artifacts.iter())
        .map(|c| (c.artifact.artifact_id.clone(), c))
        .collect();

    let resolver = SystemPathResolver;
    let observer = SystemIdentityObserver;
    let claude_approved_root = ApprovedRoot::establish(&resolved.claude_root, &resolver, &observer);
    let codex_approved_root = ApprovedRoot::establish(&resolved.codex_root, &resolver, &observer);

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
                    let (approved_root, provider_id) =
                        if classified.artifact.provider_id == "codex-cli" {
                            (&codex_approved_root, "codex-cli")
                        } else {
                            (&claude_approved_root, "claude-code")
                        };
                    delete_one(
                        approved_root,
                        &resolver,
                        &observer,
                        classified,
                        provider_id,
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

#[allow(clippy::too_many_arguments)]
fn delete_one(
    approved_root: &Result<ApprovedRoot, cancellai_safety::BoundaryError>,
    resolver: &SystemPathResolver,
    observer: &SystemIdentityObserver,
    classified: &ClassifiedArtifact,
    provider_id: &'static str,
    action: &Action,
    any_failed: &mut bool,
    any_blocked: &mut bool,
    reclaimed_bytes: &mut u64,
) -> ActionResultDoc {
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
        confidence: KnowledgeConfidence::Verified,
    };
    let sealed = SealedPlan::seal(
        root,
        root_fingerprint,
        &bound,
        ActionClass::Delete,
        AuthorityLevel::Govern,
        Reversibility::Irreversible,
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

    let claude_home = match roots::claude_home() {
        Some(p) => p,
        None => {
            return invalid_input(
                "could not resolve the Claude Code home directory ($HOME is not set)",
            );
        }
    };
    match configure_claude_retention(&claude_home, days) {
        Ok(()) => {
            println!("Set Claude Code cleanupPeriodDays to {days}.");
            0
        }
        Err(e) => {
            eprintln!("[{}] {e}", ErrorCategory::InternalFault.code());
            ErrorCategory::InternalFault.exit_code()
        }
    }
}

/// Sets Claude Code's own `cleanupPeriodDays` setting - a vendor configuration value, not a
/// cancellAI-tracked artifact, so this does not go through the mutation-executor safety
/// boundary (SI-019 is about deleting *provider artifacts*; writing one JSON key to Claude
/// Code's own settings file is the same category of operation `cancellai.py`'s
/// `configure_claude_retention` already performs outside its own deletion path).
fn configure_claude_retention(claude_home: &Path, days: u32) -> std::io::Result<()> {
    std::fs::create_dir_all(claude_home)?;
    let settings_path = claude_home.join("settings.json");
    let mut value: serde_json::Value = match std::fs::read_to_string(&settings_path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    if !value.is_object() {
        value = serde_json::json!({});
    }
    value["cleanupPeriodDays"] = serde_json::json!(days);
    let serialized = serde_json::to_string_pretty(&value).expect("value is always representable");
    let tmp_path = settings_path.with_extension("json.cancellai-tmp");
    std::fs::write(&tmp_path, serialized)?;
    std::fs::rename(&tmp_path, &settings_path)?;
    Ok(())
}

fn cmd_version(_args: &[String]) -> i32 {
    println!("cancellai-cli {VERSION}");
    0
}
