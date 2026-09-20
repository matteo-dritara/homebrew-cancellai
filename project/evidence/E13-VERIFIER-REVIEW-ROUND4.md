# E13 Independent Verifier Review — Round 4

Review-Scope: epic
Round: 4
Verifier: Codex
Date: 2026-09-20
Review-Target: `f96a38b..f215d33`

This round reviews only E13-S04 and E13-S06: the repaired local-reset scope and E13-S06's new
ownership-boundary carrier. It does not re-judge the already-closed E13-S02, E13-S03, or E13-S05.

## Brief provenance

| Story | Brief-Checksum |
| --- | --- |
| E13-S04 | Brief-Checksum: 29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73 |
| E13-S06 | Brief-Checksum: f0e748293ab2bccb98162c27fa451b0f9850d2efebd4e04d4cdfb7b29bca2b31 |

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E13-S04 | FAIL | AC2 and SI-026 remain violated through the claimed E13-S06 repair. `LocalStateRoot::resolve(&provider_dir)` publicly mints a root for an arbitrary provider directory; production `EventLedger::open(&root)` then accepts a copied marker-bearing `event_ledger.sqlite3`, and `reset()` erases its one event. A fixed-filename symlink planted after resolving a real root similarly redirects production `open()` to that provider ledger and `reset()` clears it. The budget admission tests and in-memory ephemeral-write test pass, but cannot repair a reset target boundary that remains bypassable. Brief-Checksum: 29d1f9f58400ce38b5ee0394cd6ab5c300c0ef55fa93474a069cc5a440356e73 |
| E13-S06 | FAIL | `LocalStateRoot` is not a cancellAI-owned, non-caller-suppliable capability: its only public constructor is `resolve(dir: &Path)`, and `dir` is arbitrary caller input. The temporary public-API reproducer passed: it copied a real marker-bearing ledger to `provider-root/event_ledger.sqlite3`, called `LocalStateRoot::resolve(&provider_root)`, then production `EventLedger::open(&root)` and `reset()` reduced the row count from one to zero. The Unix symlink reproducer also passed: a symlink at `<resolved-root>/event_ledger.sqlite3` redirected `open()` and `reset()` outside the root. Brief-Checksum: f0e748293ab2bccb98162c27fa451b0f9850d2efebd4e04d4cdfb7b29bca2b31 |

## Ownership-boundary reproductions

The three prior marker-mimicry shapes are **not refused** by the current design once its public
root constructor is used as the bypass: all three production openers take the same public
`&LocalStateRoot` and derive only their fixed filenames below it. The ledger reproduction was run
directly and erased the copied marker-bearing provider ledger. The same public arbitrary-directory
constructor and identical `open(root.path_for(FIXED_FILENAME))` shape apply to CurrentStateStore
and AnalyticalMemory, so their sibling-directory tests establish only that a caller who elects the
correct root cannot name a sibling file; they do not establish that the elected root is cancellAI
owned. A marker remains a corruption/migration check, not authority, but the current public
constructor lets the caller re-authorize the marker-bearing file's parent directory.

The additional fixed-filename symlink reproduction passed on macOS: after `LocalStateRoot::resolve`
returned for a genuine directory, a symlink from its fixed ledger filename to a copied
marker-bearing provider ledger was followed by `EventLedger::open`; `reset()` erased the provider
event. This violates SI-026 independently of marker mimicry and leaves a resolution/open TOCTOU.

## Required repair

Make the production local-state root non-caller-suppliable in substance, not only in its
parameter type: production resolution must select cancellAI's configured platform state location
through one reviewed path without exposing `resolve(&arbitrary_path)` publicly. Keep an explicitly
test-only/crate-private path constructor for direct unit tests. Bind later opens to the resolved
directory and refuse symlink/reparse traversal of both the root and the fixed database leaves, or
otherwise use an equivalent handle-relative mechanism that closes the resolution/open race. Add
public-API regressions for arbitrary-directory root minting and leaf/root symlink redirection for
all three layers.

This repair is required for E13-S04 AC2, E13-S06 AC1/AC2, Constitution C-10, and SI-026. The
existing residual statement is not sufficient: it explicitly describes a successful route by
which reset mutates provider payload.

