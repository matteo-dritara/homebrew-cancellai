# Safety Verdict - E10-S03

- Change: authorize and independently classify the released macOS `statfs` filesystem observer
- Risk: CR4
- Commit/PR: independent-review repair, 2026-09-13
- Independent verifier: Codex
- Date: 2026-09-13

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

ADR-0027 broadens the documented role of `cancellai-sealedfs`, the workspace's only unsafe-code
exception, to include a narrowly scoped read-only macOS filesystem-type observation. It does not
create a mutation route or a plan/authority input.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-016 | No mutation may derive from an unsealed or UI-created plan. | The probe returns only a clone-sharing confidence fact; no mutation, plan, root, or executor symbol is referenced. | PASS |
| C-02 / C-03 | Unknown observation never becomes stronger reclaim truth. | Errors become `Unsupported`; unknown/invalid names become `PossiblyShared`; only fully-known, non-sharing facts are `Verified`. | PASS |
| C-07 | Unsafe filesystem capability remains isolated. | ADR-0027 retains the sole unsafe exception in `cancellai-sealedfs`; `cancellai-platform` remains `unsafe_code = forbid`. | PASS |

## Adversarial cases

- Interior-NUL path: `CString::new` rejects it before FFI.
- Missing path: `statfs` returns an OS error, which becomes conservative `Unsupported`.
- Signed `c_char` and invalid UTF-8 type name: bytes are retained with `as u8` then lossy-decoded,
  never panicking or matching an ASCII allow-list accidentally.
- Symlink: Darwin resolves the observed backing filesystem only; no mutation or authority result
  is produced.

## Differential / compatibility evidence

Native macOS filesystem-kind tests pass. The Linux observer remains `/proc/mounts` based and
Windows remains explicitly unsupported. Miri could not run because the active stable toolchain
does not provide its component; no toolchain was installed.

## Known residual risks

The OS ABI behaviour is covered by the `libc` binding and native tests, not Miri. Filesystem
classification is deliberately conservative and has no per-file extent-sharing proof.

## Rollback / recovery

Reverting ADR-0027's implementation authority and returning `Unsupported` on macOS downgrades
reclaim confidence without affecting provider state or executing mutation.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: The owner authorized this independent verifier to repair and ship the finding on
2026-09-13. The Miri component remains unavailable locally and is recorded above.
