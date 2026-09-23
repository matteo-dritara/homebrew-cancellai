# Evidence Packet - E16-S08

- Commit/PR: the commit carrying this packet on `main`
- Executor: Claude
- Independent verifier: Codex (pending, E16 round 2)
- Change Risk: CR3
- Spec version/commit: `project/epics/E16.json` E16-S08, recorded from the E16 round-1 residual
  (`project/evidence/E16-VERIFIER-REVIEW.md`, E16-S06 row)

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "If a registry entry above Untrusted names a fixture reference that does not exist in the repository, then the provider trust check shall fail and name that reference." | `tests/test_provider_trust.py`: `test_a_nonexistent_fixture_reference_is_refused` (the reviewer's exact counterexample), `test_a_nonexistent_reference_is_refused_even_beside_a_real_one`, `test_the_whole_registry_check_applies_the_fixture_rule` | PASS |
| AC2 "If a fixture reference is absolute, contains a parent-directory component, or resolves outside the repository (including through a symbolic link), then the provider trust check shall fail." | `test_an_absolute_fixture_reference_is_refused` (POSIX absolute and `C:\` drive form), `test_a_parent_directory_reference_is_refused_even_when_it_lands_inside` (`tests/../tests/fixtures` and a backslash `..\outside`), `test_a_symlink_escaping_the_repository_is_refused` (skipped on Windows), `test_an_empty_fixture_reference_is_refused` | PASS |
| AC3 "When every fixture reference of a promoted entry resolves to an existing path inside the repository, the provider trust check shall accept the entry." | `test_a_promotion_with_real_evidence_is_accepted` (one directory and one file under a temporary root); `test_an_untrusted_entry_does_not_need_fixtures_to_exist`; `test_the_real_corpus_is_valid` | PASS |
| AC4 "The provider trust check shall remain read-only and shall not execute the referenced fixtures." | `fixture_reference_error` calls only `Path.resolve()`, `is_relative_to()` and `exists()`; no open/read/exec of the referenced path. The script's CI job still runs under `contents: read`. | PASS (inspection) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-021 | A `builtin_verified` entry with a verifier and an invented fixture path | refused with `does not exist in the repository` | PASS |
| SI-021 | Evidence pointed outside the reviewed tree by absolute path, `..`, or symlink | each refused | PASS |

Mutation check (each guard disabled in turn, then restored): disabling the existence check
failed 3 tests; disabling the containment check failed 1; disabling the parent-component check
failed 1.

## Verification Commands

```text
python3 -m pytest tests/test_provider_trust.py -q     -> 25 passed
python3 -m pytest tests -q                            -> 676 passed (after process-metrics regeneration)
pre-commit run --all-files                            -> all hooks passed (after regeneration)
python3 scripts/check_provider_trust.py check         -> provider trust OK: 3 manifest(s) linted
python3 -m mypy scripts/check_provider_trust.py       -> Success
```

## Compatibility

- Python stdlib only. `Path.is_relative_to` needs Python 3.9+, below the repository's floor.
- The committed registry has no promoted entry, so nothing that passed before now fails.

## Performance / operability

- One `resolve()` and `exists()` per fixture reference of a promoted entry; negligible.

## Documentation updated

- `.github/CONTRIBUTING.md` ("Provider contributions"), `docs/development/ENGINEERING_SYSTEM.md`,
  `CHANGELOG.md` (Unreleased / Fixed), the checker's module docstring.

## Method defects

- none

## Residual risks

- Existence is not execution: a fixture that exists but is exercised by no test still satisfies
  the check. Tying a reference to a test that consumes it would need a convention for how Rust
  and Python tests name their fixtures, which does not exist yet; the `CODEOWNERS` review of
  `project/provider_trust.json` remains the control for that property.
- After round 3: the fixture must be tracked by Git, and every Git failure refuses. The final
  code was verified by executor tests and mutation, not by a third independent round (see
  `CEILING_DECISION.md`).

## Review history

- Round 2 (Codex): REPAIRED - `project/evidence/E16-VERIFIER-REVIEW-ROUND2.md`.
- Round 3 (Codex): FAIL - `project/evidence/E16-VERIFIER-REVIEW-ROUND3.md`; repaired to its
  exact required repair, closed by owner decision at the review limit -
  `project/evidence/E16-S08/CEILING_DECISION.md`.

## Verifier verdict

See the review history above; no further independent round was run.
