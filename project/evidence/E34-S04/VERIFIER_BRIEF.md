<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E34-S04
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: 4eef8d625dfc4c40905296b5434dc66a63a729ff4896e09fe7f6741c716d3d8a

<!-- end handoff header -->
# Verifier Brief - E34-S04 - The toolchain gate sees what OpenCode would load

Status: ready_for_review | Change Risk: CR1
Outcome: scripts/check_agent_toolchain.py enumerates .opencode/ and opencode.json as it enumerates .claude/, so a component the second reviewer runtime carries is managed or fails the gate.
Dependencies: E34-S02

## Acceptance Criteria
- The system shall report every agent, command, plugin, tool, skill and MCP server that .opencode/ or opencode.json declares as a component the manifest must manage.
- If .opencode/ holds an entry the checker does not recognise, then it shall be reported as unrecognised rather than ignored.
- If opencode.json leaves language servers or formatters enabled, including by omission, then the checker shall report them, because both execute code by default.

## Verification Contract
- Unit tests over synthetic .opencode trees and configurations; check passes on the committed repository with the reviewer agents registered.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_TOOLCHAIN.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
