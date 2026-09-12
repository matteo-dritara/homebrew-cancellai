# Evidence Packet - E24-S03

- Commit/PR: the E24-S03 commit on `feat/e24-agent-execution-layer`
- Executor: Claude
- Independent verifier: Claude, in isolated agent contexts (owner waived the Codex round for this epic)
- Change Risk: CR0
- Spec version/commit: project/epics/E24.json at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the five clauses exist under "Work one story at a time" | `AGENTS.md`, subsection "Diff discipline": no unrelated improvement or reformatting; no removal of pre-existing dead code; removal limited to what this change orphaned; conformity to the conventions already in the file; flag rather than fix. | PASS |
| AC2 - the dead-code rule is stated as a safety rule with its reason | The clause gives the reason inline: code that looks unreachable may be a barrier whose reachability is the thing under dispute, so removing it is a CR3/CR4 act arriving inside a CR0/CR1 diff. | PASS |
| AC3 - the rule names its own exception | Final paragraph: a story whose purpose *is* the cleanup makes the cleanup the diff, and behavior change stays out of it. | PASS |
| AC4 - the pack points rather than restates | `.claude/skills/story-executor/SKILL.md` gains one boundary bullet naming `AGENTS.md`'s "Diff discipline" and summarising only enough to make the reader open it. `scripts/check_agent_skills.py check` passes. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | n/a | CR0, prose only. No runtime code path, no authority decision. | n/a |

## Verification Commands

```text
python3 scripts/check_docs.py check          -> docs OK, no broken link, nothing unreachable
python3 scripts/check_agent_skills.py check  -> agent skills OK: 7 skills
python3 scripts/project_os.py check          -> governance OK
python3 -m pytest tests -q                   -> full suite green
```

## Compatibility

- Prose in `AGENTS.md`, which every skills-compatible agent reads; nothing harness-specific.

## Performance / operability

- n/a.

## Documentation updated

- `AGENTS.md` - new "Diff discipline" subsection.
- `.claude/skills/story-executor/SKILL.md` - one boundary bullet pointing at it.

## Residual risks

- **Unenforceable by a checker.** "Do not reformat unrelated lines" cannot be mechanically
  distinguished from a legitimate reformat inside a cleanup story without knowing intent. This
  is a review obligation, and it is stated as one. A partial mechanisation - flagging a diff
  whose changed-line count greatly exceeds what the story's documentation impact suggests -
  was considered and rejected as a heuristic that would mostly fire on honest changes.
- **The dead-code clause can be read too strictly.** Dead code that the *current story* created
  must obviously be removed. The clause says "pre-existing" and the orphan clause covers the
  other direction, but a careless reader could freeze genuine cruft forever. The cleanup-story
  exception is the intended release valve.
- **It adds a fourth place an executor must read before editing** (AGENTS.md, the story, the
  linked architecture, now this). Accepted: it sits inside a section the executor already has
  to read, rather than in a new document.

## Verifier verdict

See `project/evidence/E24-VERIFIER-REVIEW.md`.
