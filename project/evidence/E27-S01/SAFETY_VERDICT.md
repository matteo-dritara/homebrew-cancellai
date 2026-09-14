# Safety Verdict - E27-S01

- Change: lint-policy hardening, safety arguments, and panic-to-refusal changes
- Risk: CR4
- Commit/PR: `d9e4117` plus verifier repair `1c66095`, PR #19
- Independent verifier: Codex
- Date: 2026-09-14

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

The policy now covers the sole unsafe crate and rejects local lint-table drift. Two formerly
aborting paths now return explicit failure. No new mutation primitive or authority route was
introduced. The verifier corrected two false `FILE_STANDARD_INFO` safety arguments: zero is valid
because both `bool` fields are false, not because the struct lacks validity invariants.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- |
| SI-019 | Mutation remains routed through one boundary and a malformed rename/action does not cause unaccounted mutation. | `rename_child` returns before its rename syscall on guard failure; malformed action appends a failed result and exits failure; full gates pass. | PASS |
| SI-019 | Every unsafe operation has a true written safety argument. | Eleven comments individually source-inspected; two inaccurate comments repaired in `1c66095`; cross-target clippy and independent count pass. | PASS |

## Adversarial cases

- Checked all zeroed-out-parameter error paths and `PROCESSENTRY32W` size initialization.
- Tested lint-table bypass shapes in synthetic temporary workspaces.
- Traced impossible Windows rename-buffer failure to its pre-syscall return.
- Traced malformed CLI action through result accumulation and exit status.
- Differentially examined glob semantics for empty, `*`, `**`, trailing-star, long, and multi-byte cases.

## Differential / compatibility evidence

The glob oracle is independent character-recursive `*` matching. The old matcher was sound for
valid `&str` input; no old panic was reproducible. The changed CLI and Windows paths replace
abort-only behavior with a recorded failure/refusal. Native, Windows-GNU, and Linux-GNU clippy,
workspace tests, Python tests, pre-commit, cargo-deny, and coverage checks pass.

## Known residual risks

- No nightly/Miri installation; no local native Windows execution.
- `expect_used` remains an audited policy exception (15 production invariant expects).
- The TUI's crate-wide indexing exemption remains contingent on its presentation-only role.

## Rollback / recovery

The verifier repair is comment-only and can be reverted independently. Reverting the story returns
the prior lint/panic behavior and is not recommended without a replacement safety policy; neither
commit changes persisted formats or performs a mutation during upgrade.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: pending owner acceptance; this record is the independent verifier's CR4 verdict, not
an authorization to merge PR #19.
