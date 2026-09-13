# Evidence Packet - E25-S12

- Commit/PR: the sensitivity-anchor repair on `main`
- Executor: Claude
- Independent verifier: the CI run that rejected the report (`tests` / `lint`, Python 3.14)
- Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every anchor matches exactly one site | `test_every_anchor_is_unique_in_its_file` counts occurrences of each anchor in the file it names. It found two offenders when written: `except OSError` (26 sites) and `"settings.json"` (3). Both are now anchored on the statement they mean. | PASS |
| AC2 - an ambiguous anchor is refused, with the count | `apply_mutant` raises `SensitivityError` naming the number of matches; `test_an_ambiguous_anchor_is_refused` asserts the message says `appears 2 times`. Refusing is the right direction: a mutant that silently lands elsewhere reports a kill for an invariant it never touched. | PASS |
| AC3 - the SI-008 mutant targets this repository's own code | The anchor is now `Scan.record`'s `FileNotFoundError` early return - the single place that decides an unreadable path is worth remembering, and a scope with no recorded error is a scope that hands out destructive authority. `test_the_scan_completeness_mutant_does_not_depend_on_the_interpreter` asserts the anchor against `inspect.getsource(cancellai.Scan.record)`, so moving the code breaks the test rather than silently re-aiming the mutant. | PASS |
| AC4 - a stale report says which row moved | `check` prints a unified diff to stderr. The failure that started this story printed only `the committed report is stale`, and identifying the cause took a Python 3.14 interpreter and a reproduction, which is exactly the work the message should have saved. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | An unreadable path forgotten by the scan, so that absence of evidence becomes absence of data | `Scan.record` mutated to `isinstance(exc, OSError)` -> every unreadable path dropped -> `scan.complete` true -> `build_plan(for_mutation=True)` no longer withholds. `pytest` kills it on 3.13 and on 3.14. | PASS |
| SI-008 | The previous mutant, re-examined | It never reached this invariant. It rewrote `_is_nonempty_file`, a marker validator whose documented contract is to answer `False` on any error, so the mutated function still *reduced* confidence - the safe direction. No product defect: the claim was that a gate had been shown to catch a violation of SI-008, and that claim was false. | Corrected |

## Verification Commands

```text
python3 scripts/gate_sensitivity.py generate  (Python 3.13.15)  -> report A
python3 scripts/gate_sensitivity.py generate  (Python 3.14.7)   -> report B; A == B, byte-identical
python3 scripts/gate_sensitivity.py check                       -> 11 mutants, 11 killed, control clean
python3 -m pytest tests -q                                      -> pass
```

## Compatibility

- Governance tooling only. No product behaviour, no schema, no public API.

## Performance / operability

- Unchanged: the same 11 mutants, the same gate set, ~16s.

## Residual risks

- **This gate is not in `pre-commit`.** It runs the whole gate set eleven times over exported copies
  of the tree, and a commit hook that costs that much gets disabled by the person it is protecting.
  So the report can still go stale locally and fail in CI - now with a diff that says why.
- **Two interpreters were compared, not all of them.** 3.13 and 3.14 are what this repository tests.
  A kill that depends on 3.10 would still go unnoticed until the matrix caught it.
- **Anchor uniqueness is a proxy for anchor correctness.** A unique anchor can still be in the wrong
  function; AC3 is asserted for SI-008 specifically, and nothing enforces it for the other ten.
- **Four of 31 invariants have a killing mutant.** Unchanged by this story, and still the number
  that matters most in that report.

## Verifier verdict

pending
