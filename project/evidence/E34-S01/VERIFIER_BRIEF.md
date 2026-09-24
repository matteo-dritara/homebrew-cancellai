<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E34-S01
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: bdd4184cfe0cb67b89f90f7dd271af87928b2c7075da6d0dc15d6de36a559daa

<!-- end handoff header -->
# Verifier Brief - E34-S01 - A review round is run by a harness that proves who reviewed and what they changed

Status: ready_for_review | Change Risk: CR2
Outcome: Rounds were run by hand: a prompt paraphrased by the executor, a worktree created ad hoc, and records imported by copying files - which is how one reviewer overwrote a 2026-09-01 record. scripts/review_round.py runs a round from the committed briefs for Codex or OpenCode, names the record so it can never overwrite one, and after the reviewer exits checks that every model the session streamed from is the declared, non-Anthropic model and that every changed path is one the tier allows.
Dependencies: none

## Acceptance Criteria
- The system shall build the reviewer's prompt from the committed verifier briefs of the named stories and refuse to start when a story has no committed brief.
- The system shall choose the record's filename itself and refuse to start when that file already exists.
- If the reviewer's session streamed from any model other than the declared one, or from an Anthropic model, then the round shall be reported as not independent and its record shall not be importable.
- If the reviewer changed any path outside the tier's allowed set, then the round shall be refused and nothing shall be imported.
- When a round passes its checks, the system shall import exactly the changed allowed paths into the working tree and nothing else.

## Verification Contract
- Unit tests cover prompt rendering from briefs, record naming, stream-model parsing, the Anthropic refusal, the path allow-list and import.
- A real OpenCode pre-review round runs through the harness.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
