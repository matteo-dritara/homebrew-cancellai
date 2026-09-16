# E12 Independent Verifier Review — Round 2

Review-Scope: epic
Round: 2
Review target: uncommitted working-tree repairs on `82d30c1..f8dc4cc` (main)
Verifier: Codex
Date: 2026-09-16

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E12-S01 | FAIL | Forced record-sidecar failure now rolls back. A separate concurrent recreation of the source name reproduced the advertised, honest double-fault message. But `confirmed_move_inner` renames before any durable journal/record exists; crash/power loss in that interval leaves a moved, unrecorded object with no restart recovery, violating AC3/SI-020. Brief-Checksum: `eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734`. |
| E12-S02 | PASS_WITH_RESIDUALS | `renameat2(RENAME_NOREPLACE)` / `renameatx_np(RENAME_EXCL)` replaces the old destination `fstatat` plus clobbering rename. The injected recreate-in-window regression protects the provider bytes. Unsupported primitive/filesystem returns an explicit refusal, and Windows still refuses. Brief-Checksum: `012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0`. |
| E12-S03 | FAIL | Exact equal-length rewrite is now `FingerprintMismatch`; forced third-sidecar failure rolls back and a later retry overwrites stale sidecars correctly. FNV-1a is expressly non-collision-resistant, however, and the same pre-record crash window remains. A CR4 pre-purge integrity predicate cannot accept that adversarial-collision and crash-recovery residual. AC3/SI-020 remain violated. Brief-Checksum: `a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b`. |

## Failures and required repairs

### E12-S01

Reproduction: an isolated program pre-planted the sidecar temporary name, then concurrently recreated the source once the move was visible. It emitted `rollback also failed ... object remains at the destination without a restore record - manual recovery required`; both names existed, with the source holding the newly created object. This validates the message, not recoverability across crash.

Required repair: persist and fsync an identity-bound pre-move journal, make every state transition restart-recoverable, and add crash injection after rename and every metadata transition. This is required by AC3 and SI-020.

### E12-S03

Reproduction: third-sidecar (`archive-fingerprint`) temporary-name collision produced source present, destination absent, `archive-record` and `archive-length` sidecars present, no fingerprint sidecar. A retry after removing the obstacle succeeded and rewrote the stale records. Separately, code inspection proves the rename precedes every persistent archive record; process death there has no recovery path. The documented FNV-1a residual is a deliberate-collision gap in the predicate intended to authorize future purge.

Required repair: a reviewed ADR-0019 kernel-ring digest decision followed by a versioned collision-resistant digest, plus E12-S01's durable journal/restart protocol for all archive sidecars. This is required by AC3 and SI-020.

## Gate status

| Command(s) | Result |
| --- | --- |
| `python3 -m pytest tests -v` | PASS — 665 passed, 543 subtests passed. |
| `python3 -m ruff check .`; `python3 -m ruff format --check .` | NOT RUNNABLE — Ruff is not installed (`No module named ruff`). |
| Full AGENTS.md mypy target list | NOT RUNNABLE — mypy is not installed (`No module named mypy`). |
| `gen_docs --check`; `project_os check`; every listed `scripts/*.py check`; parity self-test/check | PASS, with only recorded baseline warnings. |
| `cargo fmt --check`; native/cross-target (`x86_64-pc-windows-gnu`, `x86_64-unknown-linux-gnu`) clippy `-D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS; deny emitted its recorded non-failing licence/duplicate warnings. |
| GitHub Actions baseline query | UNKNOWN — `gh run list --branch main --limit 5` could not reach `api.github.com`; unknown was not treated as green. |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`; `project/epics/E12.json`; `docs/architecture/PERSISTENCE_MODEL.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/adrs/0019-dependency-rings-per-crate.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; all three E12 verifier briefs; round-one review and Safety Verdicts; all three executor evidence packets; and `project/templates/SAFETY_VERDICT.md`.

## Overall round verdict

`FAIL` — 2 of 3 judged stories were rejected (67% finding/rejection yield). This overlaps round one’s incomplete after-rename recovery and archive-integrity findings; it is not a zero-overlap escalation. A third review round is mandatory under ADR-0025 because the yield exceeds 10%. No verifier repair was made in this round, so no story is recorded as `REPAIRED` for yield accounting. E12-S02 is `PASS_WITH_RESIDUALS` but is recorded `blocked` rather than `done` because its E12-S01 dependency is again `in_progress`; E12-S03 is likewise recorded `blocked` while retaining its `FAIL` verdict and required repair. E12-S04 remains `planned`, already not ready because of its separate E13-S02 dependency as well as the reopened E12-S01.
