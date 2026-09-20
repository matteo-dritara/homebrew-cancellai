# E13 Independent Verifier Review — Round 5

Review-Scope: epic
Round: 5
Verifier: Codex
Date: 2026-09-20
Review-Target: `f215d33..5b89534`

This final round reviews only E13-S04 and E13-S06: the new, twice-repaired local-state/root
scope. It does not re-judge the already-closed E13-S02, E13-S03, or E13-S05.

## Brief provenance

| Story | Brief-Checksum |
| --- | --- |
| E13-S04 | Brief-Checksum: 29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73 |
| E13-S06 | Brief-Checksum: f0e748293ab2bccb98162c27fa451b0f9850d2efebd4e04d4cdfb7b29bca2b31 |

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S04 | PASS_WITH_RESIDUALS | SI-026's round-4 arbitrary-root and fixed-leaf-symlink paths are closed by E13-S06's boundary repair. An independently built external crate cannot call `LocalStateRoot::resolve(&provider_dir)`: `cargo check` fails with E0624 because the method is `pub(crate)`. The same external public-API program resolves only its configured platform root, plants pre-existing symlinks at every fixed database leaf, and observes errors from `CurrentStateStore::open`, `EventLedger::open`, and `AnalyticalMemory::open`; the outside provider file remains byte-identical. AC1/AC3 and the empty-plan/evidence, marker-mimicry regressions remain covered by the passing workspace suite. Brief-Checksum: 29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73 |
| E13-S06 | PASS_WITH_RESIDUALS | The sole public constructor, `resolve_platform_default()`, derives `$CANCELLAI_HOME/state` or `$HOME/.cancellai/state`; it accepts no root/path parameter. `path_for` uses `std::fs::symlink_metadata`, so it classifies a pre-existing link without following it, and all three production openers propagate its error before `Connection::open`. The independent external symlink reproduction above closed all three production entry points; the committed `compile_fail` doctest and workspace test suite agree. Brief-Checksum: f0e748293ab2bccb98162c27fa451b0f9850d2efebd4e04d4cdfb7b29bca2b31 |

## Round-4 reproduction re-attempts

1. **Arbitrary caller-supplied directory:** I created an external temporary Cargo package that
   used only `cancellai_store`'s public API and attempted
   `LocalStateRoot::resolve(Path::new("/a/provider-controlled/directory"))`. Its independent
   `cargo check` failed as intended with E0624, "associated function `resolve` is private".
   Thus an external caller cannot mint a reset-capable root over a provider directory. The
   crate's committed `compile_fail` doctest also passed as part of `cargo test --workspace`.
2. **Fixed-filename symlink:** A separate external public-API program resolved only a supplied
   `$CANCELLAI_HOME/state`, wrote an outside provider sentinel, and planted pre-existing symlinks
   at `current_state.sqlite3`, `event_ledger.sqlite3`, and `analytical_memory.sqlite3`. Each
   production `open(&root)` returned `Err`; the provider sentinel was byte-identical afterward.
   Therefore no reset-capable handle was created and no provider file was reached. This is the
   round-4 no-race reproduction, re-attempted for all three layers.

## Further adversarial assessment

- **Configured root framing:** I agree that controlling `CANCELLAI_HOME` or `HOME` is a weaker,
  distinct threat model from the former arbitrary-argument public constructor. The public API
  itself selects the configured cancellAI location and cannot be asked to construct an arbitrary
  provider-root capability. If an attacker controls the process environment, it controls the
  application's configured storage location; that is not the round-3/4 API bypass.
- **Link check:** `path_for` uses `symlink_metadata`, not `metadata`, and therefore does not
  follow the pre-existing fixed-leaf symlink before rejecting it. It returns a `Result` which all
  three production openers propagate with `?` before SQLite opens the path.
- **Residual accuracy:** The module documentation, architecture document, and both evidence
  packets accurately retain the two unresolved cases: replacement of the resolved root with a
  symlink after resolution, and a leaf swap between `symlink_metadata` and SQLite's open. Both
  require write access inside (or replacement of) cancellAI's already-resolved state directory;
  they are narrower than a caller choosing any provider directory or supplying a pre-existing
  leaf symlink without such access. They are grounds for the recorded residuals, not a remaining
  round-4 reproduction.
- **Prior regressions:** `cargo test --workspace` passed the empty `plan_id`, empty
  `evidence_ids`, marker-mimicry, and production-root marker-mimicry tests. The refactor leaves
  the compiled-in markers as migration/corruption checks rather than authorization.

## Gate status

| Command | Result |
| --- | --- |
| External temporary package: `cargo check` calling public `LocalStateRoot::resolve` | PASS as a closure reproduction: intentional E0624 privacy error. |
| External temporary package: `CANCELLAI_HOME=/private/tmp/e13-round5-symlink cargo run --quiet` | PASS: all three public openers refused planted fixed-leaf symlinks and preserved the outside sentinel. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS — including 133 `cancellai-store` tests and the `LocalStateRoot` compile-fail doctest; scheduled benchmarks remain ignored by design. |
| `cd rust && cargo deny check` | PASS — advisories, bans, licenses, and sources OK; existing unmatched-license and duplicate-dependency warnings reported. |
| `python3 scripts/project_os.py check` | PASS before closure; rerun after generated control-plane update below. |
| `python3 scripts/check_process.py check` | PASS with recorded historic round-count warnings. |
| `python3 scripts/check_evidence.py check` | PASS with recorded pre-convention warnings. |
| `python3 scripts/verifier_handoff.py check` | PASS before closure; rerun after verdict record below. |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS — only approved mutation paths reference/delete filesystem targets. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `docs/architecture/PERSISTENCE_MODEL.md`
- `docs/architecture/TARGET.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `project/epics/E13.json`
- `project/evidence/E13-S04/EVIDENCE.md`
- `project/evidence/E13-S06/EVIDENCE.md`
- `project/evidence/E13-S04/VERIFIER_BRIEF.md`
- `project/evidence/E13-S06/VERIFIER_BRIEF.md`
- `project/evidence/E13-VERIFIER-REVIEW-ROUND3.md`
- `project/evidence/E13-VERIFIER-REVIEW-ROUND4.md`
- `rust/crates/cancellai-store/src/local_state_root.rs`
- `rust/crates/cancellai-store/src/lib.rs`
- `rust/crates/cancellai-store/src/ledger.rs`
- `rust/crates/cancellai-store/src/rollup.rs`

## Overall verdict

PASS_WITH_RESIDUALS for both E13-S04 and E13-S06. The concrete SI-026 counterexamples from
round 4 no longer reach provider-owned files through production `open()` entry points. The
disclosed resolved-root/TOCTOU cases remain owner-visible residual risks requiring write access
inside cancellAI's own resolved storage. E13-S04 and E13-S06 are moved to `done`. E13 remains
`in_progress` only pending its required release evidence: `scripts/release.py check` correctly
refuses a `done` E13 with no such packet, while this review was explicitly prohibited from
running the required `release.py prepare`/`finalize` release steps.
