# Evidence Packet - E34-S04

- Commit/PR: the E34 follow-up commit on `main`
- Executor: Claude
- Independent verifier: round 1 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND1.md); round 2 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND2.md); round 3 PASS (Codex, E34-VERIFIER-REVIEW-ROUND3.md)
- Change Risk: CR1
- Spec version/commit: `project/epics/E34.json` at this commit; PD-028

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every agent, command, plugin, tool, skill and MCP server from `.opencode/` or `opencode.json` is a component | `_opencode_components` in `scripts/check_agent_toolchain.py`, called by `installed_project_components`. `test_agents_plugins_and_unknown_entries_are_found`, `test_config_components_and_default_code_execution_are_found`. On the real repository the gate first failed on the two reviewer agents; they are now registered as `opencode-reviewer-agents` (owner decision 2026-09-24). | PASS |
| AC2 - an unrecognised `.opencode/` entry is reported | `unrecognised:.opencode/<name>`; OpenCode's own dependency install (`node_modules`, `package.json`, lockfiles, `.gitignore`) is excluded as per-machine state. Same test. | PASS |
| AC3 - language servers and formatters enabled, including by omission, are reported | `lsp:opencode` and `unrecognised:opencode-formatter` unless explicitly `false`; `test_the_committed_configuration_carries_nothing_implicit` shows the committed shape carries nothing. | PASS |

## Repairs after independent round 1 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND1.md` (Codex) failed AC2: in a tree holding `.opencode/agents/known.md`,
`.opencode/agents/unknown/` and `.opencode/skills/rogue.txt`, only `known` was reported; the nested
directory and the stray file vanished.

| Repair | Evidence |
| --- | --- |
| Each component directory admits one shape - flat `.md` agents/commands, flat `.ts`/`.js`/`.mjs` plugins/tools, skill directories holding `SKILL.md` - and every other visible child, nested or empty directory included, is `unrecognised:<path>` | `test_unknown_entries_inside_component_directories_are_reported` (nested dir, empty dir, `.txt` under agents, file under skills, skill dir without `SKILL.md`); fails against the pre-repair walk |
| The committed repository still passes | `check_agent_toolchain.py check`: 13 managed, nothing unmanaged |

## Repairs after independent round 2 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND2.md` (Codex) failed AC2: `.opencode/agents/.hidden.md` was dropped before
it was judged, because the walk skipped every dot-name.

| Repair | Evidence |
| --- | --- |
| Only `.DS_Store` (Finder metadata) is skipped; every other entry is judged, and a hidden one is never a recognised component, so it is reported as unrecognised | `test_unknown_entries_inside_component_directories_are_reported` gains `agents/.hidden.md`, `skills/.hidden/` and a `.DS_Store` that stays silent |

## Repair after the forked self-review (`E34-SELF-REVIEW.md`, 2026-09-24)

A *directory* named `.DS_Store` under `.opencode/agents/`, holding an `.md` with `bash: allow`, was
loaded by OpenCode as a primary agent while the walk skipped it by name.

| Repair | Evidence |
| --- | --- |
| `.DS_Store` is skipped only when it is a regular file (not a directory, not a link); anything else of that name is judged like any other entry | `test_unknown_entries_inside_component_directories_are_reported` gains `.DS_Store` directories under `plugins/` and `skills/` |

## Verification Commands

```text
python3 -m pytest tests/test_agent_toolchain.py -q   -> 62 passed (after the round-1 repair)
python3 scripts/check_agent_toolchain.py check       -> 13 managed, nothing unmanaged
```

## Residual risks

- **User-scope OpenCode configuration** (`~/.config/opencode/`) is outside the repository, like
  `~/.claude` plugins: declared intent, not verified. Today it holds only a `$schema` line and
  OpenCode's own dependency install. The harness's environment (E34-S01) disables `~/.claude`
  loading, which is where the unmanaged content actually was.
- A `.jsonc` configuration with comments is reported `unreadable`, which fails closed.

## Verifier verdict

pending
