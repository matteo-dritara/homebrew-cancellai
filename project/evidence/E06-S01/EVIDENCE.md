# Evidence Packet - E06-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E06)
- Change Risk: CR3
- Spec version/commit: `rust/crates/cancellai-cli/*` (new command surface),
  `rust/crates/cancellai-policy/*` (new - was an empty skeleton), `rust/crates/cancellai-model/
  src/{agent_artifact,action,evidence}.rs` and `vocabulary.rs` additions (`RiskClass`,
  `ResidencyState`, `AgentArtifact`, `Action`), `rust/crates/cancellai-platform/src/process.rs`
  (new `ProcessObserver` seam), `rust/crates/cancellai-safety/src/mutation_executor.rs`
  (`execute_with_system_capabilities` addition), `docs/adrs/0016-...md` (new),
  `docs/CLI_RUST.md` (new) as added in this change

## Outcome

PASS

## Scope

Implements the first real `cancellai-cli` command surface (`status`/`inspect`/`plan`/`clean`/
`configure`/`version`) against the Rust engine, per the story's outcome. The generated brief
undersells this story's real size: none of the glue between "provider adapters discover
sessions" (E05) and "the safety kernel can execute a sealed plan" (E03) existed yet -
`AgentArtifact`/`RiskClass`/`Action` were not defined as Rust types, `cancellai-policy` was an
empty skeleton, and no code anywhere connected `cancellai-inventory`/the provider adapters to
`cancellai-safety`. `docs/architecture/DOMAIN_MODEL.md`'s own `AgentArtifact` section and
`cancellai-safety::authority`'s module docs both say explicitly that this classification
("deriving a ceiling from `RiskClass` is a classification decision this story does not invent")
was left for whichever story first has the provider/policy knowledge to make it - E06-S01 is
that story, not scope creep beyond it (`docs/development/WORK_ITEM_MODEL.md` "Story changes
during implementation": the AC was not wrong, the implementation was simply missing, and the
architecture was already specified).

Three additions to already-`done` epics were required and are disclosed here rather than
silently expanded elsewhere:

1. **`cancellai-platform::process`** (`ProcessObserver`/`SystemProcessObserver`/
   `SyntheticProcessObserver`) - ported from `cancellai.py`'s `active_processes`. Without it,
   `clean` would have had to either skip the "is a provider process currently writing this
   artifact" check entirely (a real safety regression versus the Python reference) or fake
   `ActivityState::Idle`/`Stale` from mtime alone, which cannot rule out an actively-appending
   process. Read-only OS capability, same seam pattern as `Clock`/`FsObserver`.
2. **`cancellai-safety::execute_with_system_capabilities`** - a thin, boundary-compliant
   production entry point. `scripts/check_mutation_boundary.py` (SI-019) statically forbids any
   file but `cancellai-platform/src/mutation.rs` and `cancellai-safety/src/mutation_executor.rs`
   from referencing `SystemMutationExecutor` or calling `.mutate(` at all - `cancellai-cli`
   could not construct one itself and stay compliant. This wrapper hardwires the real
   `SystemIdentityObserver`/`SystemMutationExecutor` inside the one file already allowed to
   reference them; it adds no new authority path (still calls the existing, unchanged
   `execute`).
3. **`RiskClass`/`ResidencyState`/`AgentArtifact`/`Action`/`Evidence` types in
   `cancellai-model`** - the domain types DOMAIN_MODEL.md already specifies but no story had
   built yet.

**Deliberately not ported** (disclosed, not silent): `--aggressive` (legacy/cache category
widening - `cancellai-policy::retention`'s own module doc names this explicitly; omitting a
*widening* flag is fail-closed, never a superset of what Python would delete), `status
--paths/--coverage/--top`, `clean --keep-claude-history`/`--verbose`. `docs/CLI_RUST.md`
tracks this list so it is not lost.

## Key design decision: `clean` deletes, it does not quarantine

