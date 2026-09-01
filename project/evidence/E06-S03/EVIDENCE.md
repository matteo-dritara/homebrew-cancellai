# Evidence Packet - E06-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E06)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-cli/tests/install_rollback.rs` (new),
  `docs/development/MIGRATION_PYTHON_RUST.md` (M7 section), `docs/RELEASING.md` (new "Beta
  side-by-side (E06)" section)

## Outcome

PASS

## Scope

Implements "Define side-by-side invocation, data compatibility, and rollback from Rust
candidate to Python reference during beta." Epic E17 owns the canonical cross-platform release
factory (`docs/RELEASING.md` "Target Rust release factory" - packages, SBOM, signed provenance,
`dist`/cargo-dist); this story's "install/rollback smoke tests" verification contract is scoped
to what actually exists during the *beta* period E06 itself defines: `cancellai-cli` built from
source, no packaged distribution yet.

Two of this story's real questions turned out to already have concrete, verifiable answers
rather than needing new design:

- **"No local state migration is irreversible"**: there is no cancellAI-owned local state to
  migrate in *either* engine today - `cancellai-store` (the crate `docs/architecture/
  PERSISTENCE_MODEL.md` reserves for a future current-state store) remains the empty skeleton
  it has been since E02-S01, and `cancellai.py` is itself a stateless scan-on-demand script.
  This AC is not satisfied by asserting that fact; `install_rollback.rs` proves it by
  filesystem snapshot: every read-only command (including `clean --dry-run`) leaves the entire
  `$HOME` tree byte-for-byte unchanged, and a real `clean` removes exactly the one artifact it
  planned to delete and creates or touches nothing else anywhere.
- **Rollback mechanism**: `cancellai` (Python's installed command, `pyproject.toml`) and
  `cancellai-cli` (this crate's package name) are different binary names sharing no install
  path - proven directly, not just stated, by asserting the built test binary's own file stem.
  "Rollback" during beta is therefore not a migration to undo at all: a user who stops invoking
  `cancellai-cli` has changed nothing about their existing Python install.

The "install smoke test" the verification contract names is scoped honestly to what a
beta-period, unpackaged binary can actually be tested for: it behaves correctly regardless of
its invocation working directory (no relative-path/cwd assumption a real installed binary,
invoked from an arbitrary shell location, would violate). Full packaged-install verification
(checksums, SBOM, signed provenance, platform installers) is out of scope until E17 builds the
release factory those checks require - this is stated explicitly in both the test file's module
doc and the documentation updates below, not silently narrower than the story name suggests.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Beta users can identify active engine/version | `cancellai-cli version` (already implemented at E06-S01) prints `cancellai-cli <version>`. Test: `install_rollback.rs::version_output_identifies_this_as_the_rust_engine_with_a_concrete_version` asserts the output names the engine explicitly and includes a version token, distinct from Python's installed `cancellai` command name. | PASS |
| AC2 - No local state migration is irreversible before cutover | No local state exists to migrate in either engine (see Scope). Tests: `install_rollback.rs::every_read_only_command_leaves_no_trace_anywhere_under_home` (filesystem snapshot equality before/after 5 different read-only invocations, including `clean --dry-run`), `::a_real_clean_touches_only_the_provider_artifact_it_deletes_nothing_else_anywhere` (a real, mutating `clean` run removes exactly the one artifact and nothing else). | PASS |

## Verification Commands

```text
# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Python governance (repository-wide, unaffected by this story - re-run for completeness)
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

All green. `cargo test --workspace` includes 5 new tests in
`rust/crates/cancellai-cli/tests/install_rollback.rs`; no existing test was modified.

## Compatibility

- The filesystem-snapshot tests (`snapshot()`) walk the entire synthetic `$HOME` tree
  recursively and compare `BTreeSet<PathBuf>` equality - platform-independent (no reliance on
  directory-entry ordering, which differs across filesystems/OSes).
- `the_built_binary_runs_correctly_regardless_of_its_invocation_directory` explicitly changes
  the process's working directory to somewhere unrelated to `$HOME` before invoking the binary
  - proves no `cwd`-relative path assumption exists, the one property this beta-period smoke
  test can meaningfully assert about "installability" without a real installer to test against.

## Performance / operability

- All 5 new tests run in well under a second combined (no cargo build inside the test process -
  they use the already-built `CARGO_BIN_EXE_cancellai-cli` Cargo provides).

## Documentation updated

- `docs/development/MIGRATION_PYTHON_RUST.md` (declared documentation impact) - M7 section now
  states concretely what E06-S03 proved and points at the specific tests.
- `docs/RELEASING.md` (declared documentation impact) - new "Beta side-by-side (E06)" section
  distinguishing this story's scope from Epic E17's canonical release factory, and stating the
  rollback mechanism plainly for a reader who has not read the ADR/story chain.

## Residual risks

- No real packaged installer exists yet to smoke-test end-to-end (checksum verification,
  platform-specific install paths, upgrade-in-place behavior) - explicitly deferred to E17, not
  silently claimed here. A future E17 story's own install smoke tests should not be considered
  satisfied by this one.
- "No local state to migrate" is a true statement about *today's* implementation; the moment a
  future story adds a `cancellai-store` type with real content, this story's proof (an empty
  snapshot diff) stops applying and that future story inherits the actual migration-safety
  obligation C-10 was written for.
- `cancellai-cli`'s `configure` command *does* write to a file outside `cancellai`'s own
  tracked state (Claude Code's own `settings.json`) - this is deliberately excluded from the
  "no local state" claim (it is vendor configuration, not cancellAI-owned state, per E06-S01's
  own `configure_claude_retention` doc comment) and is not exercised by this story's snapshot
  tests, which only cover `status`/`inspect`/`plan`/`clean`.

## Round 1 verifier verdict

PASS_WITH_RESIDUALS on its own merits (`project/evidence/E06-VERIFIER-REVIEW.md`,
2026-09-01), but blocked from closure because its dependency, E06-S02, failed round 1. No defect
in this story's own scope was found. E06-S02's round-1 defects are now repaired (see that
story's evidence packet); this story's own side-by-side/rollback tests
(`cancellai-cli/tests/install_rollback.rs`) were re-run unchanged as part of `cargo test
--workspace` after every E06-S01/E06-S02 repair above and remain green throughout - this story
required no code changes of its own.

## Verifier verdict

Round 1: PASS_WITH_RESIDUALS, blocked by E06-S02 (now repaired). Round 2:
PASS_WITH_RESIDUALS on its own merits again (`project/evidence/E06-VERIFIER-REVIEW-ROUND2.md`,
2026-09-01: "the source-built side-by-side smoke contract still passes... No cancellAI-owned
state exists in either engine to migrate"), blocked only by its E06-S02 dependency's round-2
FAIL - no defect in this story's own scope was found in either round.

## Closure - 2026-09-01, owner-authorized

Same owner-authorized closure as `E06-S01`'s evidence packet records in full (same chat session,
same date, same instruction). This story is CR2 (no CR4 Safety Verdict requirement applies) and
required no code changes of its own in either review round - it unblocks the moment its sole
dependency, E06-S02, closes. `cargo test --workspace` was re-run after E06-S01/E06-S02's
round-2 repair (see their own evidence packets) and after the Windows-symlink test addition
recorded in E06-S01's evidence; `rust/crates/cancellai-cli/tests/install_rollback.rs`'s five
tests remain unchanged and green throughout.

Status: `done`.
