# Evidence Packet - E13-S06

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E13 epic review
- Change Risk: CR3 (declared CR3 at planning time in `project/epics/E13.json`, matching
  E13-S04's own level for the identical safety obligation, SI-026; the actual diff adds no new
  cross-crate dependency - `cancellai-store`'s `Cargo.toml` is unchanged - so no reclassification
  applies)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "cancellAI-owned local-state
  root (E13-S06)"; `docs/security/SAFETY_INVARIANTS.md` SI-026 ("cancellAI reset/self-budget
  cannot target provider payload"); `project/evidence/E13-VERIFIER-REVIEW-ROUND3.md` (the
  reproduction this story closes)

## Outcome

PASS

## Scope

`cancellai_store::local_state_root::LocalStateRoot` (new module, `rust/crates/cancellai-store/
src/local_state_root.rs`): the one public, reviewed way to establish cancellAI's own local-state
root directory (`LocalStateRoot::resolve`, creating it if absent and canonicalizing it once).
`CurrentStateStore::open`, `EventLedger::open` and `AnalyticalMemory::open` each change from
`pub fn open(path: &Path)` to `pub fn open(root: &LocalStateRoot)`, deriving their own database's
location by joining a filename this crate alone fixes per layer
(`current_state.sqlite3`/`event_ledger.sqlite3`/`analytical_memory.sqlite3`) - never a path a
caller supplies directly. Each layer's previous `open` body is preserved unchanged under a new
`pub(crate) fn open_at_path(path: &Path)`, which production `open` calls after deriving the path,
and which this crate's own existing unit tests now call directly wherever they previously called
`open` with an explicit path (in-memory tests via `open_in_memory` are unchanged). No new crate
dependency, no schema change, no new SQL.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "If a caller supplies a path outside cancellAI's own resolved local-state root, the production `open()` entry point refuses to construct a handle for it, regardless of what the file at that path contains - ... but because the production API never constructs one for any other location." | Structural: `CurrentStateStore::open`/`EventLedger::open`/`AnalyticalMemory::open` take `&LocalStateRoot`, not `&Path`; each derives its file location only via `LocalStateRoot::path_for(<fixed filename>)`, a `pub(crate)` method whose filename argument is always a crate-internal `&'static str` constant, never caller/external input. There is no code path by which any caller, inside or outside this crate, can make a production `open()` construct a handle at any location other than `<resolved root>/<fixed filename>`. `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` (added to `lib.rs`, `ledger.rs`, and `rollup.rs`, one per layer) demonstrates this concretely: a full schema-and-marker-bearing mimic file placed at the fixed filename in a sibling directory is left with its one row intact after `open(&root)` + `reset()` against the real, resolved root. | PASS |
| AC2 - "The local-state root itself is established by exactly one reviewed resolution path, never accepted as an arbitrary caller-supplied argument on the production entry point." | `LocalStateRoot`'s only field is private and its only public constructor is `LocalStateRoot::resolve` - no `Default`, no `From<PathBuf>`, no other way to construct a value of this type from outside the module. `resolve` canonicalizes its argument once; the resulting `LocalStateRoot` is what every layer's `open()` takes, never a raw path. | PASS |
| AC3 - "Existing unit tests keep exercising `open()`/`reset()` directly (in-memory or a test-only path constructor), so this change costs no test coverage or ergonomics." | Every pre-existing test that previously called `CurrentStateStore::open(&path)`/`EventLedger::open(&path)`/`AnalyticalMemory::open(&path)` now calls the crate-private `open_at_path(&path)` instead, with identical bodies and identical assertions - zero test deletions, zero test behavior changes. `cargo test --workspace`: 114 passed (cancellai-store), up from 113 before this story (one new boundary test added, see below), 0 failed. | PASS |
| AC4 - "The compiled-in identity marker E13-S04 added stays in place as a corruption/migration sanity check; this story does not remove it, only stops treating it as an authorization boundary." | `STORE_IDENTITY_MARKER`/`LEDGER_IDENTITY_MARKER`/`ROLLUP_IDENTITY_MARKER` and `verify_*_identity`/`verify_*_identity_before_migrating` are unchanged in this diff. `open_refuses_a_provider_owned_file_that_only_mimics_this_crates_schema` and the two sibling tests (ledger/rollup) - the round-2-era marker-absent mimicry regressions - still pass unchanged via `open_at_path`. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Adversarial test: a path outside the resolved local-state root is refused before any file I/O against it occurs, independent of what that path's content contains." | `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` (lib.rs/ledger.rs/rollup.rs): a full marker-and-schema-bearing mimic at a path outside the root (a sibling directory, at the exact reserved filename) is never opened, read, or written by `open(&root)` - the resulting store is fresh/empty, and the mimic's one row survives `reset()` on the real, freshly opened store untouched. | PASS |
| "Regression test: the three prior mimicry reproductions (E13-VERIFIER-REVIEW-ROUND2.md, -ROUND3.md) stay refused under the new boundary, not only under the marker check they originally targeted." | The round-2 no-marker mimicry tests (`open_refuses_a_provider_owned_file_that_only_mimics_this_...schema`, one per layer) still pass via `open_at_path`, unchanged. The round-3 full-marker mimicry scenario is now covered permanently (it was previously only a temporary, removed reproduction per `E13-VERIFIER-REVIEW-ROUND3.md`) by the new `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` tests above, run against the *production* `open(&root)` path rather than the old `open(path)`. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-026 ("cancellAI reset/self-budget cannot target provider payload") | A provider-owned or fully-marker-mimicking file at a path outside cancellAI's resolved local-state root, supplied via any means a caller could reach | `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` (three layers); closes E13-S04's round-3 reproduction (`provider_rows_after_reset=0` against the old `open(path)` API) - the equivalent reproduction against the new `open(&root)` API leaves the mimic's row count at 1 | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 114 cancellai-store tests (was 113), 0 failed
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; pre-existing unmatched-license/duplicate-dependency warnings reported (unchanged from prior stories)
python3 scripts/check_mutation_boundary.py check                          # PASS
python3 scripts/check_rust_workspace.py check                             # PASS
python3 scripts/check_docs.py check                                       # PASS
pre-commit run --all-files                                                # PASS (all 32 hooks, including project_os check/generate drift, characterize/diff_harness/rust_python_parity, evidence/skill/process-metrics gates)
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither; CI's existing per-platform `cargo check`/quality matrix covers `cancellai-store`
on macOS, Linux and Windows as it already does for E13-S01 through E13-S05.

