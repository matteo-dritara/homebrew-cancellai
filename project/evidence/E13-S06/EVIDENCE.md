# Evidence Packet - E13-S06

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 4 (of the new E13-S04+E13-S06 scope) - **FAIL**
  (`project/evidence/E13-VERIFIER-REVIEW-ROUND4.md`) - `LocalStateRoot::resolve`'s only public
  form still took an arbitrary caller-supplied directory, so a caller could mint a root over a
  real provider directory and reach the identical `reset()`-erases-provider-data outcome round 3
  reproduced; a second, independent symlink-at-the-fixed-leaf reproduction also passed. Repaired
  below. Round 5 (final, of this scope's two-round budget) pending
- Change Risk: CR3 (declared CR3 at planning time in `project/epics/E13.json`, matching
  E13-S04's own level for the identical safety obligation, SI-026; the actual diff adds no new
  cross-crate dependency - `cancellai-store`'s `Cargo.toml` is unchanged - so no reclassification
  applies)
- Spec version/commit: `docs/architecture/PERSISTENCE_MODEL.md` "cancellAI-owned local-state
  root (E13-S06)"; `docs/security/SAFETY_INVARIANTS.md` SI-026 ("cancellAI reset/self-budget
  cannot target provider payload"); `project/evidence/E13-VERIFIER-REVIEW-ROUND3.md` (the
  reproduction this story closes)

## Outcome

PASS after repair (see "Repair - round 4 independent review finding" below)

## Scope

