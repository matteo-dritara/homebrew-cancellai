# E34-S04 - Review ceiling decision

- Story: E34-S04 (CR1)
- Date: 2026-09-24
- Decided by: project owner, by standing instruction - at most two independent review rounds per
  story; recorded by the executor (Claude), who does not thereby become a verifier.
- Related: ADR-0025, `E34-VERIFIER-REVIEW-ROUND1.md`, `E34-VERIFIER-REVIEW-ROUND2.md`.

## What the rounds found

| Round | Verifier | Verdict | Finding |
| --- | --- | --- | --- |
| 1 | Codex | FAIL | A nested directory and a stray file inside `.opencode/` component directories vanished from the walk. |
| 2 | Codex | FAIL | A hidden entry (`.opencode/agents/.hidden.md`) was dropped before it was judged. Required repair: enumerate these entries and classify them as recognised local state or unrecognised; add a hidden-child case. |

## The other solution taken

The round-2 repair was made to its prescription: only `.DS_Store` is skipped as local state, every other entry is judged, and a hidden one is never a recognised component. `test_unknown_entries_inside_component_directories_are_reported` pins nested, empty, stray, skill-without-`SKILL.md` and hidden entries, and a `.DS_Store` that stays silent.

The story is CR1: it closes on this decision plus a forked self-review
(`E34-SELF-REVIEW.md`), not a third independent round. The independent reviewer may still reopen it
if a later round over E34 or a story that uses the harness finds the repair unsound.