## Compatibility

- No schema change, no new dependency, no wire-format change. `cancellai-cli`/`cancellai-guardian`
  still do not call any of this from source (unchanged from E13-S04's own note) - no live caller
  is wired to `LocalStateRoot` yet; that remains later orchestration scope, matching every prior
  E13 story's own "primitive delivered, no orchestrator yet" precedent.

## Performance / operability

- `LocalStateRoot::resolve` is one `create_dir_all` plus one `canonicalize` call, paid once per
  process, not per operation - no measurable cost added to any existing hot path.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md`: new "cancellAI-owned local-state root (E13-S06)"
  subsection under "Self-budget", including the disclosed residual.
- `docs/architecture/TARGET.md`: new paragraph on the deliberate `cancellai-store` /
  `cancellai-safety`/`cancellai-platform` isolation this story preserves rather than relaxes.
- `CHANGELOG.md`: `### Security` entry under `[Unreleased]`.

## Method defects

- none

## Residual risks

- **The resolved root directory's identity is not re-verified on every `open()`.**
  `LocalStateRoot::resolve` canonicalizes once; unlike `cancellai-safety::ApprovedRoot::establish`'s
  device/inode binding for provider roots, nothing re-checks that the canonicalized directory still
  refers to the same object at open time. Reusing `ApprovedRoot` was considered and rejected because
  it would add a `cancellai-store` -> `cancellai-safety` dependency, contradicting this crate's own
  documented isolation (`docs/architecture/PERSISTENCE_MODEL.md`, `cargo_toml_declares_no_
  dependency_on_cancellai_safety`). A root directory replaced by a symlink between `resolve()` and a
  later `open()`, or a symlink placed at the fixed filename *inside* an already-resolved root, are
  therefore not defended against by this story. Both require an attacker who already has write
  access to a location cancellAI itself resolved as its own - a different threat from the one round
  3 reproduced (an arbitrary path, entirely outside cancellAI's storage, supplied to the old
  `open(path)` API, which the production API can no longer accept at all). Disclosed in
  `docs/architecture/PERSISTENCE_MODEL.md`'s own "Disclosed residual" and in
  `local_state_root.rs`'s module doc; no story currently carries closing it, since it needs the
  same device/inode-binding machinery this story deliberately declined to import.
- **No live caller wires `LocalStateRoot::resolve` to a real, platform-specific state directory
  yet.** This story only changes the crate's own API boundary; choosing where cancellAI's local
  state actually lives on disk (an XDG-style data directory, a CLI flag, etc.) is orchestration
  scope no story has picked up yet, matching every earlier E13 story's own "primitive delivered, no
  orchestrator yet" precedent (`docs/architecture/PERSISTENCE_MODEL.md`).

## Verifier verdict

(pending independent review)
