# Evidence Packet - E28-S02

- Commit/PR: on `main`, this epic's commits
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E28.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `verifier_handoff.py brief <ID>` writes `project/evidence/<ID>/VERIFIER_BRIEF.md` with a SHA-256 over its own body, excluding the header that carries it. `tests/test_verifier_handoff.py::Checksums` | PASS |
| AC2 | A verdict repeats the checksum on `Brief-Checksum:`; a mismatch, a missing brief, or a verdict that omits the checksum despite an existing brief refuses. `tests/test_verifier_handoff.py::HandoffCases::test_a_verdict_answering_a_different_document_is_refused`, `::test_a_verdict_naming_a_brief_that_does_not_exist_is_refused`, and `::test_a_verdict_cannot_bypass_a_rendered_brief_by_omitting_its_checksum` | PASS after verifier repair |
| AC3 | A verdict whose `Verifier:` equals the brief's `Rendered-by:` is refused outright. `tests/test_verifier_handoff.py::HandoffCases::test_the_executor_may_not_author_the_verdict` | PASS |
| AC4 | A brief with no verdict passes and no verdict is written; there is no fallback path. `tests/test_verifier_handoff.py::TheSeparationSurvivesTheAutomation::test_an_unavailable_verifier_leaves_no_verdict_behind` | PASS |
| AC5 | `Verifier:` is required, not defaulted; an unattributed verdict refuses. `tests/test_verifier_handoff.py::HandoffCases::test_an_unattributed_verdict_is_refused` | PASS |
| AC6 | A story with no brief is reported as having used the manual route, not failed. `tests/test_verifier_handoff.py::TheSeparationSurvivesTheAutomation::test_the_manual_route_stays_valid` | PASS |

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

- The executor-authorship refusal compares two strings. A party willing to write a different name on the `Verifier:` line defeats it. The mechanism removes an accident, not an intent, and the evidence ledger's attribution remains a claim rather than a proof.
- The checksum proves which document a verdict answers, not that the verifier read it.
- Only four stories carry a brief; every historical verdict predates the mechanism and is reported as having used the manual route, which is correct but means the gate currently protects almost nothing retroactively.

## Verifier verdict

pending
