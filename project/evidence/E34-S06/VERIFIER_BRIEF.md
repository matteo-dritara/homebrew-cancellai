<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E34-S06
Rendered-by: Claude
Rendered-on: 2026-09-24
Brief-Checksum: 57c9cc5e25e1a02e5e167164ea51b0bc9c049f001378610407679aba19795e5c

<!-- end handoff header -->
# Verifier Brief - E34-S06 - The OpenCode reviewer runs inside an OS sandbox confined to its worktree

Status: ready_for_review | Change Risk: CR2
Outcome: The forked self-review of E34 showed that OpenCode, unlike Codex, ran as the owner's user with no sandbox, so a command or a test the reviewer wrote could write the main tree, the run record or another repository's .git while the harness judged only the worktree's diff. The owner chose on 2026-09-24 to confine it: scripts/review_round.py runs OpenCode under macOS sandbox-exec, writable only in its worktree, that worktree's git directory and OpenCode's own data, state and cache.
Dependencies: E34-S01

## Acceptance Criteria
- When the harness runs an OpenCode reviewer, the system shall run it under an OS sandbox that allows writes only to the review worktree, its git administrative directory and OpenCode's own data, state and cache directories.
- The system shall not allow the sandboxed reviewer to write OpenCode's configuration directory, the main working tree or the run record.
- If no sandbox is available on the platform, then the system shall refuse the OpenCode review rather than run it unconfined.

## Verification Contract
- Tests run a shell under the harness's sandbox and assert writes inside the worktree succeed and writes to the main tree, the run record and OpenCode's configuration fail; a smoke run shows OpenCode completing a session under it.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_PROTOCOL.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
