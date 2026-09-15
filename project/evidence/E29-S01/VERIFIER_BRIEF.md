<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S01
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 00b9e4048efd755637d23ce6534926f5096139f11b39390a9688be4a8306a3f6

<!-- end handoff header -->
# Verifier Brief - E29-S01 - The review-yield measurement can see a round that repaired instead of rejecting

Status: ready_for_review | Change Risk: CR2
Outcome: ADR-0025 makes another review round mandatory while a round rejects 10% or more of the stories it judges, and `process_metrics.py` computes that fraction. E28's round 1 found four material defects across four of five stories and the measurement reported nothing at all - three independent causes. `process_metrics.py:143` skips a story-scoped record with a bare `continue`, so it never reaches the `unclassifiable` list that exists because 'a record that vanishes is worse than one that fails to parse'. `parse_verdicts` reads only a markdown table, so a standalone `## Verdict` heading yields nothing. And `REJECTING_VERDICTS = {"FAIL"}`, so a reviewer who repairs a defect rather than failing the story measures zero yield having found something. The counterfactual yield of that round was 80%; the tool read 0%. Which of those two numbers decides whether another round is required currently depends on how the reviewer chose to write, not on what the reviewer found - and that is the distinction E25 exists to preserve.
Dependencies: none

## Acceptance Criteria
- Every review record this tool declines to count shall be reported by name, because a record that vanishes is worse than one that fails to parse and the tool already says so about a different case.
- A round record shall be identified by what it declares itself to be, rather than inferred from where its file sits.
- If a round found a defect and repaired it rather than rejecting the story, then the measurement shall distinguish that from a round that found nothing.
- If the definition of a rejecting verdict changes, then the yield figures it produces shall be recomputed from the records rather than carried forward.

## Verification Contract
- The E28 round 1 records are used as the fixture: the tool is shown reporting zero rounds before the change and the real round after it.
- A round record with a repaired-in-review finding is shown to produce a non-zero yield.
- A record the tool cannot classify is shown by name in the output, not merely absent.

## Safety Obligations
- none

## Documentation Impact
- docs/adrs/
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
