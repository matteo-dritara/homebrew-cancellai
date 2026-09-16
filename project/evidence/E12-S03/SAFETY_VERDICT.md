# Safety Verdict - E12-S03

- Change: Archive/compression lifecycle, round-6 independent verification of recovery-scope reduction
- Risk: CR4
- Review target: uncommitted working-tree changes on `82d30c1..f8dc4cc` (main)
- Independent verifier: Codex
- Verifier: Codex
- Date: 2026-09-16
- Brief-Checksum: a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b

Brief-Checksum: a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b
Verifier: Codex

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Archive retains the same-volume identity-confirmed move plus explicit format, length, and SHA-256 sidecars. The witness/scanner used only for automatic post-finalize completion is removed.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-018 | Archive does not silently cross a filesystem boundary or copy. | Same-device safety-executor check and handle-relative no-clobber tests pass. | PASS |
| SI-019 | Archive stays behind the sole mutation boundary. | `scripts/check_mutation_boundary.py check` passes. | PASS |
| SI-020 | Incomplete archive metadata cannot silently authorize purge. | SHA-256 integrity remains; failed finalize returns `Err` with correct pending metadata; purge is E12-S04 planned. | PASS_WITH_RESIDUALS |

## Adversarial cases

- Workspace tests prove successful archive writes format, length, and SHA-256 sidecars and detects equal-length corruption.
- Retained write-before-move tests prove record and later digest-sidecar durable-write failures leave source unmoved and clean earlier pending entries.
- Inspection and removal search confirm no scanner completes partial sidecars or authorizes source purge.

## Differential / compatibility evidence

`pytest` (665 passed, 547 subtests), Ruff, mypy, every AGENTS.md checker, Rust fmt, native and Windows/Linux-target Clippy, workspace check/test, and `cargo deny check` pass. Windows archive refuses explicitly.

## Known residual risks

- Post-move finalize failure can leave full correct archive sidecars under `.pending`; no code completes them automatically. No future archive/purge caller may treat this state as verified, and no purge capability exists.
- Real byte compression remains out of scope.

## Rollback / recovery

Pre-move failure and no-clobber move cleanup remain. Post-move finalization is manual-only until a separately scoped, independently verified state machine exists.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS` — deferred completion is acceptable for this unwired primitive because it leaves durable correct evidence and cannot authorize purge.