`cancellai_store::local_state_root::LocalStateRoot` (new module, `rust/crates/cancellai-store/
src/local_state_root.rs`): the one public, reviewed way to establish cancellAI's own local-state
root directory. `LocalStateRoot::resolve_platform_default()` - taking no path argument at all -
computes cancellAI's own configured location itself (`$CANCELLAI_HOME/state`, or
`$HOME/.cancellai/state`) and is the only public constructor; the original `resolve(dir: &Path)`
survives narrowed to `pub(crate)`, reachable only from this crate's own tests and from
`resolve_platform_default` itself (round-4 repair, see below).
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
| AC1 - "If a caller supplies a path outside cancellAI's own resolved local-state root, the production `open()` entry point refuses to construct a handle for it, regardless of what the file at that path contains - ... but because the production API never constructs one for any other location." | Structural: `CurrentStateStore::open`/`EventLedger::open`/`AnalyticalMemory::open` take `&LocalStateRoot`, not `&Path`; each derives its file location only via `LocalStateRoot::path_for(<fixed filename>)`. Round 4 found this alone insufficient in two ways, both now closed: (1) `path_for` now refuses a path that already exists as a symlink - `open_via_local_state_root_refuses_a_fixed_filename_symlink_planted_at_the_leaf` (one per layer) reproduces round 4's exact symlink-redirection attack and confirms refusal; (2) since `resolve_platform_default` is the only public way to *obtain* a `LocalStateRoot` at all, "outside cancellAI's own resolved root" is no longer a caller choice (see AC2). `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` (unchanged from before round 4) continues to demonstrate the sibling-directory case. | PASS |
| AC2 - "The local-state root itself is established by exactly one reviewed resolution path, never accepted as an arbitrary caller-supplied argument on the production entry point." | Round 4 found the original `LocalStateRoot::resolve(dir: &Path)` was that arbitrary caller-supplied argument, merely renamed - a public-API reproduction minted a root over a real provider directory and reached the same `reset()`-erases-provider-data outcome round 3 had already reproduced. **Repair:** `resolve_platform_default()` (no arguments) is now the only public constructor; it computes `$CANCELLAI_HOME/state` or `$HOME/.cancellai/state` itself and calls the now-`pub(crate)` `resolve(dir)` internally. A `compile_fail` doctest on `LocalStateRoot` proves `resolve(dir)` is unreachable from outside the crate. | PASS |
| AC3 - "Existing unit tests keep exercising `open()`/`reset()` directly (in-memory or a test-only path constructor), so this change costs no test coverage or ergonomics." | Every pre-existing test that previously called `CurrentStateStore::open(&path)`/`EventLedger::open(&path)`/`AnalyticalMemory::open(&path)` now calls the crate-private `open_at_path(&path)` instead, with identical bodies and identical assertions - zero test deletions, zero test behavior changes. `cargo test --workspace`: 114 passed (cancellai-store), up from 113 before this story (one new boundary test added, see below), 0 failed. | PASS |
| AC4 - "The compiled-in identity marker E13-S04 added stays in place as a corruption/migration sanity check; this story does not remove it, only stops treating it as an authorization boundary." | `STORE_IDENTITY_MARKER`/`LEDGER_IDENTITY_MARKER`/`ROLLUP_IDENTITY_MARKER` and `verify_*_identity`/`verify_*_identity_before_migrating` are unchanged in this diff. `open_refuses_a_provider_owned_file_that_only_mimics_this_crates_schema` and the two sibling tests (ledger/rollup) - the round-2-era marker-absent mimicry regressions - still pass unchanged via `open_at_path`. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Adversarial test: a path outside the resolved local-state root is refused before any file I/O against it occurs, independent of what that path's content contains." | `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` (lib.rs/ledger.rs/rollup.rs): a full marker-and-schema-bearing mimic at a path outside the root (a sibling directory, at the exact reserved filename) is never opened, read, or written by `open(&root)` - the resulting store is fresh/empty, and the mimic's one row survives `reset()` on the real, freshly opened store untouched. | PASS |
| "Regression test: the three prior mimicry reproductions (E13-VERIFIER-REVIEW-ROUND2.md, -ROUND3.md) stay refused under the new boundary, not only under the marker check they originally targeted." | The round-2 no-marker mimicry tests (`open_refuses_a_provider_owned_file_that_only_mimics_this_...schema`, one per layer) still pass via `open_at_path`, unchanged. The round-3 full-marker mimicry scenario is covered by `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk`. Round 4's own two reproductions (arbitrary-directory `resolve`, fixed-leaf symlink) are covered by the new `compile_fail` doctest and `open_via_local_state_root_refuses_a_fixed_filename_symlink_planted_at_the_leaf` (one per layer), respectively. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-026 ("cancellAI reset/self-budget cannot target provider payload") | A provider-owned or fully-marker-mimicking file at a path outside cancellAI's resolved local-state root, supplied via any means a caller could reach | `open_via_local_state_root_never_reaches_a_marker_bearing_mimic_elsewhere_on_disk` (three layers); closes E13-S04's round-3 reproduction (`provider_rows_after_reset=0` against the old `open(path)` API) - the equivalent reproduction against the new `open(&root)` API leaves the mimic's row count at 1 | PASS |
| SI-026 (round 4 repair) | Round 4's exact reproduction: `LocalStateRoot::resolve(&provider_dir)` followed by production `open`/`reset` erasing a copied marker-bearing provider ledger | Now refused at compile time - `resolve` is `pub(crate)`, proven unreachable by the `compile_fail` doctest on `LocalStateRoot` | PASS |
| SI-026 (round 4 repair) | Round 4's second reproduction: a fixed-filename symlink planted after a legitimate `resolve()`, redirecting production `open`/`reset` to a provider-owned file elsewhere | Refused - `path_for` returns `Err` for a symlinked leaf; `open_via_local_state_root_refuses_a_fixed_filename_symlink_planted_at_the_leaf` (three layers) reproduces and confirms | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                              # PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS
cd rust && cargo check --workspace --all-targets                          # PASS
cd rust && cargo test --workspace                                         # PASS - 133 cancellai-store tests, 0 failed (incl. 1 compile-fail doctest)
cd rust && cargo deny check                                               # PASS - advisories, bans, licenses, sources OK; pre-existing unmatched-license/duplicate-dependency warnings reported
python3 scripts/check_mutation_boundary.py check                          # PASS
python3 scripts/check_rust_workspace.py check                             # PASS
python3 scripts/check_docs.py check                                       # PASS
pre-commit run --all-files                                                # PASS (all 32 hooks, including project_os check/generate drift, characterize/diff_harness/rust_python_parity, evidence/skill/process-metrics gates)
```

Not run: the Windows-target and Linux-target cross-compiled clippy passes AGENTS.md calls out for
changes to `cancellai-platform` or anything moving the workspace-wide lint surface - this story
touches neither; CI's existing per-platform `cargo check`/quality matrix covers `cancellai-store`
on macOS, Linux and Windows as it already does for E13-S01 through E13-S05.

## Repair - round 4 independent review finding

Codex's round-4 review (`project/evidence/E13-VERIFIER-REVIEW-ROUND4.md`) FAILed both E13-S04 AC2
and E13-S06 AC1/AC2 with two independent reproductions:

1. **Arbitrary-directory `resolve`.** `LocalStateRoot::resolve(dir: &Path)` was this module's
   only public constructor, and `dir` was arbitrary caller input - the story's own AC2 text,
   restated with the parameter renamed rather than closed. A public-API reproduction called
   `resolve(&provider_dir)` for a real provider directory, then production `open()`/`reset()`
   erased a marker-bearing ledger event copied there.
2. **Fixed-filename symlink.** After a genuine, legitimate `resolve()` of a real root, a symlink
   planted at the fixed leaf filename (e.g. `event_ledger.sqlite3`) redirected production
   `open()`/`reset()` to a provider-owned file elsewhere, with no race required - the symlink was
   already in place before the call it defeated.

**Repair for (1):** `resolve_platform_default() -> Result<Self, LocalStateRootError>` (no
arguments) is the new, and only, public constructor. It computes cancellAI's own configured
location itself - `$CANCELLAI_HOME/state` if set, else `$HOME/.cancellai/state`, Unix-only for
now, matching `cancellai_cli::roots`'s own `$CODEX_HOME`/`$CLAUDE_CONFIG_DIR`-override precedent
- via a pure, injectable helper (`platform_default_base`) so tests never mutate real environment
variables (`std::env::set_var` is `unsafe`, which this workspace forbids outright). The original
`resolve(dir)` survives, narrowed to `pub(crate)`, called only by `resolve_platform_default`
itself and by this module's/the other layers' existing direct-path unit tests (this story's own
AC3). A `compile_fail` doctest on `LocalStateRoot` is the regression proving `resolve(dir)` is
unreachable from outside the crate.

**Repair for (2):** `LocalStateRoot::path_for(filename)` now returns `Result<PathBuf,
LocalStateRootError>` instead of a bare `PathBuf`, refusing when the joined path already exists
as a symlink (`std::fs::symlink_metadata` - which does not itself follow the link - checked
before any `open()` call reaches it). All three production `open()` functions and their `?`
propagate this; three new `From<LocalStateRootError>` impls (`LedgerError`/`StoreError`/
`RollupError`) make that ergonomic. This closes the demonstrated, no-race reproduction; it is not
full TOCTOU immunity, and is documented as such (see Residual risks).

## Compatibility

- No schema change, no new dependency, no wire-format change. `cancellai-cli`/`cancellai-guardian`
  still do not call any of this from source (unchanged from E13-S04's own note) - no live caller
  is wired to `LocalStateRoot` yet; that remains later orchestration scope, matching every prior
  E13 story's own "primitive delivered, no orchestrator yet" precedent.

## Performance / operability

- `LocalStateRoot::resolve` is one `create_dir_all` plus one `canonicalize` call, paid once per
  process, not per operation - no measurable cost added to any existing hot path.

## Documentation updated

- `docs/architecture/PERSISTENCE_MODEL.md`: "cancellAI-owned local-state root (E13-S06)"
  subsection rewritten for the round-4 repair (`resolve_platform_default`, the symlink-refusing
  `path_for`, and the narrowed residual).
- `docs/architecture/TARGET.md`: new paragraph on the deliberate `cancellai-store` /
  `cancellai-safety`/`cancellai-platform` isolation this story preserves rather than relaxes.
- `CHANGELOG.md`: `### Fixed` entry under `[Unreleased]` updated for the round-4 repair.

