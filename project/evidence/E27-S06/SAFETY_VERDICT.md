# Safety Verdict - E27-S06

- Change: replace three Windows out-parameter mem::zeroed() calls with binding Default values;
  qualify the remaining generated-binding safety arguments; add a bounded Miri inventory.
- Risk: CR4
- Commit/PR: ce2e872, repaired by af97e09, 51c7af2, and b1f0d08 on PR #19
- Independent verifier: Codex
- Date: 2026-09-14

## Verdict

PASS_WITH_RESIDUALS

## Safety surface changed

The sole unsafe crate no longer locally creates three Windows output structures through
mem::zeroed(). The remaining Windows zero initialization arguments now identify the exact
windows-sys generated definitions on which their validity claim depends.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | Mutation-boundary out-buffers remain valid and do not grant unintended authority | FILE_STANDARD_INFO Default initializes valid fields; its unspecified tail padding is passed only to a documented out-parameter. BY_HANDLE_FILE_INFORMATION is output-only and has no padding. | PASS |
| SI-019 | Remaining unsafe initialization has a true, reviewable validity argument | windows-sys 0.61.2 definitions inspected for PROCESSENTRY32W and IO_STATUS_BLOCK; comments name the version and update hazard; dwSize precedes API use. | PASS |

## Adversarial cases

- Treated tail padding as unspecified, not as zero, for FILE_STANDARD_INFO.
- Checked whether the FFI APIs read an incoming buffer: both signatures document [out].
- Computed BY_HANDLE_FILE_INFORMATION layout rather than assuming it parallels FILE_STANDARD_INFO.
- Reproduced Miri refusals and test-fixture leak reports instead of accepting the packet totals.
- Attempted unsafe-inventory evasion in a block comment, raw string, and trailing comment.

## Differential / compatibility evidence

- Clippy warnings-denied passes on native macOS, Windows-GNU cross target, and Linux-GNU cross
  target.
- The locked windows-sys 0.61.2 definitions, not a 0.59 registry copy, were inspected.

## Known residual risks

- No local native Windows execution and the new scheduled Ubuntu Miri job has not yet run.
- Miri reaches a sealedfs statfs unsafe call but cannot model it; it provides no complete dynamic
  audit of the 38 remaining unsafe blocks.
- E27-S07 tracks the unrelated reproducible platform coverage-ratchet failure; its baseline was
  not lowered in this review.

## Rollback / recovery

Revert the three verifier repair commits to restore the executor's original comments, test filter,
and Miri workflow; no production data migration or mutation occurred.

## Owner decision

ACCEPT_WITH_RECORDED_RESIDUALS

Owner note: pending owner acceptance; do not merge PR #19 as part of this verdict.
