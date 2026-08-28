# Executor Evidence - E00 round 4 (response to the third independent review)

- Executor: Claude
- Independent reviewer: Codex - [round 1](E00-VERIFIER-REVIEW.md), [round 2](E00-VERIFIER-REVIEW-ROUND2.md), [round 3](E00-VERIFIER-REVIEW-ROUND3.md)
- Previous executor records: [round 1](E00-EXECUTOR-SUMMARY.md), [round 2](E00-EXECUTOR-ROUND2.md), [round 3](E00-EXECUTOR-ROUND3.md)
- Date: 2026-08-28
- Stories: E00-S01, E00-S02, E00-S03, E00-S04, E00-S05, E00-S06, E00-S07, E00-S08, E00-S09

## Outcome

The single round-3 finding is repaired. Round 3 examined all seven submitted stories and
rejected one: E00-S01, again as a class rather than an instance.

## The finding

`protected_component()` folded case but did not normalize Unicode, so
`protected_component(root / "plügins" / "state", root, {"plügins"})` returned `None`.
The two spellings name the same directory - APFS stores decomposed forms - and the barrier
compared them as different names. SI-001, SI-003 and SI-006.

The protected-name lists this build ships are pure ASCII, so the defect was not exploitable
today. That is not a defence: the barrier's correctness would have depended on nobody ever
adding a non-ASCII protected name, which is exactly the kind of implicit precondition the
first review found in the original constants.

## The repair

`canonical_name()` applies the Unicode canonical caseless form from UAX #15 - NFD, casefold,
NFD again. The second normalization matters because folding can itself emit composed
characters. Both sides of every protected-name comparison go through it.

## Verification

`RoundTwoResponseTests`:

- `test_protected_barrier_normalizes_unicode_before_casefolding` - the reviewer's
  counterexample, retained unmodified;
- `test_protected_barrier_uses_canonical_caseless_comparison` - four combinations of
  composed/decomposed candidate against composed/decomposed protected name, each also with
  case variance, in both directions;
- `test_canonical_name_is_stable_across_form_and_case` - all four spellings collapse to one
  key, and distinct names stay distinct, so normalization does not over-match;
- `test_decomposed_protected_directory_is_refused_at_deletion` - the refusal is asserted at
  `safe_remove`, not only in the predicate.

All four fail against `review/e00-round2-rejected` and pass on `main`, verified in a
detached worktree at that tag with the current test file:

```text
4 failed, 81 passed   (pre-fix tree)
108 passed            (main)
```

## Gates

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py
python3 scripts/gen_docs.py --check && python3 scripts/project_os.py check
python3 scripts/check_docs.py check && python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
pre-commit run --all-files
```

All pass, no governance warnings, 108 tests.

## Residual risks

- **Closure is an owner decision, not a fourth review verdict.** Three review rounds each
  found a class defect in every story they examined: 6 of 7, then 7 of 7, then 1 of 7. The
  trend is downward but the base rate is not zero, and no independent verifier has examined
  this round. Each CR4 story carries an owner acceptance recording that explicitly.
- Case-folded and normalized protection is over-inclusive on a case-sensitive volume: a
  genuinely distinct directory whose name differs from a protected one only by case or
  Unicode form cannot be cleaned. Non-destructive direction.
- The earlier residuals from round 3 are unchanged: custom roots are inspection-only, scan
  completeness is per tool rather than per scope, and process detection is best-effort on
  success while failing closed on observation failure.

## Reviewer verdict

Round 3 rejected E00-S01 and did not examine this repair. Closure was directed by the owner
without a fourth round; see the owner acceptance recorded against each CR4 story.
