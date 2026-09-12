# Evidence Packet - E25-S07

- Commit/PR: the E25/E26 closure commit on `main`
- Executor: Claude
- Independent verifier: self-review in isolated agent contexts
- Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PARTIAL - the predicates exist and hold for the Python reference; the Rust side is not yet checked
against them, and that is stated rather than implied.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the three behaviours stated as predicates from the invariants, not from either implementation | `scripts/safety_oracle.py`: `protected_by_predicate`, `root_permits_mutation_by_predicate`, `eligible_by_predicate`. Each carries the README/invariant text it encodes as a quoted string in `PREDICATES`, and none imports the implementation's helpers. | PASS |
| AC2 - both engines checked against the predicate | The Python reference's **own decision function** is called - `protected_component(path, root, names)` on real paths under a temporary root - for every spelling of every protected name. An earlier version looked for an entry point that does not exist and fell back to reimplementing the rule, which would have compared the predicate to itself: the exact circularity this story exists to break, reintroduced inside the file written to break it. Corrected before close. **Rust is still not checked**, because it needs a compiled harness in `rust/`. | PARTIAL |
| AC3 - VERIFICATION_STRATEGY says which evidence is a regression detector and which is an oracle | `docs/development/VERIFICATION_STRATEGY.md` gains that statement. | PASS |
| AC4 - a disagreement fails even when both engines agree | Structural: the predicate is the reference point, so agreement between engines is never consulted. `SafetyOracleTests::test_the_predicate_can_fail` shows the predicate discriminates. | PASS |
| AC5 - no silent fallback to self-comparison | `check_protected_names` returns "the reference exposes no protected_component(); the oracle would be comparing the predicate to itself" when the entry point is absent. The earlier fallback was exactly that circularity and was removed after review. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-006 | A protected name in a different case or Unicode normal form | `test_a_protected_name_is_protected_in_every_spelling`, and 200 random names that must not be protected | PASS |
| SI-002 / SI-003 | A root that looks like the provider's own | `test_only_the_default_root_permits_mutation` plus 200 generated look-alikes | PASS |
| README retention | Keep-latest bypassed by age, or the age cutoff bypassed at all | `test_keep_latest_protects_regardless_of_age`, `test_the_age_cutoff_is_never_bypassed`, plus monotonicity over 2000 cases | PASS |

## Verification Commands

```text
python3 scripts/safety_oracle.py check    -> 3 predicates hold over 2000 cases (seed 20260912)
python3 scripts/safety_oracle.py describe -> the quoted text each predicate encodes
python3 -m pytest tests/test_governance_extras.py -q -> 27 passed
```

## Compatibility

- Stdlib only; `random` with a fixed seed so a failure is reproducible rather than flaky.

## Performance / operability

- Under a second for 2000 cases.

## Documentation updated

- `docs/development/VERIFICATION_STRATEGY.md`.

## Residual risks

- **The Rust engine is not checked against these predicates.** AC2 is half met and marked so. The
  circularity the story exists to break is therefore broken on one side only.
- **`check_retention` checks the rule's properties, not an engine.** There is no entry point to
  call without importing planning internals, so it verifies that the rule as written has the
  properties it claims - monotonic in `--days` and `--keep-latest`, never bypassing either. A
  divergence between the rule and the planner would not be caught here.
- **`check_protected_names` now refuses to run rather than degrading.** If the reference stops
  exposing `protected_component`, the oracle reports that it would be comparing the predicate to
  itself instead of silently doing so. The earlier fallback is gone.
- **The comparison is one-directional.** It fails when the rule protects a name and the engine does
  not; it does not fail when the engine protects something the rule does not. Over-protection is
  not a hazard for this product, and treating it as one would make the oracle noisy about the
  engine's deliberate `plugins` guard.
- **Three behaviours out of many.** These are the ones where being wrong is unrecoverable; every
  other behaviour still rests on fixtures recorded from the implementation.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.
