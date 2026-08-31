# Evidence Packet - E05-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E05)
- Change Risk: CR3
- Spec version/commit: `rust/crates/cancellai-provider-api/src/protection.rs` (new),
  `rust/crates/cancellai-provider-api/src/root_probe.rs` (new),
  `rust/crates/cancellai-provider-claude/src/{fingerprint,protected_names,session,lib}.rs`
  (new/rewritten), `rust/crates/cancellai-provider-claude/tests/claude_fixture_parity.rs`
  (new) as added in this change

## Outcome

PASS

## Scope

Ports `cancellai.py`'s Claude-specific discovery/classification/session-relationship logic to
Rust and wires it behind E05-S01's `ProviderCapabilities` contract: root fingerprinting
(`ROOT_MARKERS["claude"]`/`fingerprint_root`), the protected-name barrier
(`canonical_name`/`protected_component`, shared with a future Codex adapter via
`cancellai-provider-api`), and flat project/session discovery
(`discover_claude_sessions`). Deliberately out of scope: `build_plan`/action selection
(cutoff, `--keep-latest`, coverage buckets), `execute_plan`/mutation, `configure_claude_retention`
(a write path), and `active_processes` (OS process detection) - these are PLAN/EXECUTE-stage or
policy-dependent concerns `docs/architecture/TARGET.md`'s OBSERVE→CLASSIFY→RESOLVE→PLAN→
REVALIDATE→EXECUTE pipeline assigns to later stories (E06/E11/E12), not to a CLASSIFY-stage
provider adapter. `cancellai-provider-codex` is untouched (still the E02-S01 skeleton); its own
port is E05-S04.