`cancellai-safety::mutation_executor::execute` implements exactly one destructive operation
today (`ActionClass::Delete` on a plain file); `Quarantine` has no OS-primitive wiring and no
destination field on `SealedPlan` yet. Given that constraint, capping ordinary sessions'
`authority_ceiling` at `AuthorityLevel::Quarantine` (the "safer-looking" choice) would make
`clean` permanently unable to do anything at all, since `effective_authority`'s monotonic
minimum can never then reach `Govern` (`minimum_authority_for(Delete)`). This is recorded as
[ADR-0016](../../../docs/adrs/0016-rust-artifact-risk-classification.md), including why C-04
("quarantine before purge") does not mandate `Quarantine` here (quarantine is not *technically
available* in this build) and what changes once a future story adds it.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Read-only default is explicit and no flag implies clean | `main.rs::split_command`: no subcommand, or a leading flag with no subcommand, always resolves to `status` - never `clean`. `status`/`inspect`/`plan` never call `cancellai_safety::execute_with_system_capabilities` at all (only `cmd_clean`'s own function does, and only after `--dry-run` is false and confirmation/`--yes` is affirmative). Test: `cli_behavior.rs::no_arguments_defaults_to_read_only_status_and_never_mutates`, `::plan_is_read_only_and_produces_a_schema_conformant_document`, `::clean_dry_run_never_deletes_anything`, `::clean_without_confirmation_or_dry_run_declines_and_deletes_nothing`. | PASS |
| AC2 - JSON schemas match versioned contract | `documents.rs` builds the common envelope (`schema_version`, `document_type`, `generated_at`, `generator`) plus inventory/plan/result document shapes exactly as `docs/architecture/JSON_CONTRACTS.md` specifies, reusing `cancellai_model::{AgentArtifact, Action}`'s own `Serialize` impls for the safety-critical per-record envelopes. Test: `cli_behavior.rs::plan_is_read_only_and_produces_a_schema_conformant_document`, `::clean_yes_deletes_a_stale_unprotected_session_and_reports_it_in_the_result_document` assert `document_type`/`schema_version`/summary fields directly against a real running binary's output. Manually verified against the exact shape of `tests/fixtures/schemas/golden/{inventory,plan}.golden.json` (field names/nesting match; `scripts/check_schemas.py check` still passes unchanged since those golden files were not modified). | PASS |
| AC3 - Exit codes match the documented taxonomy | `cancellai_model::ErrorCategory::exit_code()` (already built at E02-S03) is the single source of truth every command path returns through: 2 for invalid/ambiguous input (`invalid_input`), 4 for incomplete-scan/safety-block, 3 for a real mutation failure, 1 for a declined confirmation, 0 for success. Test: `cli_behavior.rs::an_unrecognized_flag_is_refused_with_exit_code_2_and_never_partially_runs`, `::clean_json_without_yes_or_dry_run_is_refused_before_touching_anything` (both assert exit 2), `::clean_without_confirmation_or_dry_run_declines_and_deletes_nothing` (asserts exit 1). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-007 (ambiguous CLI never escalates) | Unrecognized flag; `clean --json` with neither `--yes` nor `--dry-run` (an automation caller that did not state destructive intent explicitly) | `cli_behavior.rs::an_unrecognized_flag_is_refused_with_exit_code_2_and_never_partially_runs`, `::clean_json_without_yes_or_dry_run_is_refused_before_touching_anything` - both refuse (exit 2) before touching the filesystem, file still exists afterward | PASS |
| SI-008/SI-009 (incomplete scan/missing evidence never authorizes) | Claude's `projects/` present but structurally unavailable (symlink) is reported `scan_complete=false` and surfaces `IncompleteInventory` (exit 4) from `status`/`inspect`/`plan`, not silently treated as "no sessions." Session mtime unreadable -> `IntegrityState::Unknown` -> `lifecycle_ceiling` collapses to `Recommend` (< `Govern`) | `cancellai-policy::retention::tests` (`a_missing_provider_root_is_a_known_empty_scan_not_a_withheld_one` distinguishes "not installed" from "structurally broken"); `resolve_claude`'s `SessionDiscoveryScope::Unavailable` branch | PASS |
| A live provider process must block deletion (mirrors Python's `active_processes`/`--allow-running`, not a numbered SI but directly required for `clean` to be honest about SI-008/SI-009's spirit) | A `SyntheticProcessObserver` reporting `claude` running, and separately an *incomplete* process probe (probe itself failed) | `retention::tests::a_running_provider_process_blocks_every_action_for_that_tool_even_when_stale`, `::an_incomplete_process_probe_fails_closed_exactly_like_a_running_process` - both produce `Observe`-only actions, never `Delete` | PASS |
| Protected-name barrier (SI-006) | A path the provider reports `ProtectionOutcome::Protected` for, even though it is stale and unpinned | `retention::tests::a_protected_name_is_never_a_deletion_candidate_even_if_stale_and_unpinned` - `authority_ceiling` stays `Observe`, action is `Observe` | PASS |
| `keep_latest` protects regardless of protected-name/process state (per-tool, per-session/tree) | Two sessions, one more recently modified than the other, `keep_latest=1` | `retention::tests::keep_latest_protects_the_most_recently_modified_sessions_from_deletion` (Claude), `::codex_keep_latest_protects_a_whole_subagent_tree_even_when_the_root_looks_old` (Codex subagent tree - a recent child protects an old-looking root, matching `cancellai.py::choose_codex_old_sessions`) | PASS |
| SI-019/SI-020 (single mutation boundary, irreversible actions stronger-gated) | `clean --yes` end-to-end through `execute_with_system_capabilities` -> `cancellai_safety::execute` (unchanged) -> real `SystemMutationExecutor` | `cli_behavior.rs::clean_yes_deletes_a_stale_unprotected_session_and_reports_it_in_the_result_document` (asserts both files are actually gone from disk afterward); `scripts/check_mutation_boundary.py check` still passes (only `mutation.rs`/`mutation_executor.rs` reference the capability) | PASS |
| SI-021/SI-022 (trust cannot self-assign) | `cancellai-policy::trust::builtin_provider_trust` goes through the real `TrustedTier::promote` gate (not a bare `ProviderTrust::BuiltinVerified` construction, which the type system does not even allow outside `cancellai-safety`) | `trust::tests::builtin_provider_trust_reaches_the_top_tier`; `cargo check` itself proves no bypass compiles (the gate is a private-field type, not a runtime check) | PASS |

## Verification Commands

```text
# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Python governance (repository-wide)
python3 scripts/check_docs.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check
```

All green. `cargo test --workspace` includes 3 new unit tests (`cancellai-cli::timestamp`), 10
new integration tests (`cancellai-cli/tests/cli_behavior.rs`, spawning the real built binary
against isolated synthetic `$HOME`/`CLAUDE_CONFIG_DIR`/`CODEX_HOME` directories - never the real
user's provider state, per AGENTS.md), 9 new tests in `cancellai-model` (up from the prior 4:
`RiskClass` ordering, `AgentArtifact`/`Action`/`Evidence` serialization), 8 new tests in
`cancellai-policy` (all of it new), 3 new tests in `cancellai-platform::process`. No existing
test in any of the 5 pre-existing epics' crates was modified; all continue passing unchanged.

**Manual verification beyond the automated suite** (not committed as tests, recorded here per
the evidence hierarchy's "manual claim" tier): ran the built binary directly against a synthetic
`$HOME` for every command (`status`, `plan --json`, `clean --dry-run`, `clean --yes --json`,
`configure --claude-retention`, `version`, several invalid-input cases) and inspected output by
hand before writing the equivalent automated test - this is how the "a running `claude` process
on the development machine correctly blocks deletion" behavior was discovered to be *correct*
or first appeared to be a bug (real `ps` on the development machine reports actual `claude`/
`codex`-named processes - i.e. this very agent session and an unrelated running Codex instance
- which is exactly the fail-closed behavior SI-008/SI-009's spirit requires, not a defect).

## Compatibility

- `SystemProcessObserver` shells to `ps -axo pid=,comm=` (Unix). On a platform where `ps`/
  `/bin/ps` do not exist (Windows), `Command::spawn` fails and the observer reports
  `complete: false` - fails closed to "treat every named process as possibly running," not a
  crash or a false negative. No explicit `#[cfg(windows)]` branch was needed for this to be
  honest; it falls out of the existing error handling. Real Windows process enumeration remains
  future work (E07, matching `SystemIdentityObserver`'s own documented gap).
- `mutation_executor::execute` (unchanged) only performs `Delete` for `FileKind::File` -
  directories (e.g. a Claude session's companion payload directory) are never deletion targets
  in this build; `cancellai-policy::retention` only ever proposes `Delete` for the session
  `.jsonl` file itself, consistent with what the executor can actually confirm-and-delete
  safely. A companion directory left behind after its `.jsonl` sibling is deleted is a disclosed
  gap versus `cancellai.py` (which deletes the whole tree), not silently claimed as covered.
- `tests/fixtures/schemas/golden/*.json` were not modified; `scripts/check_schemas.py check`
  passes unchanged.

## Performance / operability

- Every integration test uses isolated temp directories with at most a handful of synthetic
  files; `cargo test --workspace` (whole repository) completes in a few seconds.
- `SystemProcessObserver` bounds its `ps` call to 5 seconds (matching `cancellai.py`'s own
  `subprocess.run(..., timeout=5)`), hand-rolled via `Child::try_wait`/`kill` polling since no
  process-timeout dependency exists in this workspace (AGENTS.md: no dependency merely to
  reduce implementation effort).

## Documentation updated

- `docs/CLI_RUST.md` (new) - the Rust CLI's own command/flag/exit-code reference. `docs/CLI.md`
  (the story's literally declared documentation impact) was deliberately **not** touched:
  AGENTS.md states it "remains generated from the current Python CLI until the Rust CLI
  generator replaces it through an explicit story/ADR," and no such story/ADR exists. Creating a
  separate, clearly-scoped document satisfies the underlying need (a CLI reference for what this
  story built) without violating that carve-out.
- `docs/INDEX.md` - links the new `CLI_RUST.md`.
- `docs/adrs/0016-rust-artifact-risk-classification.md` (new) - the `RiskClass`/authority
  ceiling mapping decision.
- Module-level doc comments throughout the new/changed source files explain the rationale
  inline (matching this codebase's established convention), most notably
  `cancellai-policy::retention`'s module doc and `cancellai-safety::mutation_executor`'s
  `execute_with_system_capabilities` doc.

## Residual risks

- **No quarantine/undo** for anything `clean` deletes - permanent, matching `cancellai.py`
  today, disclosed in ADR-0016 and `docs/CLI_RUST.md`. A future story adding real quarantine
  support to `cancellai-safety` should revisit ADR-0016's mapping.
- **`--aggressive` and several minor Python CLI flags are not ported** (see Scope) - a tracked
  parity gap, not attempted to be silently covered.
- **Companion payload directories are never deleted** by `clean` (only the session `.jsonl`
  file itself) - `mutation_executor` has no directory-tree deletion path yet; disclosed above.
- **No golden CLI snapshot was run on Windows/Linux CI as part of this evidence** - the
  integration tests are OS-agnostic (temp dirs, no hardcoded paths, `HOME`/`CLAUDE_CONFIG_DIR`/
  `CODEX_HOME` env overrides) and will run automatically once CI's existing three-OS matrix
  (`rust.yml`, ADR-0015) picks up this commit; this evidence packet was produced on macOS only,
  matching the development machine.
- **`SystemProcessObserver`'s exact-name matching has real false-negative potential** (a
  differently-named binary, a containerized `ps` that cannot see the host's `claude`/`codex`
  process) - explicitly documented in that module's own docs as "never the sole safety
  control," the same caveat `cancellai.py`'s own `active_processes` carries.
- **E06-S01's own review has not happened yet** - per `AGENT_PROTOCOL.md`, epic-scoped review
  begins once every story in E06 is `ready_for_review`; this is the first of four.

## Round 1 verifier verdict

FAIL (`project/evidence/E06-VERIFIER-REVIEW.md`, 2026-09-01). Four independently-reproduced
defects, all repaired below.

## Repairs for E06 verifier review round 1

Each defect from the round-1 review, its root cause, the fix, and the regression test that now
reproduces the exact adversarial case and proves it is closed:

1. **Custom root treated as default (SI-002/SI-004).** Root cause: `resolve_all`/
   `provider_root_docs` in `cancellai-cli/src/main.rs` each hard-coded
   `ClaudeProvider::new(&root, true)`/`CodexProvider::new(&root, true)` - `is_default_root` was
   never actually derived from whether `CLAUDE_CONFIG_DIR`/`CODEX_HOME` was set, so a custom
   root was always fingerprinted `origin=default`. Fixed: `cancellai-cli/src/roots.rs` now
   derives `is_default` from comparing the resolved path against the real
   `$HOME/.claude`/`$HOME/.codex` path (matching `cancellai.py::fingerprint_root`'s own
   comparison exactly, including the "an override that happens to point at the literal default
   path is still default" edge case); `Resolved` computes each provider's fingerprint exactly
   once from the real value and every caller reuses it. A new hard gate,
   `withhold_for_root_authority` (ADR-0013: only `origin=default` may be mutated, confidence is
   never sufficient on its own), downgrades every `Delete` action for a non-default root to
   `Observe` before `plan`/`clean` ever see it, and `delete_one` independently re-checks the same
   condition immediately before sealing/executing (defense in depth, not reliance on the
   upstream decision alone). `documents.rs`'s `mutation_eligible` field, previously also wrongly
   `true` for `RootConfidence::High`, now matches the same origin-only rule.
   Regression: `cancellai-cli/tests/cli_behavior.rs::clean_refuses_to_mutate_a_custom_claude_config_dir_root_even_with_yes`,
   `::plan_reports_a_custom_root_as_not_mutation_eligible_and_withholds_the_delete_candidate`;
   `cancellai-cli/src/roots.rs` unit tests (`low_confidence_custom_root_containing_only_a_low_confidence_marker_is_still_non_default`
   and 5 others covering absent-`$HOME`, override-equals-default, etc.). Independently
   reproduced outside the automated suite too (recorded in this evidence packet's manual
   verification note).
2. **`configure` follows a predictable symlink and silently discards malformed settings
   (SI-007/SI-019).** Root cause: the temp file used a fixed, guessable name
   (`settings.json.cancellai-tmp`) written via `std::fs::write` (follows an existing symlink at
   that path); a JSON parse failure silently fell back to `{}`, discarding whatever was there.
   Fixed: `configure_claude_retention` now opens a per-attempt, PID-and-nanosecond-unique temp
   path with `OpenOptions::create_new` (`O_CREAT|O_EXCL`, refuses to open anything already at
   that path, symlink or not) and `fsync`s before the final `rename` (which POSIX-replaces
   whatever sits at `settings.json` itself, never following it, matching
   `cancellai.py::atomic_write_json`'s `tempfile.mkstemp` + `os.replace` shape); a JSON parse
   failure or a non-object root now returns a distinct `ConfigureError::MalformedSettings` and
   the command refuses (exit `SAFETY_BLOCK`), leaving the file untouched. `cmd_configure` also
   now fingerprints the target root exactly like `clean` does and refuses a non-default root
   before ever touching the filesystem.
   Regression: `cli_behavior.rs::configure_never_writes_through_a_preexisting_settings_json_symlink_to_an_outside_file`
   (asserts the symlink's outside target is byte-for-byte unchanged and `settings.json` is a
   real file afterward), `::configure_refuses_malformed_settings_json_instead_of_silently_replacing_it`,
   `::configure_refuses_a_custom_claude_config_dir_root`.
3. **Partial-scan confidence/exit-code violations (SI-008/SI-009, `docs/architecture/
   JSON_CONTRACTS.md`).** Root cause: `cancellai-policy::retention::classify` only downgraded
   the *specific* artifact whose own companion evidence was degraded to `KnowledgeConfidence::
   Observed` (not even `LOW/UNKNOWN`) - the other, perfectly-readable sessions from the same
   incomplete scope kept `Verified`, violating JSON_CONTRACTS.md's documented ceiling. Separately,
   `cmd_clean`'s `--dry-run` path and its "nothing to clean" short-circuit always returned exit 0
   regardless of whether work was actually withheld. Fixed: `resolve_claude` now downgrades
   every artifact from an incomplete scope to `LowUnknown` before returning, not only the
   degraded one; `cmd_clean` computes `safety_withheld` (incomplete scan OR root-authority
   withholding) once and every return path - dry-run, nothing-to-clean, and a real run - reflects
   it in the exit code (`SAFETY_BLOCK`, 4). `cmd_version` now also rejects unrecognized arguments
   instead of ignoring them silently.
   Regression: `cancellai-policy::retention::tests::a_degraded_companion_withholds_every_action_for_the_whole_tool_not_only_its_own_session`
   (extended with a `knowledge_confidence` assertion over every artifact, not just the degraded
   one); `cli_behavior.rs::version_rejects_unrecognized_arguments`. Independently reproduced end
   to end with a real built binary against a `claude-partial-tree`-shaped root plus an eligible
   Codex candidate in the same run: `clean --dry-run` now exits 4 (was 0) and correctly names the
   withheld tool; a real `clean --yes` exits 4 while still deleting the genuinely eligible Codex
   candidate (partial safety-withholding is visible in the exit code even when other work
   succeeds) - not committed as an automated test (ad hoc reproduction, recorded here per the
   evidence hierarchy's "manual claim" tier), because it duplicates what the parity gate
   (E06-S02) and the two automated tiers above already prove separately.
4. **`process_not_running` recorded but never revalidated at execution time (SI-013's TOCTOU
   principle, not yet applied to process liveness).** Root cause: `Action.execution_preconditions`
   included a `process_not_running` entry in the emitted plan document, but nothing in
   `cancellai-safety::mutation_executor::execute` ever re-checked it - only filesystem identity
   was revalidated immediately before mutation. Fixed: `SealedPlan` now carries an optional
   `process_guard: Option<&'static [&'static str]>`; `execute` re-probes it with a fresh
   `ProcessObserver` call immediately before the delete operation (after identity revalidation,
   before the OS call), failing closed exactly like `revalidate` does. `cancellai-cli::delete_one`
   seals every real deletion with the correct provider process names, unless the operator passed
   `--allow-running` (the same explicit override already governs the plan-build-time check, so a
   stated intent is honored consistently rather than silently overridden by a second,
   un-opt-out-able gate).
   Regression: `cancellai-safety::mutation_executor::tests::execute_blocks_when_the_guarded_process_is_reported_running`,
   `::execute_blocks_when_the_process_probe_is_incomplete_fail_closed`,
   `::execute_never_calls_mutate_when_the_process_guard_blocks`,
   `::execute_proceeds_when_the_guarded_process_is_confirmed_not_running`.

Every existing test in the crate (including the pre-existing `cli_behavior.rs`/
`install_rollback.rs` suites) was also updated where it depended on the old, incorrect
behavior: both integration-test harnesses previously pointed `CLAUDE_CONFIG_DIR`/`CODEX_HOME` at
their fixture directories unconditionally, which - once the root-authority gate was fixed - is
exactly the *custom*-root path and would never again reach the delete path these tests exist to
exercise. They now resolve the real OS-default root via `$HOME/.claude`/`$HOME/.codex` (no
override), and new tests were added specifically for the custom-root path (see item 1 above).

## Round 2 verifier verdict

FAIL (`project/evidence/E06-VERIFIER-REVIEW-ROUND2.md`, 2026-09-01): round-1 fixes held (custom
`CLAUDE_CONFIG_DIR` root, configuration symlink, malformed settings, partial scan, process
precondition all reproduced closed), but a new, independently-reproduced defect survived: a
*default-named* root that is itself a symlink (`$HOME/.claude -> <outside>`, no
`CLAUDE_CONFIG_DIR` override at all) was still lexically classified `origin=default` and a
stale session reachable only through it was deleted. Per the two-round ceiling (ADR-0014/
PD-022), this was recorded as new backlog item E07-S07 rather than a third E06 review round;
E06-S01 itself returned to `in_progress`.

## Repair for the round-2 finding

Root cause: `roots.rs`'s "no override" branch set `is_default: true` unconditionally, without
ever checking whether the leaf path (`$HOME/.claude`/`$HOME/.codex`) is itself a symlink -
authority was inferred from the lexical name alone, exactly as the finding states.

Fixed:

- `roots::is_symlink` (new, `pub`) checks the literal leaf path with `symlink_metadata` (never
  following it) - a nonexistent path is correctly *not* a symlink (a fresh machine's absent
  `~/.claude` stays positively default, matching `cancellai.py::fingerprint_root`'s own
  "authoritative by definition, including when empty or absent").
- `roots::resolve_from` now takes the caller's `default_is_symlink` fact and folds it into
  `is_default` uniformly for *both* the no-override and override-naming-the-same-path branches
  - an operator (or attacker) writing `CLAUDE_CONFIG_DIR=$HOME/.claude` when that path is itself
  a symlink gets the same refusal as the bare no-override case.
- Classification time was not enough on its own (the review's own repair text: "reject ... at
  plan and execution time"): `main.rs::establish_verified_root` (new) re-checks `is_symlink`
  fresh, independent of the cached fingerprint, immediately before `ApprovedRoot::establish` in
  `execute_clean` - closing the TOCTOU window between `resolve_all()` (top of `cmd_clean`, before
  the interactive confirmation prompt) and the real mutation. `cmd_configure` gained the
  identical fresh re-check immediately before `configure_claude_retention`.

Regression: `roots.rs` unit tests
`a_default_path_that_is_itself_a_symlink_is_never_the_default_origin`,
`an_override_literally_naming_the_default_path_is_still_refused_when_that_path_is_a_symlink`,
`a_nonexistent_default_path_is_not_a_symlink`, `is_symlink_detects_a_real_symlink_but_not_a_real_directory`;
`cli_behavior.rs`'s `clean_refuses_to_mutate_when_home_dot_claude_is_itself_a_symlink` and
`configure_refuses_when_home_dot_claude_is_itself_a_symlink` reproduce the review's exact
scenario end-to-end against the real built binary (no `CLAUDE_CONFIG_DIR` override, a real
Unix symlink) and assert `SAFETY_BLOCK` (4) with the session/settings untouched.

Verified: `cargo fmt --check` / `clippy --workspace --all-targets --all-features -D warnings` /
`cargo test --workspace` / `cargo deny check`, run against *both* the pinned 1.94.0 toolchain
and CI's actual 1.98.0 stable (via `rustup run 1.98.0 ...`) - all green.

**Scope note, not overclaimed as done:** this closes the Unix symlink case only, matching
`roots.rs`'s own pre-existing, disclosed "Unix-only for now" scope. E07-S07's full AC also
covers Windows junction/reparse points, which remain untouched (tracked by that story, along
with the pre-existing E03-S01/E07-S02 gap that Windows has no real file identity at all yet -
see `docs/development/RELEASE_GATES.md`'s G3 section, updated 2026-09-01). This repair is
recorded here, against the story whose files it lives in, rather than as a premature
`ready_for_review` claim on E07-S07 itself, which depends on E06-S01 (still `in_progress`) and
is not fully done.

## Verifier verdict

Round 1: FAIL, repaired (see above). Round 2: FAIL on one new finding, repaired (see above).
E06's two independent-review rounds are exhausted per ADR-0014/PD-022; no further E06-scoped
review is expected. The repair above is offered as evidence for whoever picks up E07-S07 (which
formally tracks this class of defect) or reopens E06-S01, not as a self-graduated status change.
