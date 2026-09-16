# Safety Verdict - E12-S02

- Change: Restore protocol, round-6 independent verification of recovery-scope reduction
- Risk: CR4
- Review target: uncommitted working-tree changes on `82d30c1..f8dc4cc` (main)
- Independent verifier: Codex
- Verifier: Codex
- Date: 2026-09-16
- Brief-Checksum: 012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0

Brief-Checksum: 012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0
Verifier: Codex

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Restore retains the identity-confirmed, handle-relative atomic no-replace move. It writes no sidecar and neither invokes nor depends on the removed scanner.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Restore never silently replaces unrelated provider state. | Linux `renameat2(RENAME_NOREPLACE)` and macOS `renameatx_np(RENAME_EXCL)`; recreate regression and full workspace suite pass. | PASS |

## Adversarial cases

- Existing and concurrently recreated destinations are refused by the kernel no-replace primitive.
- Unsupported primitives/filesystems and Windows refuse; no path-based fallback exists.
- Removal search and call-site inspection confirm no Restore dependency on automatic pending-sidecar recovery.

## Differential / compatibility evidence

`pytest` (665 passed, 547 subtests), Ruff, mypy, every AGENTS.md checker, Rust fmt, native and Windows/Linux-target Clippy, workspace check/test, and `cargo deny check` pass.

## Known residual risks

- Windows and Unix systems without a verified atomic no-replace rename refuse restore.
- A failed quarantine-record finalization needs manual completion before a future caller consumes that record; Restore creates none itself.

## Rollback / recovery

The completed restore move is atomic and creates no provider-directory metadata. The removed scanner does not affect this no-clobber commit property.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS` — refusal on unsupported platforms/filesystems and the quarantine manual-completion residual cannot authorize unsafe restore.
