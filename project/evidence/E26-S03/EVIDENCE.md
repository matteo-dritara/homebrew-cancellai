# Evidence Packet - E26-S03

- Executor: Claude | Independent verifier: self-review | Change Risk: CR1
- Spec version/commit: `project/epics/E26.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - invocations recorded in a form that survives a session and is committed | `python3 scripts/check_agent_toolchain.py record <id>` writes `project/agent_toolchain_usage.json`, versioned with the project rather than living in one machine's home directory. | PASS |
| AC2 - the report shows invocations since the last decision, beside the cost | `render_report` prints a usage line and, for anything not invoked since its decision, a retirement-candidate list with its always-on token cost. | PASS |
| AC3 - zero invocations since the decision names a retirement candidate | `usage_since_decision` returns `0 since decision` when the last invocation predates the decision date; the report lists those separately. | PASS |
| AC4 - the record holds no transcript, no prompts, no machine paths (and `record` refuses an id the manifest does not name, after review: a usage record for an unmanaged component would make the retirement report describe something nobody decided to carry) | `record_usage` writes exactly `{"invocations": int, "last": "YYYY-MM-DD"}` per component id. `test_recording_writes_identity_and_a_count_and_nothing_else` asserts the key set is exactly those two. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| C-09 (contentless by default) | A usage record leaking content or paths | `test_recording_writes_identity_and_a_count_and_nothing_else` | PASS |
| n/a | Absence read as idleness | `test_no_record_is_unknown` - a component with no record returns `unknown`, and the report states that unknown is not unused, because retiring on missing data is a decision made from nothing. | PASS |

## Verification Commands

```text
python3 scripts/check_agent_toolchain.py record cancellai-skill-pack -> recorded, 1 invocation
python3 scripts/check_agent_toolchain.py report -> usage line and retirement candidates
python3 -m pytest tests/test_governance_extras.py -q -> 27 passed
```

## Residual risks

- **Recording is manual.** Nothing calls `record` automatically, so the numbers reflect discipline
  rather than reality. Making it automatic means a hook on skill invocation, which is a component
  that would itself need managing - and the first thing it would record is itself.
- **The committed record has two entries**, seeded so the report path is exercised rather than
  described; the other nine components read `unknown`, which is the honest state and the one the
  report is designed to say out loud. An independent review pointed out that with no file at all
  AC2 and AC3 were unexercised against real data.
- **A count is not usefulness.** A component invoked once by accident looks the same as one relied
  on daily.
- **Committing usage data makes it a merge-conflict surface** on a file nothing else touches.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.
