# Evidence Packet - E25-S02

- Commit/PR: `9cfd9e7` on `feat/e24-agent-execution-layer-v2`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PARTIAL. The mechanical half of the story - the floor - is complete and gated. The second half of
AC2, a second classification produced without sight of the first, is **structurally supported and
not yet exercised**: the manifest carries `independent_classifications`, the checker validates and
reports them, and zero exist. That is stated rather than claimed as done.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a floor derived from paths, with a named-argument override | `project/risk_floors.json` names nine authority surfaces, each with a reason; `scripts/check_risk_classification.py` fails a story declaring below its floor. `FloorSelectionTests` covers the selection, including that the **highest** floor wins rather than the most specific - a most-specific rule could be defeated by adding a narrow low-level pattern beside a broad high-level one. | PASS |
| AC2 - a second classification, recorded and compared | Structure present and validated (`independent_classifications`, with `EvaluationTests` refusing an unnamed classifier and an unknown story). **No classification has been produced**, so the mechanism is untested in use. | PARTIAL |
| AC3 - the disagreement rate is reported, and zero over many is itself a signal | `disagreement_summary`; `test_a_zero_disagreement_rate_over_many_stories_is_itself_flagged`. `test_no_classifications_says_so_rather_than_reporting_agreement` pins the distinction between "none recorded" and "all agreed", which a summary that conflated them would report as a perfect second opinion. | PASS |
| AC4 - an override names a reason or is refused | `test_an_override_with_no_reason_is_refused`. The error text is "'it is a small change' is not a reason", because that is the argument that will actually be offered. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | Attribution guessing rather than refusing | A first implementation derived a story's paths from every commit mentioning it and flagged 38 stories, most for files they never touched, because this repository batches stories into commits 58 times. Attribution is now refused unless a commit names exactly one story, and the uncovered count is reported. `test_attribution_reports_what_it_could_not_attribute`. | PASS |
| n/a | A baseline becoming a silent permanent exemption | `test_a_baseline_entry_demotes_the_error_to_a_visible_warning`. Every baseline entry prints on every run. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_risk_classification.py -q -> 23 passed, 44 subtests
python3 scripts/check_risk_classification.py check     -> 30 of 127 attributable, 5 baseline warnings
python3 scripts/check_risk_classification.py floor rust/crates/cancellai-safety/src/lib.rs -> CR4
python3 -m mypy / ruff check . / ruff format --check . -> clean
```

## What the floor found

Five stories were already below their floor. Recorded as `baseline`, printed every run, owner
decision pending. One is not routine:

**E20-S04 declared CR2 while touching `rust/crates/cancellai-sealedfs/src/lib.rs`** - the only crate
exempt from `forbid(unsafe_code)` (ADR-0017), and the seam through which every filesystem mutation
is performed. A CR2 declaration removes the adversarial-test, fault-injection, independent-verification
and Safety Verdict obligations from a change to that seam. This is precisely the failure the
methodology review's M-01 predicted, found by the gate written because of it.

## A defect this gate found in itself

On its first run against a later commit, the checker flagged **E25-S04** (CR1) for touching
`scripts/release.py`, which E25-S04 never touched. The cause: this repository writes several
stories in one subject as `E25-S04/S05/S10`, and `E\d{2}-S\d{2}` finds exactly one id in that
string - so a three-story commit read as *unambiguous* and every file in it was attributed to the
first story. That is precisely the confident wrong attribution the design refuses to make, arriving
through a commit-message convention nobody had thought about.

Repaired by `story_ids()`, which expands the shorthand; `StoryIdExtractionTests` pins it. Worth
recording rather than quietly fixing: the gate caught its own author, one commit after being
written, which is the only kind of evidence that a gate can fail.

## Compatibility

- Stdlib only. `git` is resolved absolutely and soft-fails, so a source export with no history
  reports zero attributable stories rather than erroring.

## Performance / operability

- One `git log` over the whole history; under a second.

## Residual risks

- **Coverage is 30 of 127 stories.** Attribution is refused for batched commits, which is correct
  and which means most of the backlog is unchecked. The honest reading of a clean run is "nothing
  attributable is below its floor", and the checker prints exactly that.
- **The floor only binds after a commit exists.** `check` reads history, so it cannot constrain the
  change being made right now. `floor` answers that question on demand and the `risk-gate` skill
  invokes it, but no gate runs it against a staged diff - so an executor who never asks is never
  told.
- **`fnmatch` patterns do not cross directory separators for `*`.** A file in a new subdirectory
  under an authority surface may not match its floor. The current tree has no such subdirectory;
  this is a live gap the moment one appears, and it is the most likely way the floor silently stops
  binding.
- **AC2 is partial.** No second classification exists, so the disagreement rate is undefined rather
  than zero, and the strongest half of the standards' mechanism is present in structure only.
- **The floor list is a judgement.** Nine patterns name what this executor believes are the
  authority surfaces. A surface nobody listed has no floor.

## Verifier verdict

pending