## Method defects

- none

## Residual risks

- **The resolved root directory's identity is not re-verified on every `open()`, and the
  symlink-leaf check has a narrow TOCTOU window.** `resolve_platform_default`/`resolve`
  canonicalize once; unlike `cancellai-safety::ApprovedRoot::establish`'s device/inode binding for
  provider roots, nothing re-checks that the canonicalized directory still refers to the same
  object at open time, and `path_for`'s symlink check is a check-then-open, not an atomic
  operation. Reusing `ApprovedRoot` was considered and rejected because it would add a
  `cancellai-store` -> `cancellai-safety` dependency, contradicting this crate's own documented
  isolation. Both remaining gaps - a root directory replaced by a symlink between `resolve()` and
  a later `open()`, and a race between `path_for`'s check and the `open()` call that follows it -
  require an attacker who already has write access to a location cancellAI itself resolved as its
  own, a fundamentally different, narrower threat than round 3/4 reproduced (which required no
  such access - an arbitrary path or a pre-existing symlink, reached from entirely outside
  cancellAI's own storage). Closing either fully needs an fd-relative/`openat`-style mechanism
  this crate does not have today; disclosed in `local_state_root.rs`'s module doc and
  `docs/architecture/PERSISTENCE_MODEL.md`; no story currently carries closing it.
- **`resolve_platform_default`'s own choice of location (`$CANCELLAI_HOME`/`$HOME/.cancellai`) is
  Unix-only and unwired.** No live caller (`cancellai-cli`/`cancellai-guardian`) invokes it yet -
  choosing when and how to call it, and Windows/macOS-idiomatic location conventions if this
  system needs them, is orchestration scope no story has picked up, matching every earlier E13
  story's own "primitive delivered, no orchestrator yet" precedent. Unlike the prior version of
  this residual, this is no longer standing in for AC1/AC2 being unmet - those are now closed
  regardless of whether a caller exists.

## Verifier verdict

Round 4: **FAIL** (Codex) - `project/evidence/E13-VERIFIER-REVIEW-ROUND4.md`. Repaired above;
round 5 (final) pending.