## Acceptance-criterion assessment

- E13-S04 AC1: the ledger and rollup admission/stress tests passed; Layer-1 pressure is exercised
  by `rebuild_within_current_state_budget_wires_the_write_path_to_enforcement`. No contrary budget
  counterexample was reproduced in this round.
- E13-S04 AC2: FAIL as reproduced above.
- E13-S04 AC3: the existing `ephemeral_layers_perform_zero_persistent_writes_across_many_operations`
  passed in the store suite. No persistent-write path from the in-memory constructors was found.
- E13-S06 AC1/AC2: FAIL as reproduced above. AC3 holds structurally (`open_at_path` is crate-private
  and `open_in_memory` remains public); AC4 holds because all three marker checks remain present,
  but neither closes the failed authorization boundary.

## Gate status

| Command | Result |
| --- | --- |
| `cargo test -p cancellai-store public_resolve_can_retarget_production_open_to_an_arbitrary_directory -- --nocapture` (temporary, removed) | PASS as a failure reproduction: public `resolve(provider_dir)` followed by production open/reset erased the copied marker-bearing ledger event. |
| `cargo test -p cancellai-store fixed_filename_symlink_redirects_production_open_outside_the_resolved_root -- --nocapture` (temporary, removed) | PASS as a failure reproduction on macOS: a fixed-name symlink redirected production open/reset outside the root. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test -p cancellai-store` | PASS — 125 tests |
| `cd rust && cargo test --workspace` | PASS |
| `cd rust && cargo deny check` | PASS with existing unmatched-license and duplicate-dependency warnings; initial sandbox attempt could not lock Cargo's advisory DB, rerun with the approved cache-lock access passed. |
| `python3 -m pytest tests -v` | PASS — 668 passed, 579 subtests |
| `python3 -m ruff check .`; `python3 -m ruff format --check .`; `python3 -m mypy …` | NOT RUN: this interpreter has no `ruff` or `mypy` module. |
| `python3 scripts/gen_docs.py --check`, `check_docs.py check`, `check_workflows.py check`, `check_fixtures.py check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check` | PASS |
| `python3 scripts/check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py check`, `check_provider_trust.py check`, `check_platforms.py check` | PASS |
| `python3 scripts/rust_python_parity.py self-test`; `python3 scripts/rust_python_parity.py check` | PASS |
| `python3 scripts/check_process.py check`, `release_manifest.py check`, `check_repository_topology.py check`, `check_agent_skills.py check`, `process_metrics.py check`, `check_risk_classification.py check`, `check_agent_toolchain.py check`, `check_skill_content.py check`, `verifier_handoff.py check`, `check_evidence.py check`, `safety_oracle.py check`, `check_ears.py check`, `gate_sensitivity.py check` | PASS with their recorded baseline warnings. |
| `gh run list --branch main --limit 5` | UNKNOWN: `gh` could not connect to `api.github.com`; CI is not treated as green. |

`scripts/release.py` was intentionally not run, as directed. No Python reference behavior changed.

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/architecture/PERSISTENCE_MODEL.md`
- `docs/architecture/TARGET.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/CLI.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `project/epics/E13.json`
- `project/evidence/E13-VERIFIER-REVIEW.md`
- `project/evidence/E13-VERIFIER-REVIEW-ROUND2.md`
- `project/evidence/E13-VERIFIER-REVIEW-ROUND3.md`
- `project/evidence/E13-SELF-REVIEW.md`
- `project/evidence/E13-SELF-REVIEW-ROUND2.md`
- `project/evidence/E13-S04/VERIFIER_BRIEF.md`
- `project/evidence/E13-S06/VERIFIER_BRIEF.md`
- `rust/crates/cancellai-store/src/{local_state_root.rs,lib.rs,ledger.rs,rollup.rs,budget.rs}`

## Overall verdict

FAIL. E13-S06 returns to `in_progress`. E13-S04's FAIL is recorded above, but it remains
`ready_for_review` solely because E13-S06 has a same-epic dependency on it and the control-plane
gate refuses an in-progress dependent while its dependency is in progress; its AC2 remains
unclosed pending the E13-S06 repair. This is the first and only allowed failure round for the new
S04+S06 scope; no implementation repair was made by the verifier.
