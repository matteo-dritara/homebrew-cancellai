# Safety Verdict - E00-S01

- Risk: CR4
- Independent verifier: Codex
- Date: 2026-08-27, round 2

## Verdict

`FAIL`

| Invariant | Evidence | Result |
| --- | --- | --- |
| SI-001 | `Plugins` is not recognised as protected `plugins`. | FAIL |
| SI-003 | macOS/APFS can resolve that case variant to the protected entry. | FAIL |
| SI-006 | `protected_component()` compares components case-sensitively. | FAIL |

`RoundTwoIndependentVerifierTests.test_protected_name_barrier_is_case_insensitive_for_apfs` fails. The repaired out-of-root symlink case does not cover case or Unicode-normalized aliases on supported APFS.

## Owner decision

`REJECT`
