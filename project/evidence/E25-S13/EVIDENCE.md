# Evidence Packet - E25-S13

- Commit/PR: the review-rule alignment on `main`
- Executor: Claude
- Independent verifier: none - the owner waived the Codex round for this session's work
- Change Risk: CR0
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every statement of the rule states the rule in force | `docs/development/AGENT_PROTOCOL.md` in two places (the standing assignment and the verifier procedure) and `.claude/skills/story-executor/SKILL.md`. A grep for "at most twice" and "two rounds" now matches only the audit document, the two ADRs, the generated backlog text quoting a story outcome, and PD-022's own superseded record - all of which are history and should say what the rule was. | PASS |
| AC2 - supersession rather than an edit | PD-022 moves to `superseded` with its decision text rewritten to the convention this register uses ("Superseded by PD-024. ..."), and PD-024 carries the rule in force with the measurement that justifies it. `scripts/check_process.py check` validates the lifecycle. | PASS |
| AC3 - a skill links rather than restates | The skill now sends the reader to `docs/development/AGENT_PROTOCOL.md` instead of carrying its own copy of the count. `scripts/check_agent_skills.py check` confirms the path resolves. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A reader following the skill's copy of a rule that has changed | This is what happened, and it is the reason the pack's governing rule exists. The pack had one restated rule and that is the one that drifted. | Corrected |

## Verification Commands

```text
python3 scripts/check_process.py check       -> ADR lifecycle and decision supersession consistent
python3 scripts/check_docs.py check          -> 286 files, links and safety IDs consistent
python3 scripts/check_agent_skills.py check  -> 8 skills, every cited path resolves
```

## Compatibility

- Documentation and the decision register only. No code, no gate behaviour.

## Performance / operability

- Not applicable.

## Residual risks

- **Nothing enforces this.** No gate compares a rule's statements across documents; this was found
  by grep while looking for something else. A checker that could would need to know what a rule
  *is*, which is not a cheap thing to build, and the cheaper mitigation is the pack rule: do not
  restate.
- **PD-022's text survives in generated documents** (`docs/BACKLOG.md` quotes a story outcome that
  describes the old rule). That is history quoted as history and is left alone.

## Verifier verdict

closed on the owner's waiver; no independent verdict
