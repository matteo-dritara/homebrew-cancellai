# Evidence Packet - E25-S01

- Commit/PR: the E25-S01 commit on `feat/e24-agent-execution-layer-v2`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR0
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS (executor self-assessment; this story exits at `ready_for_review`, unlike E24, because the
owner's waiver of the independent round covered E24 only).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - generates and drift-checks the report | `scripts/process_metrics.py` with `report`/`generate`/`check`; `project/generated/PROCESS_METRICS.md` committed. `tests/test_process_metrics.py::StaleReportTests` covers a stale report and a missing one, both exit 2. | PASS |
| AC2 - distinguishes reviewer and record scope | `ReviewRecordClassificationTests` - an epic round, a numbered round, a pre-ADR-0014 story-scoped record, a self-review, and an unrelated evidence file. The story-scoped case is the one that matters: counting `E07-S07-VERIFIER-REVIEW.md` as an epic round invented rounds that never happened and corrupted the yield table during development. | PASS |
| AC3 - an unparsable record is not reported as zero | `render()` emits **not machine-readable** rather than `0`; visible for `E00` round 3 in the committed report. `VerdictParsingTests::test_a_change_risk_level_is_not_a_verdict` pins the regression that produced the confident wrong number. | PASS |
| AC4 - registered as a gate, carries the banner | `.pre-commit-config.yaml` hook `process-metrics-check` (`always_run`); `tests.yml` and `release.yml` steps; `scripts/check_process.py`'s `GENERATED_FILES` now includes the report, and `check_process.py check` passes. | PASS |
| AC5 - states what the numbers cannot support | The first-pass-rejection section says the self-review sample is tiny, that the published self-preference-bias range is wide, and that the split can neither confirm nor rule out bias - then states the conclusion that does not depend on the split. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A measurement tool reporting a confident wrong number | `VerdictParsingTests` - four cases, including the CR-level regression found during development and the prose-mention case. The tool now reports "not machine-readable" where it cannot parse, which is the only acceptable failure mode for an instrument nothing downstream contradicts. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_process_metrics.py -q -> 23 passed (the count was 20 when written; a later commit added tests and the packet was not refreshed - found by review)
python3 scripts/process_metrics.py check          -> process metrics OK
python3 scripts/check_process.py check            -> process OK (banner enforced)
python3 scripts/check_docs.py check               -> docs OK, the report is reachable from docs/INDEX.md
python3 -m mypy                                   -> Success: no issues found in 14 source files
python3 -m ruff check . / format --check .        -> clean
```

## Compatibility

- Stdlib only. The single subprocess call is `git log` with a fixed argument list, and a missing or
  failing `git` degrades to an empty rework proxy rather than an error, so the tool works in a
  source export with no history.

## Performance / operability

- Reads 168 evidence files and 25 epic files; completes well under a second.

## Documentation updated

- `docs/INDEX.md` links the report and the methodology review.
- `AGENTS.md` check list, `.pre-commit-config.yaml`, both workflows, `pyproject.toml`.
- `docs/audits/2026-09-12-METHODOLOGY_REVIEW.md` is the analysis the tool serves.

## Residual risks

- **The measurement depends on a filename convention.** A self-review committed under a
  `VERIFIER-REVIEW` name is silently counted as independent and inflates the headline statistic.
  E25-S05 turns the convention into a rule; until then this is an unguarded assumption and the
  most likely way these numbers become wrong.
- **Verdict counts come from Markdown tables.** A record that states its verdicts in prose is
  reported as not machine-readable, which is honest but means the headline rate is computed over
  the subset that happens to use a table.
- **`fix:` against `feat:` is a weak rework proxy.** Many `fix:` commits here repair a review
  finding, which is the process working. The number is reported as a trend rather than a level,
  and it should not be cited as DORA's change failure rate.
- **Lincoln-Petersen assumes independent samples with equal catchability**, and the literature is
  unanimous that both assumptions fail in software and the estimate is a systematic under-estimate.
  The tool reports the overlap alongside the estimate for exactly this reason; the overlap is the
  signal.
- **Repaired after review**, which found five ways the tool could report a confident wrong number:
  a verdict table inside a fenced example was parsed as real verdicts; a story appearing in two
  tables silently took the last value, so a record saying FAIL then PASS read as a clean round; a
  record whose filename could not be parsed vanished with no diagnostic at all, including a
  self-review named with different capitalisation; acceptance-criteria rows were counted rather
  than matched, so AC5-AC9 satisfied a three-criterion story; and the headline rate's denominator
  silently excluded unreadable records. All five are repaired and the report now names the excluded
  count and lists any record it cannot classify.
- **This tool measures the process and is part of it.** Nothing measures whether the measurement
  is any good, and a metric that becomes a target stops measuring - the report says so, which is
  not the same as preventing it.

## Verifier verdict

pending
