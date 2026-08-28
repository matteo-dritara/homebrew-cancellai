# Evidence Packet - E02-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E02)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-model/src/diagnostic.rs` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Errors separate invalid input, safety block, incomplete inventory, compatibility failure, mutation failure, and internal fault | `ErrorCategory` (`cancellai-model/src/diagnostic.rs`) has exactly these six variants, exhaustively matched (no wildcard arm) by both `exit_code()` and `code()` - adding a category without updating both is a compile error. `docs/architecture/DOMAIN_MODEL.md`'s new "Diagnostics" section documents the six categories and their stable exit-code grouping, generalizing (not reusing 1:1) the coarser Python exit taxonomy in `AS_IS.md`. `exit_codes_are_stable_and_distinguish_the_documented_severity_bands` (golden test) asserts the exact exit code for every category. | PASS |
| AC2 - Human and JSON diagnostics share stable error codes | `Diagnostic` stores only `category` and `message` - no separate/duplicated code field. `Display` and `serde::Serialize` both read the code exclusively through `ErrorCategory::code()` (`Display` via `self.category.code()`; `Serialize` derives through `ErrorCategory`'s own manual `Serialize` impl, which calls `code()`). `human_and_json_diagnostics_share_the_same_stable_code` (golden test) asserts both renderings embed the identical code string for every category, and `display_and_serialize_never_diverge` (unit test) asserts the same at the `ErrorCategory` level directly. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story; `Diagnostic`/`ErrorCategory` are pure data types with no mutation capability.

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
```

All passed: 179 Python tests, 22 subtests, all governance checks; Rust `fmt`/`clippy -D
warnings`/`check`/`cargo deny check` clean; `cargo test --workspace` includes 6 new tests (2
unit in `diagnostic.rs`, 4 golden in `tests/diagnostic_golden.rs`), all passing. This is the
"golden diagnostic tests across all categories" the story's verification contract names -
one committed JSON fixture per category under `cancellai-model/tests/golden/`, generated (not
hand-written) by `cargo run --example print_golden` and compared byte-for-byte by
`json_diagnostics_match_their_golden_document`.

Falsification-tested directly: a golden fixture (`safety_block.json`) was tampered
(`SAFETY_BLOCK` -> `TAMPERED`) and `cargo test --test diagnostic_golden` failed with an exact
expected-vs-actual diff naming the file to regenerate; the fixture was restored and the test
suite passed again. This proves the golden comparison actually catches drift, not only that
the six committed fixtures happen to match today.

This is also the first real (non-`use ... as _`) code in the workspace, and the first real
external dependency (`serde`/`serde_json`, added to `[workspace.dependencies]` in
`rust/Cargo.toml`): `cargo deny check` passed against the full resulting dependency tree
(including transitive dependencies pulled in by `serde_derive`'s proc-macro machinery),
confirming the ADR-0015 license/source policy `scripts/check_rust_workspace.py`/E02-S02
established works end-to-end against a real graph, not only the synthetic fixtures used to
prove it in isolation.

## Compatibility

- No platform-specific behavior. `serde`/`serde_json` are pure-Rust, `no_std`-agnostic in the
  features used here, and already verified against the license/source policy.

## Performance / operability

- `cargo test -p cancellai-model` (6 tests, all in-memory) completes in well under a second.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md` - new "Diagnostics" section (the story's declared
  documentation impact).
- `docs/CLI.md` - deliberately **not** edited: it is 100% generated from the frozen Python
  reference's argparse definitions (`scripts/gen_docs.py`, `AGENTS.md`'s Python reference
  freeze) and has no hand-editable section; hand-editing it would be reverted by the
  generator and flagged by `gen_docs.py --check`. See Residual risks.
- `docs/security/SUPPLY_CHAIN.md`/`AGENTS.md` are unaffected by this story (already updated
  by E02-S02 for the dependency-policy mechanism this story's new dependency exercises for
  real for the first time).

## Residual risks

- **The story's own declared documentation impact (`docs/CLI.md`) could not be satisfied
  literally**, because that file is machine-generated from `cancellai.py`'s argparse
  definitions and `cancellai.py` is frozen (E01-S06) with no typed error model of its own to
  generate this taxonomy from. The cross-reference instead lives in
  `docs/architecture/DOMAIN_MODEL.md`'s new Diagnostics section, and this evidence packet
  states explicitly why `docs/CLI.md` was not touched, rather than silently substituting one
  documentation target for another. A reviewer should confirm this substitution is
  reasonable rather than a scope gap - the actual CLI-facing documentation for this taxonomy
  will exist once E06 (Rust CLI Parity and Cutover) produces a Rust CLI reference doc.
- `Diagnostic` is not yet wired into any real error path (no crate raises one yet - there is
  no fallible logic in the workspace to raise it from). This story defines the shared type;
  using it as the actual `Result<T, Diagnostic>` error type across crates is expected as
  those crates gain real logic in later stories/epics, not part of this story's scope.
- `std::error::Error` is implemented for `Diagnostic` but `source()` is not overridden (no
  underlying cause is modeled yet); this is adequate for a story with nothing yet to wrap,
  and revisitable once real fallible operations (e.g. filesystem I/O in `cancellai-inventory`)
  need to preserve an underlying `std::io::Error` as a diagnostic's cause.

## Verifier verdict

PENDING - epic E02 review runs once every story in E02 is `ready_for_review` (at most twice per epic, per ADR-0014).
