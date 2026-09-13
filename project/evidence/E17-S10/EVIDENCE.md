# Evidence Packet - E17-S10

- Commit/PR: the release-outcome model on `main`
- Executor: Claude
- Independent verifier: none - the owner waived further review for this stretch
- Change Risk: **CR3**, the floor `project/risk_floors.json` sets for `scripts/release.py`. Filed as CR1; the commit gate refused it, and it was right to - this story changes what the release check will accept
- Spec version/commit: `project/epics/E17.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the state is recordable | A `- Published: yes \| no \| pending` line in each release packet's Source block, written by `prepare` as `pending` and set by `release.py outcome --version X --state ... --reason ...`. All fifteen committed packets are backfilled, and `test_every_committed_packet_records_an_outcome` refuses a packet that has no marker. | PASS |
| AC2 - the check reports a cut version that never shipped | `release.py check` now prints one line per unpublished version with its cause, and asks about any older version whose outcome nobody recorded. It **reports** rather than fails: they are history, and a gate that refuses forever because a release failed in the past is a gate that gets deleted. | PASS |
| AC3 - the lag rule stays as strict where nothing failed | `formula_should_point_at` returns the previous cut version unless that version is recorded as `no`. `test_it_is_exactly_the_old_rule_where_nothing_failed` pins the unchanged case, and `test_a_pending_outcome_does_not_let_the_formula_skip` pins the important asymmetry: unknown is not permission, only a recorded failure moves the pointer back. | PASS |
| AC4 - a failure says what failed | `record_outcome` refuses `no` without a reason. Each of the four backfilled failures names its workflow run and the story that repaired it. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| Release integrity | The new state used to wave a formula past a version that *did* publish | Only `no` moves the expected pointer; `pending` and `yes` do not, and `no` requires a written reason. The formula still cannot be ahead of the source, and after `finalize` it must equal it. | PASS |
| Release integrity | The report hiding a failure by treating an unrecorded version as fine | The opposite: an older version with no recorded outcome is reported as a question, and a test refuses any committed packet without a marker. | PASS |

## Verification Commands

```text
python3 scripts/release.py outcome --version 1.10.0 --state no --reason "..."   -> recorded
python3 scripts/release.py check      -> consistent, and names all four unpublished versions
python3 -m pytest tests/test_release.py -q -> 32 passed, 59 subtests
python3 -m mypy scripts/release.py    -> clean
```

## Compatibility

- Release tooling and the evidence packets' Source block. No product behaviour, no schema.
- Existing packets gained one line each; nothing else in them changed.

## Performance / operability

- Not applicable.

## Residual risks

- **The level was wrong when filed.** CR1 was written for a story described as recording a state;
  what it actually does is change the condition under which a release is judged consistent. The
  floor caught it at the commit gate, which is the third time this session a floor has raised a
  story's level after the work was done.

- **The record is written by hand.** Nothing watches the release workflow and sets it, so the
  state is as current as whoever last ran the command. The report's "no recorded outcome" line is
  the mitigation, and it is a prompt, not a guarantee.
- **It cannot tell you a release published.** `yes` is an assertion by whoever recorded it. The
  backfill was taken from `gh release list` against the tag list, which is how v1.10.0 turned up -
  it had been forgotten entirely, a month before the three failures this story was filed for.
- **Four failures, four different causes**, all of them in jobs that only run on a tag. The model
  now names the state; nothing here makes the pipeline less likely to produce it again.

## Verifier verdict

closed on the owner's waiver; no independent verdict
