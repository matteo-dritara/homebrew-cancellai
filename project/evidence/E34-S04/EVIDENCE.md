# Evidence Packet - E34-S04

- Commit/PR: the E34 follow-up commit on `main`
- Executor: Claude
- Independent verifier: pending
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

## Verification Commands

```text
python3 -m pytest tests/test_agent_toolchain.py -q   -> 61 passed
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
