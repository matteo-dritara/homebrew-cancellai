//! Prints the reference-provider compatibility matrix as JSON (E05-S05, AC1: "Compatibility
//! is reported per capability, not a single supported boolean"; AC2: "Unknown version
//! behavior is documented and fail-closed").
//!
//! `scripts/check_provider_compatibility.py generate`/`check` runs this example and renders
//! its output into `docs/PROVIDERS.md`'s generated matrix section - the "Generated matrix
//! drift check from adapter metadata" this story's verification plan names. This is an
//! `examples/` binary, not a `cancellai-cli` command surface: the real CLI (E06 Rust CLI
//! Parity and Cutover) does not exist yet, and this generator is not it.
//!
//! Two layout scenarios are run against each of the two reference adapters
//! (`cancellai-provider-claude`, `cancellai-provider-codex`), both against an empty candidate
//! root - the only difference between them is whether the caller asserts it is the OS-default
//! provider directory:
//!
//! - `known_default_root`: `is_default_root = true`, matching how every fixture in
//!   `tests/fixtures/` is characterized (`scripts/characterize.py`'s own comment: a non-default
//!   root is always inspection-only, so every existing NORMATIVE record already assumes this).
//! - `unknown_custom_root`: `is_default_root = false` and no recognizable marker present -
//!   the fail-closed case AC2 requires.
//!
//! Deliberately not attempted here: per-real-version compatibility ("Claude Code v1.2.3
//! layout X"). No version-tagged fixture corpus exists yet (`docs/PROVIDERS.md`'s own
//! standing note: "exact tested versions and capability evidence will become generated
//! adapter metadata during P1/P2") - this matrix reports what the two reference adapters
//! actually produce today, not invented version history.
//!
//! `NativeDeleteCapability` is pinned to an explicit, deliberately nonexistent `codex_bin`
//! rather than left to resolve `codex` off this process's own `PATH` - a generated, committed
//! matrix must not depend on whatever happens to be installed on the machine that ran
//! `generate` (a real `codex` CLI on a developer's `PATH` would otherwise make this file's
//! committed content diverge from CI's, or from another contributor's machine, even though
//! nothing in the adapter itself changed).

use std::path::PathBuf;

use cancellai_provider_api::{ProviderCapabilities, capability_report};
use cancellai_provider_claude::ClaudeProvider;
use cancellai_provider_codex::CodexProvider;

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "cancellai-compatibility-matrix-{label}-{}",
            std::process::id()
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("create temp root");
        Self(dir)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

fn provider_rows(
    provider_id: &str,
    layout: &str,
    provider: &dyn ProviderCapabilities,
) -> Vec<serde_json::Value> {
    capability_report(provider)
        .into_iter()
        .map(|(kind, outcome)| {
            serde_json::json!({
                "provider_id": provider_id,
                "layout": layout,
                "capability": kind.code(),
                "support": outcome.support().code(),
                "confidence": serde_json::to_value(outcome.confidence()).expect("KnowledgeConfidence serializes"),
            })
        })
        .collect()
}

fn main() {
    let claude_known = TempRoot::new("claude-known");
    let claude_unknown = TempRoot::new("claude-unknown");
    let codex_known = TempRoot::new("codex-known");
    let codex_unknown = TempRoot::new("codex-unknown");

    let mut rows = Vec::new();
    rows.extend(provider_rows(
        "claude-code",
        "known_default_root",
        &ClaudeProvider::new(&claude_known.0, true),
    ));
    rows.extend(provider_rows(
        "claude-code",
        "unknown_custom_root",
        &ClaudeProvider::new(&claude_unknown.0, false),
    ));
    // `native_delete_capability` otherwise probes whatever `codex` binary happens to be on the
    // *generating machine's* PATH (`CodexProvider::native_delete_support` with no explicit
    // `codex_bin`), which would make this generated, committed matrix depend on local
    // developer/CI machine state rather than on the adapter's own logic - a nonexistent path
    // pins every run to the same deterministic "no binary" answer regardless of environment.
    let no_codex_binary = "/nonexistent/cancellai-compatibility-matrix-no-real-codex-binary";
    rows.extend(provider_rows(
        "codex-cli",
        "known_default_root",
        &CodexProvider::new(&codex_known.0, true).with_codex_bin(no_codex_binary),
    ));
    rows.extend(provider_rows(
        "codex-cli",
        "unknown_custom_root",
        &CodexProvider::new(&codex_unknown.0, false).with_codex_bin(no_codex_binary),
    ));

    println!(
        "{}",
        serde_json::to_string_pretty(&rows).expect("matrix rows serialize")
    );
}
