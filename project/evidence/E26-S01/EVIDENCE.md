# Evidence Packet - E26-S01

- Commit/PR: the E26-S01 commit on `feat/e24-agent-execution-layer-v2`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR1
- Spec version/commit: `project/epics/E26.json` at this commit

## Outcome

PASS (executor self-assessment).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - reconciliation runs both ways | **Corrected after review.** The original row was true of `.sh` files in `.claude/hooks/` and of nothing else: an independent review installed seven unmanaged components of six kinds - a `.py` hook, an extensionless hook, a subagent, a slash command, an eighth skill, a nested `.claude/skills/`, an output style - plus an inline `curl \| sh` hook in `settings.json` and an MCP server in `.mcp.json`, and the checker reported *nothing unmanaged*. The enumerator now covers every one of those, names each skill individually through the pack's `members`, and reports any unrecognised entry under `.claude/` so a component kind it has never heard of fails closed. All nine reproduce as errors. | PASS after repair |
| AC2 - capability sets the trust bar | `PrivilegeTests` - `Community` + `executes-code` refused, `Unknown` + `credentials` refused, `Community` + `prompt` accepted, `Vendor` + `executes-code` accepted. `RealManifestTests::test_every_privileged_component_is_trusted_for_it` holds the committed manifest to it. | PASS |
| AC3 - decisions expire, retirement does not | `ExpiryTests` - a decision past the cadence warns, a recent one does not, a retired one never expires, and a malformed date raises rather than reading as fresh. | PASS |
| AC4 - context is budgeted | `BudgetTests` - four 400-token components against a 1000-token budget fail; a retired component consumes nothing. Committed total: 3206 of 6000. **Corrected after review:** `always_on_tokens` was not a required field, so deleting it from every component reported a cost of zero against any budget. It is required now, as is `decision.date`, whose absence exempted a component from expiry forever. | PASS after repair |
| AC5 - user scope is declared intent, stated as such | `test_a_user_scoped_component_is_not_required_to_be_present`; the checker's docstring and `docs/development/AGENT_TOOLCHAIN.md` both state the boundary rather than implying coverage. | PASS |
| AC6 - the skill proposes and installs nothing | **Corrected after review: the original row was false.** It claimed `allowed-tools` granted no install command while granting `Bash(claude plugin:*)`, which covers `install`, `uninstall`, `update`, `enable`, `disable` and `prune` - the capability the prose forbade, handed over in the frontmatter, and the packet's sole evidence for this AC. Now `Bash(claude plugin list:*), Bash(claude plugin details:*)` and no network tool at all. | PASS after repair |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A component silently gaining a shell | `PrivilegeTests`. A privileged capability on `Community`/`Unknown` trust is refused, so a plugin cannot arrive with `executes-code` unless a named vendor stands behind it and the decision records a rationale. | PASS |
| n/a | A date the checker cannot read reading as fresh | `test_a_malformed_date_is_an_error_rather_than_an_assumed_pass`. An unparsable decision date raises; it must never be the case that garbage means current. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_agent_toolchain.py -q -> 27 passed, 14 subtests passed
python3 scripts/check_agent_toolchain.py check     -> 11 components managed, nothing unmanaged
python3 scripts/check_agent_toolchain.py report    -> renders
python3 scripts/check_agent_skills.py check        -> 8 skills
python3 scripts/check_docs.py check / project_os check / check_workflows check -> pass
```

## Compatibility

- Stdlib only, no network. The checker reads the manifest and the repository; it never contacts an
  upstream, which is E26-S02's scope and is deliberately separate because a check that needs the
  network cannot be a gate.

## Performance / operability

- Reads one JSON file and two directories; instantaneous.

## Documentation updated

- `docs/development/AGENT_TOOLCHAIN.md` (new), linked from `docs/INDEX.md`.
- `AGENTS.md` check list; `.pre-commit-config.yaml`; both workflows; `pyproject.toml`.
- `.claude/skills/toolchain/SKILL.md` (new) and `orient` now runs the check at session start.

## Residual risks

- **The always-on token figures are recorded, not measured by this checker.** They come from
  `claude plugin details` at the time of the decision. A component that grows between reviews
  overruns the budget without the gate noticing until someone re-measures. E26-S02 is where that
  becomes mechanical.
- **User-scoped components are unverifiable from CI**, so the strict half of the control covers
  only what is in the repository. Stated in the doc and in the checker rather than papered over.
- **A retired component is now refused while still installed**, after a review showed one could
  leave the budget and the expiry accounting while continuing to execute.
- **Capability is self-declared in the manifest.** Nothing inspects a plugin to confirm it ships no
  hooks. A component that gains a hook in a new version keeps its old declaration until a human
  updates it; E26-S02's capability-growth check is the intended answer.
- **`always_on_tokens` for the in-repo skill pack is an estimate.** The pack's real cost varies
  with description length, and nothing here recomputes it when a skill is edited.
- **The trust tiers are a judgement, not a verification.** "Vendor" means a named organisation with
  a public track record. It does not mean the code was read.

## Verifier verdict

pending
