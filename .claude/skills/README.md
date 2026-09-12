# cancellAI agent-skill pack

Seven Agent Skills that make the cEOS contract **executable by the harness** instead of
pasteable into a chat. They follow the open [Agent Skills](https://agentskills.io) format
(`SKILL.md` + YAML frontmatter, progressive disclosure), so the same pack loads in Claude Code,
Codex, Gemini CLI, Cursor, Copilot and the rest of the skills-compatible ecosystem - which matters
here because `AGENTS.md` assigns the executor and the verifier to *different* agents.

## The one design rule

**A skill points at the contract. It never restates it.**

`docs/`, `project/*.json` and `AGENTS.md` are the source of truth. A skill that copies a rule into
its own prose creates a second truth, and a second truth drifts - which is the same failure mode
this repository already refuses for generated documentation. So every skill here is a *runner*
and an *index*: it injects live state with `` !`command` ``, links the canonical document, and adds
only the procedural glue that no document currently holds.

`_lint_skills.py` enforces it: every repository path and every `scripts/*.py` command a skill names
must still exist.

```sh
python3 .claude/skills/_lint_skills.py
```

## The pack

| Skill | Role | Replaces |
|---|---|---|
| [`orient`](orient/SKILL.md) | session bootstrap from control-plane state | the "run these four commands" preamble in `AGENTS.md` |
| [`story-executor`](story-executor/SKILL.md) | the executor loop, exit at `ready_for_review` | `project/templates/EXECUTOR_PROMPT.md` |
| [`epic-verifier`](epic-verifier/SKILL.md) | forked, context-isolated falsification review | `project/templates/VERIFIER_PROMPT.md` |
| [`adversarial-cases`](adversarial-cases/SKILL.md) | the eleven falsification axes as named tests | prose in `AGENT_PROTOCOL.md` that nothing executes |
| [`risk-gate`](risk-gate/SKILL.md) | CR0-CR4 -> the exact gate commands, run honestly | the check lists in `AGENTS.md` |
| [`rust-kernel-guard`](rust-kernel-guard/SKILL.md) | dependency rings, `forbid(unsafe_code)`, one mutation boundary | ADR-0019 / ADR-0017 read by hand |
| [`evidence-packet`](evidence-packet/SKILL.md) | evidence written from real command output | `project/templates/EVIDENCE_PACKET.md` |

`epic-verifier` runs with `context: fork` so it does not inherit executor reasoning - the context
isolation `AGENT_PROTOCOL.md` requires, enforced by the harness rather than by good intentions.
It still is not an *independent* review when Claude both executed and reviewed; the skill says so
and labels its own output accordingly.

## Hook

`.claude/hooks/guard-generated-docs.sh` refuses a hand-edit to a generated document at the moment
it is attempted, instead of at CI time. It matches on the real path, made relative to the project
directory and compared case-insensitively, because `docs/BACKLOG.md`, `docs/backlog.md` and
`docs/adrs/../BACKLOG.md` are one file on an APFS volume.

Two limits are deliberate. It sees only the structured file-writing tools its matcher names, so a
write through Bash (`sed -i`, a heredoc) never reaches it - matching those would mean parsing
shell, which a guard should not attempt. And it fails **open**: any input it cannot interpret is
allowed through. `scripts/project_os.py check` and `scripts/gen_docs.py --check` remain the
authority on drift and catch every path, including the ones this guard cannot see.

Wire it up in `.claude/settings.json`:

```json
{ "hooks": { "PreToolUse": [ { "matcher": "Edit|Write|NotebookEdit",
  "hooks": [ { "type": "command",
    "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/guard-generated-docs.sh" } ] } ] } }
```

## Governance status

This pack is **agent tooling, not product code**, and it is uncommitted on purpose. Landing it
needs a control-plane entry like anything else: a CR0 story covering the pack, the linter promoted
to `scripts/check_agent_skills.py` with tests and a `pre-commit` hook, and `AGENTS.md` pointing at
it. Until then it is usable and reviewable, but it is not a gate.
