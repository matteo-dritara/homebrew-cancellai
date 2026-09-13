# Evidence Packet - E26-S04

- Commit/PR: the branch-health step in `orient` on `main`
- Executor: Claude
- Independent verifier: none - the owner waived the Codex round for this session's work
- Change Risk: CR0
- Spec version/commit: `project/epics/E26.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the branch's conclusions are reported | `.claude/skills/orient/SKILL.md` injects `gh run list --branch main --limit 5`, alongside the control-plane and toolchain state it already loaded, so it is read before a story is selected rather than after a push fails. | PASS |
| AC2a - the command actually reports the state | Run against this repository before being trusted, which was the point: `gh` returns `conclusion: ""` rather than `null` for a run still going, and jq's `//` only falls back on `null` - so the first version printed a blank column for every in-progress workflow, which reads exactly like a clean report. It now branches on the empty string and prints `queued` / `in_progress` / the conclusion. | PASS |
| AC2 - unknown is said, not implied | The line above the command says a blank or error result means unknown and that unknown is not green. This is the same rule `check_risk_classification.py` now enforces in code for its own missing evidence (E25-S14); here it is prose, because a skill cannot refuse. | PASS |
| AC4 - the same step for every agent | `AGENTS.md`'s "Before any code change" block carries the command and the rule, because that document is what a non-Claude agent reads and the skill pack is Claude-only. Leaving it in the skill alone would have made an agent's obligations depend on which agent it is, which is the asymmetry E24-S01 exists to prevent. | PASS |
| AC3 - a red branch is a finding | Step 6 of the skill says so, and says it is reported to the owner before work starts rather than worked around - with the four-day MSRV failure named as the reason the step exists. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | An agent reading only the green workflows and concluding the branch is healthy | That is what happened for four days. The step lists every workflow's conclusion rather than a single aggregate, so a red one is visible next to the green ones instead of being averaged away. | Corrected |

## Verification Commands

```text
python3 scripts/check_agent_skills.py check      -> 8 skills; frontmatter and cited paths resolve
python3 scripts/check_agent_toolchain.py check   -> 11 components managed, nothing unmanaged
```

## Compatibility

- Skill text and one added `allowed-tools` entry. No product behaviour, no gate behaviour.

## Residual risks

- **The pack's tool surface grew.** `Bash(gh run list:*)` is read-only and narrowly scoped, and it
  is the first command in this pack that touches the network and acts as the authenticated user -
  `git log` and `python3 scripts/*` do not. The manifest declares the pack's capability as
  `prompt`, which already covers skills carrying `git` and `python3` commands; if the owner reads
  this as a capability change, `project/agent_toolchain.json` is where it is recorded, and no agent
  changes that entry.
- **It is advisory.** A skill cannot refuse to proceed. An agent that reads the red line and builds
  anyway is not prevented by anything here.
- **Only the default branch is checked**, and only the most recent runs.

## Verifier verdict

closed on the owner's waiver; no independent verdict
