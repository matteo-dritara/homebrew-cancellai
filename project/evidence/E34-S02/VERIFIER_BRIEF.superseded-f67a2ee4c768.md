<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E34-S02
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: f67a2ee4c768ed1a25d97575c01930ad7fc2778279284d1d5ceded8363618012

<!-- end handoff header -->
# Verifier Brief - E34-S02 - OpenCode reviewer agents with tool-enforced permissions and a pinned model

Status: ready_for_review | Change Risk: CR1
Outcome: OpenCode loads this repository's skill pack natively from .claude/skills and can enforce per-agent permissions. Two agents - verifier and pre-reviewer - are defined in .opencode/agents with edits allowed only under project/evidence and the story statuses, dangerous shell commands denied, no web access, and one pinned model with no fallback; opencode.json pins the small model too, so no second model enters a session, and disables sharing and autoupdate.
Dependencies: none

## Acceptance Criteria
- The system shall define verifier and pre-reviewer agents whose permissions deny git commit, push, tag and reset, package installation, file removal and web access.
- The system shall pin one model for every agent and for the small model, so a session streams from exactly one model.
- If a permission is not explicitly allowed, then the agent shall be denied rather than prompted, because a non-interactive run cannot answer a prompt.

## Verification Contract
- A smoke run shows one streamed model and a denied commit attempt.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
