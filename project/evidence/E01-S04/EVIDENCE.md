# Evidence Packet - E01-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E01)
- Change Risk: CR2
- Spec version/commit: `scripts/characterize.py` + `tests/fixtures/characterization/*.characterization.json` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Each fixture records observed output and whether it is normative, intentionally changed, or legacy-only | `scripts/characterize.py` runs `cancellai.py`'s real `build_plan`/`coverage_payload` against every one of the 10 E01-S02 fixtures (patched in as the *default* provider root, matching `tests/test_cancellai.py::use_as_default_roots` - otherwise ADR-0013's custom-root inspection-only rule would mask everything else being characterized) and writes one `tests/fixtures/characterization/<fixture-id>.characterization.json` record per fixture with a `classification` field. `test_every_manifest_fixture_has_a_reviewed_classification` proves every manifest fixture has exactly one reviewed classification, no more, no fewer. | PASS |
| AC2 - Known defects are marked non-normative | The classification enum is `NORMATIVE \| INTENTIONAL_DIVERGENCE \| LEGACY_ONLY \| KNOWN_DEFECT` (`docs/development/VERIFICATION_STRATEGY.md`'s existing taxonomy); `characterize_one` refuses to run a fixture through an invalid classification value (`test_characterize_one_rejects_an_invalid_classification_value`). All 10 current fixtures classify `NORMATIVE` because none of them currently reproduce a known Python defect - every fixture's observed behavior is traced to a specific already-closed E00 story or documented invariant in its `classification_rationale` (see the committed JSON files), not asserted without justification. Should a future fixture reproduce a live defect, `KNOWN_DEFECT` is the mechanism this story adds for marking it non-normative rather than silently treating it as the contract. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story. `characterize.py` never calls `execute_plan`/`safe_remove`; it only calls `build_plan(..., for_mutation=True)` (which decides what *would* be selected, still pure data) and read-only helpers (`plan_summary_dict`, `root_entry_sizes`, `coverage_payload`). No fixture's filesystem is ever mutated by this story's code.

## Verification Commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/characterize.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All passed (156 tests, 22 subtests; `characterization OK: 10 fixtures match their committed characterization`). `scripts/characterize.py check` is the "golden characterization suite is reproducible on clean checkout" verification named in the story's contract: it regenerates every record in memory from a fresh temp tree and diffs it byte-for-byte against the committed JSON, run three times in a row during development to confirm determinism (`test_committed_characterization_is_reproducible_across_runs`). Wired into `.pre-commit-config.yaml` (new `characterization-check` hook) and `.github/workflows/tests.yml`/`AGENTS.md`, mirroring E01-S02/E01-S03.

Run inside the same local virtualenv as the prior E01 stories (system Python 3.13 is externally managed per PEP 668); CI installs into its own ephemeral runner unchanged.

## Compatibility

- Pure-stdlib, POSIX chmod semantics only (matching `tests/test_cancellai.py`'s existing convention for the partial-tree case). No platform-specific behavior beyond what the fixture corpus already relies on.

## Performance / operability

- `scripts/characterize.py check` rebuilds and plans against 10 small synthetic trees; negligible cost (well under a second locally).

## Documentation updated

- `docs/development/MIGRATION_PYTHON_RUST.md` - M1 now names the characterization suite concretely and states that only `NORMATIVE` records bind the Rust candidate at M6 (the story's declared documentation impact).
- `tests/fixtures/README.md` - new "Python behavior characterization" section explaining the record format, the default-root patch, and the classification taxonomy.
- `AGENTS.md`, `.pre-commit-config.yaml`, `.github/workflows/tests.yml`, `pyproject.toml` - wired `scripts/characterize.py` into the standard gate set.

## Residual risks

- **A fixture defect found and fixed during this story, recorded because a future reviewer should re-check it independently rather than trust this description:** the first `claude-partial-tree` recipe (from E01-S02) locked an *unrelated* sibling subdirectory, which `discover_claude_sessions` never walks into at all - it only recurses into a session's own companion payload directory (`projects/<project>/<session-id>/`). Running the real discovery path during this story's work showed `scan.complete: True` where the fixture's whole purpose was to be incomplete. The recipe now locks a real companion payload directory instead (`tests/fixtures/recipes.py::_claude_session_with_payload`), and `tests/test_fixtures.py::test_partial_tree_fixture_produces_an_incomplete_scan_on_the_real_discovery_path` now asserts this against `build_plan` itself, not only against the lower-level `directory_size` helper used before. This is exactly the kind of gap E01-S04 exists to surface - a fixture that looked right by construction but did not reproduce the real code path - and it was caught by generating the characterization and reading the actual output rather than trusting the fixture's docstring.
- The characterization corpus currently only covers Python's *destructive-plan* view (`build_plan(..., for_mutation=True)` with the fixture patched in as the default root). It does not yet characterize the `for_mutation=False` ("status") view, `execute_plan`'s actual mutation/result path, or `--dry-run` specifically. Extending characterization to those is future work if a later story needs it; this story's scope is the plan-selection behavior the fixture corpus was built to exercise.
- All 10 current classifications are `NORMATIVE`. This corpus does not yet contain a fixture that reproduces a genuine `KNOWN_DEFECT`, `INTENTIONAL_DIVERGENCE`, or `LEGACY_ONLY` case - those categories are exercised mechanically by `test_characterize_one_rejects_an_invalid_classification_value` (rejects an invalid enum value) but not by a real example, because none currently exists in this reference build. A reviewer should not read "all NORMATIVE" as the taxonomy being untested; the taxonomy's *enforcement* is tested, its *real-world use* is pending a fixture that needs it.
- `docs/development/VERIFICATION_STRATEGY.md`'s "Python reference contract" section already defined this four-value taxonomy before this story; this story is the first to actually populate committed records against it.

## Verifier verdict

PENDING - epic E01 review runs once every story in E01 is `ready_for_review` (at most twice per epic, per ADR-0014).
