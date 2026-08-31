# Evidence Packet - E05-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E05)
- Change Risk: CR3
- Spec version/commit: `rust/crates/cancellai-provider-api/src/root_fingerprint.rs` (new,
  factored out of E05-S03's Claude adapter),
  `rust/crates/cancellai-provider-codex/src/{fingerprint,protected_names,session,graph,
  native_delete,lib}.rs` (new/rewritten),
  `rust/crates/cancellai-provider-codex/tests/codex_fixture_parity.rs` (new) as added in this
  change

## Outcome

PASS

## Scope

Ports `cancellai.py`'s Codex-specific discovery/classification/subagent-graph/native-delete
logic to Rust behind E05-S01's `ProviderCapabilities` contract: root fingerprinting
(`ROOT_MARKERS["codex"]`/`fingerprint_root`), the protected-name barrier
(`CODEX_PROTECTED_NAMES`, reusing E05-S03's shared `cancellai-provider-api::protection`),
rollout discovery with bounded parent-lineage parsing (`discover_codex_sessions`/
`read_codex_parent_session_id`), the subagent/rollout graph-*building* half of
`choose_codex_old_sessions` (`root_id_for`/grouping, as `group_into_subagent_trees`), and
native-delete capability detection (`codex_delete_supported`). Deliberately out of scope, same
rationale as E05-S03: `choose_codex_old_sessions`'s own age/`--keep-latest`
*selection*/deduplication, `execute_plan`/mutation, and `active_processes` - PLAN/EXECUTE-stage
or policy-dependent concerns later stories own.

**Refactor ahead of this story:** `RootOrigin`/`RootConfidence`/`RootFingerprint`/
`derive_root_confidence` moved from `cancellai-provider-claude` (E05-S03) into
`cancellai-provider-api::root_fingerprint`, matching `cancellai.py`'s own `RootAuthority` -
one dataclass shared across both tools via a `tool` field, not duplicated per tool. E05-S03's
`ClaudeProvider`/`fingerprint_claude_root` now consume the shared type; no behavior changed
(verified: all pre-existing E05-S03 tests still pass unmodified in content, only import paths
changed).

**Bug found and fixed ahead of this story, in E05-S03's own `extract_uuid`** (separate commit
`fix(E05-S03): extract_uuid must match the last UUID and lowercase it`): `cancellai.py`'s
`extract_uuid` takes the *last* regex match and lowercases it; the E05-S03 port matched the
*first* occurrence and preserved case. Latent in E05-S03 (its fixtures never exercised
multi-UUID or mixed-case input); found while designing Codex rollout filename parsing
(`rollout-<timestamp>-<uuid>.jsonl`, the exact shape where this matters), before it could
propagate into this story's own logic.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Root/subagent trees are preserved as graph relationships | `group_into_subagent_trees` ports `root_id_for`/grouping unchanged: a session's parent chain is walked via a `by_id` lookup, a session with no parent or an undiscovered parent is its own root, and a cycle isolates the *originating* session rather than looping or over-merging. `graph::tests` cover: two-child tree grouping, no-parent single-member tree, undiscovered-parent isolation, cycle isolation (with an assertion the function actually terminates), and duplicate-id-same-tree. `codex_subagent_tree_matches_the_committed_characterization` (fixture-recipe parity) proves the exact `codex-subagent-tree` fixture's 3 rollouts resolve to one tree with all 3 as members. | PASS |
| AC2 - Native delete capability is detected without assuming filesystem fallback equivalence | `NativeDeleteSupport` is a four-variant enum (`Supported`/`Unsupported`/`BinaryNotFound`/`ProbeFailed`), never a bare boolean - `Unsupported` (binary ran, declined `--force`) and `BinaryNotFound`/`ProbeFailed` (no answer obtained at all) are distinct evidentiary claims a caller cannot collapse. `native_delete::tests` drive this against a small, test-controlled fake `codex` shell-script CLI (the "native-delete fake CLI integration tests" this story's verification plan names): advertises `--force` → `Supported`; omits it → `Unsupported`; exits non-zero even while mentioning `--force` → `Unsupported` (exit code matters independently of text, matching `cancellai.py`); hangs → killed and reported `ProbeFailed`, not a hang; writes >64KiB of output before exiting → still completes correctly (the exact pipe-deadlock regression this module's background-thread readers exist to prevent - see Residual risks for the one case that cannot be fully closed). `tests::ac2_native_delete_capability_is_never_inferred_from_root_detection_alone` proves a successfully-detected root does not by itself grant native-delete support. | PASS |
| AC3 - SQLite/config/auth/plugin state stays protected | `CODEX_PROTECTED_NAMES` is a verbatim copy of `cancellai.py`'s list (auth.json/config.toml/skills/rules/memories/plugins). `every_documented_protected_name_is_actually_protected_at_the_top_level` and `codex_protected_state_matches_the_committed_characterization` (fixture-recipe parity, all 6 names) are the evidence. `sqlite/` itself is a root-fingerprint marker in `cancellai.py`, not a `CODEX_PROTECTED_NAMES` entry - this port matches that exactly rather than expanding the protected list beyond what the reference documents. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 (protected/unknown state is non-destructive) | Same posture as E05-S03: this adapter performs no mutation (`scripts/check_mutation_boundary.py` still passes, zero new references); unknown layouts and protected entries are reported with `Unsupported`/`Protected` evidence for a future PLAN-stage authority computation to consume | PASS (evidence-supply half only; end-to-end authority wiring remains a documented residual, matching E05-S03/E04-S03) |
| SI-004 (unknown provider layout/version reduces capability) | `ac_an_unknown_layout_downgrades_detection_to_inspection_only` | `RootConfidence::Unknown` → `SupportState::Unsupported`/`AuthorityLevel::Observe` regardless of the provider id being recognized as "codex-cli" | PASS |
| SI-018 (filesystem/volume boundaries are explicit, applied to discovery accounting) | `codex-symlink-escape` fixture: a symlink inside `sessions/` pointing outside the approved root | `codex_symlink_escape_matches_the_committed_characterization` proves only the one real rollout is discovered; separately, `a_symlinked_directory_is_never_descended_into_but_a_symlinked_file_still_is_a_rollout` proves the *general* rule this port implements - matching `cancellai.py`'s `iter_files` precisely: a symlinked *directory* is excluded from descent, a symlinked *file* is still processed as a file (unfiltered, matching the reference), and a discovered rollout's `size_bytes` always comes from `entry.metadata()` (lstat-equivalent - the symlink's own size), never the resolved target's size | PASS |
| Pipe-deadlock robustness (not a numbered SI, but a real adversarial-filesystem/process class this CR3 story's own subprocess probe introduces) | A fake CLI writing >64KiB before exiting, and a fake CLI that never exits | `a_fake_cli_with_large_output_does_not_deadlock`, `a_fake_cli_that_hangs_is_killed_and_reported_as_a_probe_failure_not_a_hang` (originally hung the test suite for 60+ seconds before this story's fix - see Residual risks for what the fix does and does not close) | PASS |

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
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check
```

`cargo test -p cancellai-provider-codex` runs 27 unit tests plus 5 fixture-parity integration
tests, all green in ~1s (after the pipe-deadlock/hang fix below - before it, the hang test
alone took 60+ seconds). `cargo test -p cancellai-provider-api` runs 32 tests (2 new:
`root_fingerprint`'s `derive_root_confidence` tests). No regression anywhere else in the
workspace.

## Compatibility

- `native_delete.rs`'s `Command`/threading/`PATH`-resolution code is written against `std`
  only; the `#[cfg(unix)]`-gated tests (fake CLI shell scripts, symlink cases) do not run on
  Windows, matching this workspace's existing Windows-identity residual posture (E03-S01).
- No new external dependency in this story (both `serde_json` used by `session.rs` and
  `cancellai-provider-api`'s `unicode-normalization` were already added in E05-S03).

## Performance / operability

- `discover_codex_sessions` walks `sessions/`/`archived_sessions/` recursively with no entry
  budget (unlike fingerprinting's `MAX_ROOT_PROBE_ENTRIES`) - matching `cancellai.py`'s own
  `iter_files`, which is likewise unbounded for this specific call. `read_codex_parent_session_id`
  is bounded (10 lines / 512KiB) per rollout, matching the reference exactly.
- `codex_delete_supported`'s probe has an 8-second deadline (`cancellai.py`'s own `timeout=8`)
  enforced via a poll loop plus background reader threads, not a blocking `Command::output()`
  call - `std::process` alone cannot express this timeout.

## Documentation updated

- `docs/architecture/PROVIDER_MODEL.md` - new paragraph under "Native adapter" (the story's
  declared documentation impact).

## Residual risks

- **`child.kill()` does not kill a grandchild process the probed binary may have spawned**
  (e.g. a shell script's own `sleep` child). `std` has no safe, dependency-free process-group
  kill (a raw `libc::killpg` call would need `unsafe`, forbidden workspace-wide by ADR-0015).
  On a probe timeout, this function kills only the direct child and returns promptly
  (`ProbeFailed`) without waiting for a surviving grandchild - correct for this function's own
  control flow (verified: the fix for the 60-second test hang was exactly not blocking on
  this), but the grandchild and its still-open pipe can outlive the call, leaking one or two
  reader threads for the grandchild's remaining lifetime. This is a narrow, documented,
  non-hanging residual, not an unbounded resource leak.
- Same residuals as E05-S03, applying identically here: no `scripts/diff_harness.py`
  JSON-document comparison yet (E06 scope); `ScopeCompleteness`/`KnowledgeConfidence` not yet
  wired from this adapter's evidence into `cancellai-safety`'s authority lattice;
  `coverage_state`'s literal-name reporting bucket is not ported (only the safety-relevant
  `protected_component` barrier is).
- `ActivityState`/`ProjectAttribution`/`RetentionCapability` report `Unsupported` with evidence
  stating what is deferred (`active_processes` is not ported; Codex has no project-attribution
  concept in the Python reference; Codex has no retention-configuration write path at all,
  unlike Claude's `configure_claude_retention`) - honest deferral, not a partial/faked
  implementation.

## Verifier verdict

PENDING - epic E05 review runs once every story in E05 is `ready_for_review` (at most twice
per epic, per ADR-0014).
