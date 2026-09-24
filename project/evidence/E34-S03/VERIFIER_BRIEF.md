<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E34-S03
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: bd7b2ae65c48a06e3c3a8221bc71a721d401ed05a5f2f3409b8cc5fed654f18a

<!-- end handoff header -->
# Verifier Brief - E34-S03 - Advisory pre-review is recorded and never counted; the reviewer pool is a decision

Status: ready_for_review | Change Risk: CR1
Outcome: A pre-review on a free model is useful exactly because it is cheap, and harmful if it can pass for the independent verdict. Pre-review records are named <EPIC>-PRE-REVIEW-<n>.md, never count as a round or against the owner's two-review limit, and cannot close a story; process metrics report them separately and break independent rounds down by reviewer family. PD-028 names the reviewer pool and the rule.
Dependencies: E34-S01

## Acceptance Criteria
- The system shall never count a pre-review record as a review round or as a verdict.
- The system shall report independent rounds per reviewer family from each record's Verifier line.
- If a record names no reviewer, then it shall be reported rather than attributed to a default.

## Verification Contract
- process_metrics tests with a pre-review record and records from two reviewers.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md
- project/decisions.json

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
