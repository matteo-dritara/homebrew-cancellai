# Evidence Packet - E22-S06

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E22 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`

## Outcome

PASS

## Scope

`scripts/check_process.py`'s `VERIFIER_REVIEW_RE = re.compile(r"^(E\d{2})-VERIFIER-REVIEW.*\.md$")`
matched only the epic-scoped review filename shape (`E07-VERIFIER-REVIEW.md`). A story-scoped
review record - used for a standalone CR4 carry-forward review during an epic that has not yet
closed, e.g. `E07-S07-VERIFIER-REVIEW.md` - starts with `E07-S07-...`, not `E07-VERIFIER-...`,
so it never matched the pattern and never counted against E07's ceiling at all.
`project/evidence/` actually holds four review records for E07 (one epic-scoped,
`E07-S07-VERIFIER-REVIEW.md`, `E07-S07-VERIFIER-REVIEW-ROUND2.md`,
`E07-S09-VERIFIER-REVIEW.md`), but the check reported one.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - story-scoped verifier review records count against their epic's ceiling | `VERIFIER_REVIEW_RE` changed to `^(E\d{2})(?:-S\d{2})?-VERIFIER-REVIEW.*\.md$`, still capturing only the epic id in group 1 so a story-scoped filename is counted against its *epic*. Confirmed against the real repository: E07 now reports 4 records (was 1). | PASS |
| AC2 - rounds already committed under the previous counting are recorded as explicit exceptions with a reason | Added `"E07": "..."` to `REVIEW_ROUND_EXCEPTIONS`, naming which three records were previously uncounted and why (they predate the owner-authorized combined verify+fix+close round that superseded them). The fix also surfaced a second, previously-hidden case: `E00-S03-VERIFIER-REVIEW.md` was *also* story-scoped and uncounted, so E00's own exception reason ("ran three rounds") was already stale at 4 real records - updated to say 4 and name the newly-counted record, rather than leaving a now-inaccurate reason in place. | PASS |
| AC3 - the ceiling failure message names which records were counted | Already true of the pre-existing message (`f"{epic_id}: {len(records)} ... : {records}"` - the `records` list was already interpolated); unchanged by this story, verified still present in both the error and warning paths. | PASS |

## Safety Evidence

Not safety-bearing (CR1: a process/governance check, no runtime behaviour).

## Verification Commands

Falsification, matching the story's Verification Contract exactly:

```text
$ python3 scripts/check_process.py check    # baseline, before the fix
WARNING: E00: 3 independent review rounds ... (only 3, E00-S03 not yet counted)
process OK

$ # after the regex fix, before adding exceptions:
$ python3 scripts/check_process.py check
PROCESS ERROR: E00: 4 independent review rounds committed, above the ceiling of 2 (ADR-0014):
  ['E00-S03-VERIFIER-REVIEW.md', 'E00-VERIFIER-REVIEW-ROUND2.md', 'E00-VERIFIER-REVIEW-ROUND3.md', 'E00-VERIFIER-REVIEW.md']
PROCESS ERROR: E07: 4 independent review rounds committed, above the ceiling of 2 (ADR-0014):
  ['E07-S07-VERIFIER-REVIEW-ROUND2.md', 'E07-S07-VERIFIER-REVIEW.md', 'E07-S09-VERIFIER-REVIEW.md', 'E07-VERIFIER-REVIEW.md']
# confirms AC1 (E07 now counted correctly) and that AC2's exceptions were required, not optional

$ # after adding both exceptions with reasons:
$ python3 scripts/check_process.py check
WARNING: E00: 4 ... - recorded exception: 4 records once story-scoped reviews are counted ...
WARNING: E07: 4 ... - recorded exception: 4 records once story-scoped reviews are counted ...
process OK: ADR lifecycle, decision supersession, evidence, review rounds, and generated banners are consistent

# VC1 - a synthetic third story-scoped record for an epic at the ceiling makes check fail
$ cp project/evidence/E06-VERIFIER-REVIEW.md project/evidence/E06-S01-VERIFIER-REVIEW.md
$ python3 scripts/check_process.py check
PROCESS ERROR: E06: 3 independent review rounds committed, above the ceiling of 2 (ADR-0014):
  ['E06-S01-VERIFIER-REVIEW.md', 'E06-VERIFIER-REVIEW-ROUND2.md', 'E06-VERIFIER-REVIEW.md']
$ rm project/evidence/E06-S01-VERIFIER-REVIEW.md
$ python3 scripts/check_process.py check   # restored, OK

# VC2 - the existing repository passes with its recorded exceptions and without any new ones
$ python3 scripts/check_process.py check
process OK ... (only E00, E07 warnings; no epic besides those two is at/above the ceiling)
```

Committed regression test: `tests/test_process.py::ProcessConventionTests::
test_a_story_scoped_review_record_counts_against_its_epics_ceiling` reconstructs the
E06-S01 scenario above against a temporary evidence directory (rather than touching the real
one) and asserts both that it fails and that the message names all three records (AC3).
`test_review_rounds_are_bounded` now also asserts E07 appears in the warnings.

Full local gate set (`pytest`, `ruff`, `mypy`, `gen_docs`, `project_os`, `check_docs`,
`check_workflows`, `check_process`, `release.py check`) re-run and green; this story does not
touch the Rust workspace or fixtures/schemas/parity, so those checks were not re-run beyond
their state from the prior E22 stories in this same session.

## Compatibility

- No product behaviour change. Process/governance tooling only.

## Performance / operability

- Not applicable.

## Documentation updated

- `docs/development/WORK_ITEM_MODEL.md` - "Review is per epic, and bounded to two rounds"
  section now describes story-scoped counting and both recorded exceptions.
- `docs/adrs/0014-epic-closure-is-a-release-and-review-is-bounded.md` - clarifies the
  enforcement mechanism now counts story-scoped records against their epic, and records why
  (the E07 undercounting this story closes).

## Residual risks

- The regex still matches by filename convention (`E##(-S##)?-VERIFIER-REVIEW*.md`), not by
  parsing file content. A verifier review committed under a non-conforming filename would
  still go uncounted - the same class of risk the pre-existing epic-scoped check already
  carried, not a new one introduced here.
- `REVIEW_ROUND_EXCEPTIONS`' reasons are prose a human must keep accurate; this story
  demonstrates that risk concretely (E00's previously-recorded reason was already wrong at 4
  real records before this fix corrected it) rather than eliminating it structurally. A future
  regex or corpus change should re-check every recorded exception's actual count, not just add
  new epics to the dict.

## Verifier verdict

pending
