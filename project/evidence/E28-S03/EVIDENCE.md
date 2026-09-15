# Evidence Packet - E28-S03

- Commit/PR: on `main`, this epic's commits
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR1
- Spec version/commit: project/epics/E28.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | The `## Method defects` section lives in `project/templates/EVIDENCE_PACKET.md`, inside the packet the story already commits. `tests/test_method_defects.py::TheMechanismProposesAndNeverWrites::test_the_template_carries_the_section` | PASS |
| AC2 | `Prevented by:` is required and names a document, gate or skill, or `none exists`. `tests/test_method_defects.py::Disposition::test_an_entry_that_points_nowhere_is_refused` | PASS |
| AC3 | An entry with no `Disposition:` refuses the packet. `tests/test_method_defects.py::Disposition::test_an_entry_without_a_disposition_is_refused` | PASS |
| AC4 | No path writes to `.claude/skills/` or any canonical document - asserted over the gate's own source. `tests/test_method_defects.py::TheMechanismProposesAndNeverWrites::test_no_path_in_the_gate_writes_anywhere` | PASS |
| AC5 | A declined entry stays in the packet and still parses. `tests/test_method_defects.py::Disposition::test_a_declined_entry_stays_in_the_packet` | PASS |
| AC6 | The section lives in the existing packet; no second store was created. Only `check_evidence.py` reads it. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | This story declares no safety obligation: it governs the toolchain and the process, not the mutation boundary. | The mutation-boundary gate (SI-019) is unchanged and still passes. | PASS |

## Verification Commands

```text
python3 -m pytest tests -q
python3 scripts/check_skill_content.py check
python3 scripts/verifier_handoff.py check
python3 scripts/check_agent_toolchain.py check
python3 scripts/check_evidence.py check
pre-commit run --all-files
```

## Compatibility

- Platforms/providers/schemas exercised: macOS, Python 3.13. No product surface is touched; no schema changed.

## Performance / operability

- The skill-content scan adds roughly 30 seconds to `pre-commit run --all-files`. It is the only gate in the set that shells out to a third-party analyser.

## Documentation updated

- `docs/development/AGENT_TOOLCHAIN.md`, `docs/development/AGENT_PROTOCOL.md`, `AGENTS.md`

## Method defects

- none

## Residual risks

- A disposition of `proposed` is a valid resting state with no expiry, so an entry can sit unresolved indefinitely without any gate noticing. The review cadence that covers toolchain decisions does not reach here.
- The gate checks that an entry is well-formed, never that it is true or useful. A packet can satisfy it with a trivial defect and a rubber-stamped disposition.

## Verifier verdict

pending
