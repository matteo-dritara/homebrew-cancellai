# E12 Independent Verifier Review — Round 3

Review-Scope: epic
Round: 3
Review target: uncommitted working-tree repairs on `82d30c1..f8dc4cc` (main), including rounds 1 and 2 repairs
Verifier: Codex
Date: 2026-09-16

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E12-S01 | FAIL | The intended restart paths work independently: a moved primary plus pending record finalizes byte-for-byte, and a pending record without a primary discards. But a different reproduction placed an existing primary and final record alongside a pending record from an aborted conflicting operation. `recover_pending_moves` returned `Finalized`, removed the pending file, and silently replaced `prior record` with `second operation record`, because it proves only filename presence. Brief-Checksum: `eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734`. |
| E12-S02 | PASS_WITH_RESIDUALS | The restore commit is still a single atomic no-replace rename on supported Unix targets, with explicit refusal where unavailable; no check-then-clobber path returned. Its prerequisite E12-S01 remains rejected, so this story is blocked despite its direct primitive passing. Brief-Checksum: `012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0`. |
| E12-S03 | FAIL | ADR-0030 and SHA-256 close the round-2 digest finding: independent equal-length `ABCDE` -> `12345` corruption returned `DigestMismatch`, and `cargo tree -i sha2@0.11.0` showed one shared dependency. Archive recovery nevertheless has the same filename-only completion predicate, so its record/length/digest sidecars can overwrite metadata for an existing archive after an aborted conflicting move. Brief-Checksum: `a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b`. |

## Failures and required repairs

### E12-S01

Reproduction: in an isolated executable, I created a store containing `artifact` and its final
`artifact.quarantine-record.json` with bytes `prior record`. I then used the production sealed
root writer to durably create `artifact.quarantine-record.json.pending` with bytes `second
operation record`, representing a second operation that dies after writing its pre-move sidecar
but before the no-replace move rejects the pre-existing destination. `recover_pending_moves`
reported `Finalized`; the pending file disappeared and the final record read `second operation
record`.

Required repair: journal each operation with a durable identity/operation binding, then verify
that binding against the destination artifact before finalizing. A pre-existing destination is
not evidence that a pending sidecar belongs to it. Collision, crash-before-failed-commit, and
partial multi-sidecar recovery cases need regressions. This is a new defect in the round-3
recovery repair; it violates AC3 and SI-020.

### E12-S03

Reproduction: the same state-machine error applies to archive's three pending sidecars. An
existing archive artifact makes filename-only recovery finalize new `archive-record`,
`archive-length`, and `archive-digest` values from an operation whose no-replace move never
completed. A later integrity check can consequently certify metadata that was not captured from
the existing archive.

Required repair: apply E12-S01's identity/operation-bound recovery design to all archive
sidecars, with conflict and partial-recovery tests. This is a new defect in the round-3 recovery
repair; it violates AC3 and SI-020. The round-2 SHA-256 repair itself is closed.

## Gate status

| Command(s) | Result |
| --- | --- |
| `python3 -m pytest tests -v` | PASS — 665 passed, 544 subtests passed. |
| `python3 -m ruff check .`; `python3 -m ruff format --check .` | NOT RUNNABLE — Ruff is not installed (`No module named ruff`). |
| Full AGENTS.md mypy target list | NOT RUNNABLE — mypy is not installed (`No module named mypy`). |
| `gen_docs --check`; `project_os check`; every listed `scripts/*.py check`; `rust_python_parity.py self-test` and `check` | PASS (with recorded baseline warnings only). |
| `cargo fmt --check`; native and cross-target (`x86_64-pc-windows-gnu`, `x86_64-unknown-linux-gnu`) Clippy `-D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS; cargo deny emitted its recorded non-failing licence/duplicate warnings. |
| `cargo tree -i sha2@0.11.0` | PASS — one shared `sha2 v0.11.0` is used by `cancellai-platform` and `cancellai-safety`. |
| Independent isolated recovery/digest executable | Reproduced valid finalize and discard paths, `DigestMismatch` on equal-length corruption, and the new false-finalize metadata overwrite. |
| GitHub Actions baseline query | PASS — all five latest main workflows reported `success`. |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `docs/architecture/PERSISTENCE_MODEL.md`;
`docs/architecture/PLATFORM_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`;
`docs/development/AGENT_PROTOCOL.md`; `docs/adrs/0019-dependency-rings-per-crate.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
`docs/adrs/0030-sha2-for-archive-integrity-in-cancellai-platform.md`;
`project/templates/SAFETY_VERDICT.md`; all three E12 verifier briefs; both prior E12 verifier
review records; all three executor evidence packets; and the final Rust/source diff.

## Overall round verdict

`FAIL` — two of three judged stories have genuine unresolved CR4 defects (67% finding/rejection
yield). The finding overlaps round 2's crash/recovery area directly: it is a flaw in the new
restart-recovery repair supplied specifically to close that finding, rather than a zero-overlap
population signal. The SHA-256/ADR-0030 repair closes the separate round-2 digest finding.

This is the owner-authorized third and cost-ceiling round. Reaching the ceiling is not a pass:
E12-S01 and E12-S03 remain open, E12-S02 is blocked on E12-S01, and the owner must explicitly
decide whether to authorize the required further repair/follow-up review or to accept a recorded
residual. No verifier product-code repair was made in this round.
