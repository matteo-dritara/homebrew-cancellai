# Evidence Packet - E25-S03

- Commit/PR: the E25-S03 commit on `feat/e24-agent-execution-layer-v2`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR1
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS, with AC1's "quoted rather than paraphrased" clause **not implemented** and recorded as such
rather than claimed - see Residual risks.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - one row per acceptance criterion | `CriterionCoverageTests` - a row per criterion passes, fewer fails, duplicated row numbers do not inflate the count, more rows than criteria is not an error. The quoting half is not implemented; see residuals. | PARTIAL |
| AC2 - a CR3/CR4 packet with an empty residual section fails | `ResidualRiskTests` - `- none`, an empty section, and four spellings of "none" behind list markers all fail at CR3/CR4 and pass at CR1. | PASS |
| AC3 - a CR4 story at done without a Safety Verdict fails | `SafetyVerdictTests`, including that a `ready_for_review` CR4 story does **not** need one yet, because the verdict is the reviewer's output and is gated at `done`. | PASS |
| AC4 - a claimed command names something that exists | `ClaimedCommandTests` - a packet citing `scripts/imaginary.py` fails. | PASS |
| AC5 - pre-convention packets are grandfathered explicitly with a reason | `BASELINE`, fifteen entries, each with a reason, each printed on every run. `BaselineTests` holds every entry to naming a real story and a real reason, so the list can only shrink. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A CR4 story closing with no examined residual risk | `test_a_cr4_packet_with_no_residual_risk_fails`, and `RealRepositoryTests::test_no_cr3_or_cr4_packet_in_the_repository_declares_no_residual_risk` holds the whole committed ledger to it. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_evidence.py -q -> 19 passed, 40 subtests
python3 scripts/check_evidence.py check     -> 90 packets checked, 15 pre-convention recorded
python3 scripts/project_os.py check         -> governance OK
```

## Compatibility

- Stdlib only, no network, no git. Reads the control plane and the evidence tree.

## Performance / operability

- Reads 90 packets; instantaneous.

## Documentation updated

- `AGENTS.md` check list, `.pre-commit-config.yaml`, both workflows, `pyproject.toml`.

## Residual risks

- **AC1's quoting clause is not implemented.** The checker counts rows; it does not compare a row's
  text against the criterion it claims to discharge. Paraphrasing an acceptance criterion is how one
  quietly gets easier, and that is still possible. Doing it properly needs the row to carry the
  criterion index unambiguously and a similarity rule nobody has justified yet; asserting it in the
  packet while not doing it would be the exact failure this checker exists to catch.
- **A row claiming PASS proves nothing.** The E24 review found two rows certified by tests that
  could not have detected the defects present. This checker cannot see that; only review can, and
  E25-S06's gate sensitivity harness is the mechanical part of the answer.
- **The baseline is fifteen entries and will not shrink on its own.** Nothing schedules their
  repair, and a list that only warns can be read past indefinitely.
- **"None" detection is textual.** A residual section that says "nothing material was identified"
  passes while meaning the same thing.

## Verifier verdict

pending
