# Evidence Packet - E22-S07

- Commit/PR: the fix-release shape on `main`
- Executor: Claude
- Independent verifier: none - the owner waived the Codex round for this session's work
- Change Risk: CR3 (the floor `project/risk_floors.json` sets for `scripts/release.py`: release authority)
- Spec version/commit: `project/epics/E22.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - exactly one shape | `prepare(version, epic_id=None, reason=None)` refuses when both or neither is given, before it writes anything; argparse enforces the same as a mutually exclusive required group, so the refusal holds whether the function is called or the command is run. `test_prepare_refuses_both_shapes_at_once`, `test_prepare_refuses_neither_shape`. | PASS |
| AC2 - a fix takes the next patch | `is_patch_of` accepts only same-major, same-minor, patch+1. `test_a_fix_release_takes_the_next_patch_number` rejects a minor bump, a major bump, a skipped patch and the current version itself; `test_a_fix_release_may_not_claim_a_feature_number` drives it through `prepare`. | PASS |
| AC3 - prose does not credit an epic | `released_epics()` now reads `EPIC_DECLARATION_RE` (`- Epic: EXX`) instead of every `E\d\d` in the file. Measured before changing it: loose matching credited 18 epics, the declared line credits 14, and the 14 are exactly the `done` set - so four epics (E06, E12, E16, E17) were credited for being mentioned. None was `done`, so nothing had gone wrong yet. `test_an_epic_named_only_in_prose_is_not_credited`, `test_every_done_epic_has_release_evidence`. | PASS |
| AC4 - a fix release states its reason | `prepare` refuses an empty or whitespace reason. `test_a_fix_release_needs_a_stated_reason`. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| PD-021 (a closed epic has a release) | A fix release used as a back door to close an epic without the verification a closure requires | A fix-release packet contains no declared epic line at all, so it cannot credit one: `test_a_fix_release_packet_declares_no_epic` asserts it against the very regular expression the gate uses, rather than against a copy of it. The two shapes are mutually exclusive at the boundary. | PASS |
| PD-021 | An epic quietly credited by prose after this change | `test_every_committed_packet_declares_at_least_one_epic_or_says_it_closes_none` holds over all 13 committed packets, and `release.check()` returns no problems with the strict reading. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_release.py -q   -> 17 passed, 17 subtests
python3 scripts/release.py check             -> consistent; no epic closed without a release
python3 -m mypy scripts/release.py           -> clean
```

## Compatibility

- `prepare --epic` is unchanged, including its output. `--fix` is new and additive.
- No product behaviour, no schema, no user-facing surface.

## Performance / operability

- Not applicable.

## Residual risks

- **`--fix` is a judgement call with no gate behind it.** Nothing stops a feature being shipped as
  a patch by writing a persuasive reason. The constraint is the patch number and the recorded
  sentence, not a check on the diff.
- **The declared-epic line is now load-bearing and is written by a template.** A packet edited by
  hand can still drop it; the new test catches a packet that neither declares nor disclaims, which
  is the reachable failure, but it cannot catch a packet that declares the wrong epic.
- **This does not fix the release workflow itself.** v1.12.0 failed on a Windows packaging step and
  v1.13.0 on a clippy denial; the second is repaired (E06-S05), the first has not recurred and has
  not been diagnosed. A third failure would be the evidence that it is not transient.
- **Two tags remain published with no release behind them.** They stay as history; nothing removes
  or rewrites them.

## Verifier verdict

closed on the owner's waiver; no independent verdict