Two tool-agnostic utilities moved to `cancellai-provider-api` rather than being duplicated in
`cancellai-provider-claude` alone, since `cancellai.py` itself shares them across both tools
(`protected_names_for`/`canonical_name`/`protected_component` take `protected_names` as a
parameter, and every `_is_json_object`/`_is_jsonl_of_objects`/`_is_nonempty_file`/
`_contains_uuid_named_jsonl` validator is tool-agnostic in the Python reference too):
`protection.rs` (added `unicode-normalization` as a new workspace dependency, MIT OR
Apache-2.0, ADR-0015 allow-list) and `root_probe.rs`.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Reference fixtures match normative Python contract | `tests/claude_fixture_parity.rs` reproduces five `tests/fixtures/recipes.py::build_claude_*` recipes by hand (byte-for-byte matching the recipe source) and asserts this adapter's fingerprint markers/session discovery against values copied character-for-character from the committed `tests/fixtures/characterization/claude-*.characterization.json` files: `claude-normal-session`, `claude-active-data` (markers, session count/id), `claude-protected-state` (markers, every protected name reported protected), `claude-partial-tree` (all 3 sessions still reported despite a locked companion, which is marked `degraded_companions` rather than silently dropped or silently succeeding), `claude-symlink-protected-name` (markers exclude the case-variant symlink, `projects/` correctly `Unavailable`, and the symlink is still caught by `protected_component`). All 5 pass. Documented residual: this is fixture-recipe parity, not yet a `scripts/diff_harness.py` JSON-document comparison (see Residual risks). | PASS |
| AC2 - Memory/settings/plugin protected classes are explicit artifacts or exclusions with evidence | `CLAUDE_PROTECTED_NAMES` is a verbatim copy of `cancellai.py`'s list (settings/keybindings/plugins/skills/agents/commands/rules/workflows/output-styles/agent-memory). `claude_protected_component`/`ClaudeProvider::protection` classify any path against it with an explicit `ProtectionOutcome::Protected{matched_name}` (never a bare boolean) - `every_documented_protected_name_is_actually_protected_at_the_top_level` and the `claude-protected-state`/`claude-symlink-protected-name` fixture tests are the evidence. | PASS |
| AC3 - Unknown layouts downgrade to inspection-only | `RootConfidence::Unknown` maps to `SupportState::Unsupported` with `authority_ceiling: Some(AuthorityLevel::Observe)` in `ClaudeProvider::capability` for `Detect`/`FingerprintRoot` - `ac1_an_unknown_layout_downgrades_every_root_dependent_capability_to_inspection_only` asserts this directly against an empty candidate root. `RootConfidence::Low` similarly caps at `AuthorityLevel::Recommend` (not full inspection-only, but still non-mutating), matching the graduated `default > high > low > unknown` vocabulary rather than collapsing everything non-default to one bucket. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 (protected/unknown state is non-destructive) | An unknown-layout root and a protected top-level entry, both checked for whether this adapter's evidence would let a later authority computation treat them as mutable | This adapter performs no mutation itself (pure reads only - `scripts/check_mutation_boundary.py` still passes with zero new references) and reports `Unsupported`/`AuthorityLevel::Observe` for unknown layouts and an explicit `Protected` outcome for protected entries; it supplies the evidence a future PLAN-stage `effective_authority` call would consume as `ArtifactAuthorityCeiling`/`ConfidenceAuthority` inputs (not yet wired end-to-end - see Residual risks, matching E04-S03's own recorded residual for `ScopeCompleteness`) | PASS (evidence-supply half only; see residual) |
| SI-004 (unknown provider layout/version reduces capability) | `an_empty_directory_has_unknown_confidence_and_no_markers`, `ac1_an_unknown_layout_downgrades_every_root_dependent_capability_to_inspection_only` | `RootConfidence::Unknown` → `SupportState::Unsupported` regardless of the provider name/id being recognized as "claude-code" - capability is never assumed from identity alone (reusing E05-S01's AC1 contract) | PASS |
| SI-006 (protected-name/category barriers are defense in depth, applied here) | `claude-symlink-protected-name`: a case-variant symlink ("Plugins") of a protected name ("plugins"), pointing outside the root | `si006_a_case_variant_of_a_protected_name_is_still_protected` (in `cancellai-provider-api`) and `claude_symlink_protected_name_matches_the_committed_characterization`'s own protection assertion both confirm the barrier fires; checked lexically *before* symlink resolution specifically so a protected entry that is itself a symlink cannot escape detection by resolving outside the relative-path computation | PASS |

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

`cargo test -p cancellai-provider-api` runs 28 tests (12 new: 8 `protection`, 8 `root_probe` -
some overlap with existing capability tests); `cargo test -p cancellai-provider-claude` runs 17
unit tests plus 5 fixture-parity integration tests, all green; no regression anywhere else in
the workspace (full `cargo test --workspace` count unchanged elsewhere).

## Compatibility

- `unicode-normalization` is a new workspace dependency (`cancellai-provider-api` only, so
  far); `cargo deny check` passes (MIT OR Apache-2.0, on the ADR-0015 allow-list).
- Fixture-parity tests using `chmod`/Unix symlinks are `#[cfg(unix)]`-gated; the rest of the
  suite is platform-neutral. No Windows-specific behavior is claimed or tested here (matching
  the existing Windows-identity residual this workspace already carries from E03-S01).
- `fs::metadata`/`fs::read_dir`/`fs::symlink_metadata` are the only filesystem operations used
  anywhere in this change - no new dependency on `cancellai-platform`'s seams was needed, since
  every probe here is a direct, unconditional read (matching `cancellai.py`'s own direct
  `pathlib` usage for the same functions).

## Performance / operability

- `fingerprint_claude_root` performs at most 12 marker probes per call (one per
  `CLAUDE_ROOT_MARKERS` entry); `contains_uuid_named_jsonl` (the `projects` marker probe) is
  bounded by `MAX_ROOT_PROBE_ENTRIES` (2000 files), matching `cancellai.py`'s own
  pre-authority-probe budget (C-11).
- `discover_claude_sessions` walks `projects/<project>/*.jsonl` plus, for each session, its own
  companion payload directory - no unrelated subdirectory is ever walked (mirrors
  `cancellai.py`'s exact recursion boundary, verified by
  `a_project_subdirectory_that_is_a_symlink_is_never_walked`).

## Documentation updated

- `docs/architecture/PROVIDER_MODEL.md` - new paragraphs under "Root fingerprinting" and
  "Native adapter" (the story's declared documentation impact).

## Residual risks

- **Not yet run through `scripts/diff_harness.py`.** That harness compares two
  JSON_CONTRACTS-conformant *documents*; producing one from this adapter would require a full
  OBSERVE+CLASSIFY+serialization pipeline that does not exist yet (E06 Rust CLI Parity and
  Cutover is where a real inventory document gets assembled and emitted). This story's
  differential proof is fixture-recipe parity instead (see AC1 evidence) - documented as a
  narrower, but still genuine and reproducible, form of differential checking. Closing this
  residual is E06 scope, not a defect in this story.
- **`ScopeCompleteness`/`KnowledgeConfidence` are not yet wired from this adapter's
  `degraded_companions` signal.** `SessionDiscoveryResult.degraded_companions` names which
  companion directories could not be fully read, but nothing yet feeds that into
  `cancellai-safety::authority`'s `ConfidenceAuthority`/`LifecycleAuthority` constraints - the
  same "supplies evidence, not yet wired to the authority lattice" residual E04-S03 already
  recorded for `ScopeCompleteness`.
- **`canonical_name`'s casefold is Rust's `to_lowercase()`, not Python's full Unicode
  `str.casefold()`.** Documented in `protection.rs`'s own doc comment: the two differ only for
  a handful of exotic characters (e.g. German ß); every protected name this workspace defines
  is plain ASCII, where they agree exactly, so no NORMATIVE fixture behavior is affected.
- **`protected_component`'s resolved-symlink view is skipped (not reported "unresolvable")
  when `canonicalize()` fails**, rather than reproducing `cancellai.py`'s rarer
  `"<unresolvable>"` fail-closed sentinel for a genuine `OSError` during resolution (a symlink
  loop, a permission error mid-resolution). The lexical view - checked first, unconditionally -
  is unaffected; this narrows only the *additional* resolved-view check, and this module is a
  classification/evidence signal, not itself the mutation safety boundary (that remains
  `cancellai-safety`'s `ApprovedRoot`/`BoundedPath`, SI-002/SI-003).
- **`coverage_state`/`coverage_report`'s literal-name reporting bucket (`status` command
  output) is not ported.** This story ports the safety-relevant protected-name barrier
  (`protected_component`), not the separate, non-safety-relevant top-level reporting
  classification `cancellai.py` uses for `status` - the two are independent mechanisms in the
  Python reference itself (confirmed directly: `claude-symlink-protected-name`'s coverage
  bucket reports the case-variant symlink as `unknown`, while `protected_component` still
  catches it for the actual barrier).
- **`ActivityState`/`NativeDeleteCapability`/`RetentionCapability` capabilities report
  `Unsupported`** with evidence stating what is deferred, rather than partial/fake
  implementations - `active_processes`, native delete (Claude has none in the Python
  reference), and `configure_claude_retention` (a write path) are out of this story's scope.

## Verifier verdict

PENDING - epic E05 review runs once every story in E05 is `ready_for_review` (at most twice
per epic, per ADR-0014).
