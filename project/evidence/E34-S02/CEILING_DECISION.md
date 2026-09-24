# E34-S02 - Review ceiling decision

- Story: E34-S02 (CR1)
- Date: 2026-09-24
- Decided by: project owner, by standing instruction - at most two independent review rounds per
  story; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025, `E34-VERIFIER-REVIEW-ROUND1.md`, `E34-VERIFIER-REVIEW-ROUND2.md`.

## What the rounds found

| Round | Verifier | Verdict | Finding |
| --- | --- | --- | --- |
| 1 | Codex | FAIL | `python3 *` re-admitted `pip install`, file removal and web access that direct rules denied. |
| 2 | Codex | FAIL | After AC1 was narrowed by the owner, `python3 scripts/*` still admitted `check_agent_toolchain.py updates`, which calls `gh api`. Required repair: allow only reviewed script/subcommand pairs whose call graphs cannot reach the prohibited actions. |

## The other solution taken

The round-2 repair was made to its prescription: exact reviewed script/subcommand pairs replace the wildcard, cargo runs offline in the harness environment, and `tests/test_opencode_agents.py` evaluates the committed rules with OpenCode's last-match-wins order - round 1's three commands, round 2's `updates`, `check_platforms.py check`, the `release.py` network commands and cargo offline overrides are all denied, and any allowed script that contains a network marker must carry a written reason its subcommand never reaches it. The residual the owner accepted when narrowing AC1 stands: a test the reviewer writes runs as arbitrary code, bounded by the worktree and the harness (E34-S01).

The forked self-review that was to close this story (`E34-SELF-REVIEW.md`) failed it again on
routes the rounds had not tried; each is repaired and pinned in the evidence packet's
"Repairs after the forked self-review" section. The part no repair inside this story can reach -
an unsandboxed OpenCode writing outside its worktree - the owner assigned on 2026-09-24 to a new
story, E34-S06 (an OS sandbox for the reviewer).

The story is CR1: it closes on this decision plus a forked self-review
(`E34-SELF-REVIEW.md`), not a third independent round. The independent reviewer may still reopen it
if a later round over E34 or a story that uses the harness finds the repair unsound.
