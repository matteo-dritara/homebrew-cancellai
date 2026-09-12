# E25 / E26 review record - round 1

- Epics: E25 Engineering System Falsification, E26 Agent Toolchain Governance
- Review target: `1b53a69`, `9ab0791`, `9cfd9e7` on `feat/e24-agent-execution-layer-v2`
- Date: 2026-09-12
- Round: 1

## Reviewer independence

**This is a self-review**, per `docs/development/AGENT_PROTOCOL.md`'s "What \"independent\" can mean
here": Claude reviewing Claude's own work, in an isolated agent context that received none of the
executor's reasoning, with the owner having waived the Codex round. It may repair; it does not
close a CR3/CR4 story, and it must not be cited as the independent verification a CR3/CR4 gate
names. The one CR2 story here (E25-S02) closes on the owner's explicit waiver, recorded as such.

## Verdicts

| Story | Verdict | After repair | CR | Concrete evidence |
| --- | --- | --- | --- | --- |
| E25-S01 | PASS_WITH_RESIDUALS | PASS_WITH_RESIDUALS | CR0 | Six ways the tool could report a confident wrong number, all reproduced. Repaired. |
| E25-S02 | FAIL | PASS_WITH_RESIDUALS | CR2 | The floor was invoked by nothing, the baseline was a permanent per-story exemption, and three surfaces the AC names had no floor. Repaired. |
| E26-S01 | FAIL | PASS_WITH_RESIDUALS | CR1 | Seven unmanaged components of six kinds, plus an inline `curl \| sh` hook and a `.mcp.json` server, all reported as "nothing unmanaged". Repaired. |

## The two findings that mattered

**The toolchain enumerator saw almost nothing.** It inspected `.claude/skills` as a single entry,
`.claude/hooks/*.sh`, and `settings.json` `mcpServers`. Everything else was invisible: a `.py`
hook, an extensionless hook, `.claude/agents/`, `.claude/commands/`, an eighth skill inside the
pack, a nested `<subdir>/.claude/skills/`, an output style, an **inline hook command in
`settings.json`** with no file behind it, and `.mcp.json`. Nine unmanaged components at once, and
the gate said `agent toolchain OK`. The enumerator now covers each of those, names every skill
through the pack's `members`, and reports any unrecognised entry under `.claude/` so an unknown
component kind fails closed rather than arriving unseen.

**The risk floor was invoked by nothing.** `check` reads git history, so by the time it can see a
story's paths the change is already made; `floor` answered on demand and was wired into no hook, no
workflow and no skill. The floor was an audit of the past rather than a constraint on the present.
`check_risk_classification.py commit-msg` now runs from the `commit-msg` hook, where the staged
diff and the story ids are both available, and refuses a commit whose named story declares below
its floor.

## Every finding

| # | Story | Sev | Finding | Repair |
| --- | --- | --- | --- | --- |
| 1 | S01 | Med | A story in two verdict tables silently took the last value; FAIL then PASS read as a clean round. | Ambiguity returns no verdicts - the record reports as not machine-readable. |
| 2 | S01 | Med | A verdict table inside a fenced example was parsed as real verdicts. | `prose_only()` strips fenced blocks before parsing, in both this tool and `check_evidence.py`. |
| 3 | S01 | Med | A record whose filename could not be parsed vanished with no diagnostic - including a self-review with different capitalisation. | Reported under "Review records this tool could not classify"; matching is now case-insensitive and recursive. |
| 4 | S01 | Low-Med | AC rows were counted, never matched: AC5-AC9 satisfied a three-criterion story. | The *set* of criterion numbers is compared against `1..n`, in both tools. |
| 5 | S01 | Low-Med | The headline rate's denominator silently excluded unreadable records. | The report names the excluded count directly beneath the rates. |
| 6 | S01 | Low | Lincoln-Petersen columns were labelled "Round 1 / Round 2" whatever the real numbers. | The column names the rounds actually compared. |
| 7 | S02 | **High** | `floor` was invoked by nothing; the change being made was unconstrained. | `commit-msg` gate, wired into `.pre-commit-config.yaml`. |
| 8 | S02 | **High** | Any commit naming a second story - even in the body - is unattributable and unchecked. Coverage 29 of 127. | Not repaired. Needs a `Story:` trailer convention; carried as a residual and named in E25-S02's packet. |
| 9 | S02 | **High** | `baseline` was a permanent per-story exemption: a new violation on a *higher* surface was still a warning, with the stale note attached. | Keyed on the recorded floor; a higher floor is an error. |
| 10 | S02 | **High** | Three surfaces AC1 names had no floor - `cancellai-model`, root/path resolution, the rest of `cancellai-platform` including `mutation*` (SI-019). Kernel `Cargo.toml` files and `deny.toml` had none, so adding a dependency to the safety kernel was ungoverned. | Twenty-one surfaces now. |
| 11 | S02 | Med | AC3 says the disagreement rate is reported by `process_metrics.py`; it was reported only by the risk checker. | A Risk classification section in the generated report. |
| 12 | S02 | Med | AC2's enforcement was absent: nothing required a CR4 story to carry a second classification. | Reported as a warning for all 18 CR4 stories past `ready_for_review`. Not an error, with the reason recorded. |
| 13 | S02 | Low | `./`-prefixed and absolute paths returned no floor. | `normalise()` before matching, with the same argument the README makes for case-insensitive protected names. |
| 14 | S02 | Med | `(E25-S04/S05/S10)` shorthand matched one id, so a four-story commit attributed 16 files to E25-S04. | Repaired during the review; `StoryIdExtractionTests`. |
| 15-17 | E26-S01 | **High** | The enumerator missed six component kinds, inline hook commands, and `.mcp.json` / `settings.local.json`. | Enumerates all of them and fails closed on anything unrecognised. |
| 18 | E26-S01 | Med | `always_on_tokens` was optional: deleting it reported a cost of zero against any budget. | Required field. |
| 19 | E26-S01 | Med | `decision.date` was optional: omitting it exempted a component from expiry forever. | Required field. |
| 20 | E26-S01 | Med | A component marked `retired` while still installed and wired passed. | An installed retired component is an error. |
| 21 | E26-S01 | **High** | The skill's `allowed-tools` granted `Bash(claude plugin:*)` - install, uninstall, update, enable, disable, prune - the capability its own prose forbids, and the packet's sole evidence for AC6. | `list` and `details` only; no network tool. |

## Claims corrected in the executor's own evidence

Eight, all corrected in the packets rather than left standing. The two that were outright false:
E26-S01's AC6 row claimed the skill's tools granted no install command while they granted six; and
E25-S02's AC3 claimed a report that was produced somewhere else. The rest were true of what was
tested but did not support the criterion as written - the failure mode `check_evidence.py` exists
to make visible, arriving in the same commits that introduced it.

## What held up

`fnmatch`'s `*` does cross `/`, so there is no subdirectory bypass - a claim in an earlier residual
said otherwise and was itself wrong; it is corrected in the packet. The CR-level-as-verdict
regression is genuinely fixed. The privilege/trust matrix is sound. The headline 47% reproduces
against an independent hand count. `render()` is deterministic and the drift check fires on a stale
and a missing report.

## Round verdict

**PASS_WITH_RESIDUALS**, subject to the independence caveat above. Residual work carried forward:
the `Story:` trailer (finding 8), converting AC2's warning to an error, and E25-S06's gate
sensitivity harness, which remains the highest-value open finding in the methodology review.
