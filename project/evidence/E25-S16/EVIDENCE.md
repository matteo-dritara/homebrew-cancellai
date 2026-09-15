# Evidence Packet - E25-S16

- Commit/PR: the second sensitivity-anchor repair on `main`, following the `tests` job going red
  after E11 closed
- Executor: Claude
- Independent verifier: none yet - epic E25 is `in_progress` again on this one story; review runs
  at epic scope once it reaches `ready_for_review`
- Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the `story-status-forged` mutant anchors on E19-S02 and matches exactly one site | `scripts/gate_sensitivity.py`'s `story-status-forged` mutant now targets `project/epics/E19.json`, anchored on E19-S02's id/title/status triple. `tests/test_governance_extras.py::MutantIntegrityTests::test_every_mutant_anchor_is_still_present` and `test_every_anchor_is_unique_in_its_file` both pass for it. | PASS |
| AC2 - the selection reasoning is recorded next to the mutant, not just the id | The comment above the mutant records the property used (most unmet transitive dependencies of any planned story in the backlog, 9 at time of writing) and names both prior failures (a story this session was moving; a story whose "future phase" closed anyway) so a third failure updates the same comment instead of rediscovering the pattern. | PASS |
| AC3 - `GATE_SENSITIVITY.md` and the committed report are regenerated | `python3 scripts/gate_sensitivity.py generate` was run against the fixed anchor and committed; `python3 scripts/gate_sensitivity.py check` confirms the committed report is byte-identical to a fresh run. | PASS |
| AC4 - the `tests` workflow reports success after this change | Locally reproduced the exact CI failure (`AssertionError: 1 != 0 : story-status-forged: anchor is not unique`, then `AssertionError: '...status": "planned"' not found in ...`) against E11.json before the fix, then reran the full suite after it: `python3 -m pytest tests -q` -> 528 passed, 512 subtests passed. Not yet independently confirmed by a live GitHub Actions run, since that requires this commit to reach `origin/main` first. | PASS (local); pending live CI confirmation |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| process (`story-status-forged`'s claim) | A story's status is forged from `planned` to `done` without an evidence packet, on the new anchor | `scripts/gate_sensitivity.py check` plants the mutant on E19-S02, runs `project-os check` against the mutated export, and the gate refuses it - `gate sensitivity OK: 11 mutants, 11 killed, every gate clean on an unmutated tree`. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_governance_extras.py -v   -> 34 passed, 182 subtests passed
python3 scripts/gate_sensitivity.py generate            -> wrote project/generated/GATE_SENSITIVITY.md
python3 scripts/gate_sensitivity.py check                -> gate sensitivity OK: 11 mutants, 11 killed, every gate clean on an unmutated tree
python3 -m pytest tests -q                                -> 528 passed, 512 subtests passed
python3 -m ruff check .                                   -> All checks passed!
python3 -m ruff format --check .                          -> 383 files already formatted
python3 -m mypy scripts/gate_sensitivity.py scripts/process_metrics.py scripts/project_os.py -> Success: no issues found
python3 scripts/gen_docs.py --check                        -> docs/CLI.md is up to date
python3 scripts/project_os.py check                        -> governance OK: 24 decisions, 28 epics, 151 stories
python3 scripts/check_docs.py check                        -> docs OK
python3 scripts/check_fixtures.py check                    -> fixtures OK
python3 scripts/check_schemas.py check                     -> schemas OK
python3 scripts/characterize.py check                       -> characterization OK
python3 scripts/diff_harness.py check                        -> diff harness OK
python3 scripts/check_process.py check                        -> process OK (pre-existing E00/E07 round-ceiling exceptions, unrelated)
python3 scripts/release.py check                              -> release OK: v1.14.0 consistent
python3 scripts/release_manifest.py check                     -> release manifest OK
python3 scripts/check_repository_topology.py check             -> repository topology OK
python3 scripts/check_agent_skills.py check                     -> agent skills OK
python3 scripts/process_metrics.py check                         -> process metrics OK (regenerated: adding E25-S16 changed the story/epic counts the report reads from project/epics/*.json)
python3 scripts/check_risk_classification.py check                -> risk classification OK (pre-existing historical warnings, unrelated)
python3 scripts/check_agent_toolchain.py check                     -> agent toolchain OK
python3 scripts/check_evidence.py check                              -> evidence OK (this packet included; pre-existing E00/E07/E20 baseline warnings, unrelated)
python3 scripts/safety_oracle.py check                                -> safety oracle OK
python3 scripts/check_ears.py check                                    -> EARS OK: 454 acceptance criteria classified, 46 unwanted (10%); AC4 above classified "unwanted" per E25-S08's CR2 rule
```

## Compatibility

- Governance/tooling only. No product behaviour, no schema, no public API, no `rust/` change.

## Performance / operability

- Unchanged: the same 11 mutants, the same gate set, comparable runtime to E25-S12's report.

## Documentation updated

- `project/generated/GATE_SENSITIVITY.md` (regenerated)
- `project/generated/PROCESS_METRICS.md` (regenerated - stale after the new story entry, unrelated to the anchor fix itself)
- `docs/ROADMAP.md`, `docs/BACKLOG.md`, `project/generated/PROJECT_STATUS.md`, `docs/DECISION_REGISTER.md` (regenerated by `project_os.py generate` after adding E25-S16)
- `CHANGELOG.md` (this entry)

## Residual risks

- **The initial E19-S02 repair was insufficient.** Its nine unmet dependencies made the mutant
  fail earlier than its claimed evidence obligation. The independent verifier replaced the fixed
  anchor with a dynamic mutation and added a test for the evidence-specific diagnostic; see
  `E25-S16-VERIFIER-REVIEW.md`.
- **AC4 is confirmed locally, not yet by a live CI run.** The GitHub Actions confirmation requires
  this change to land on `origin/main`; recorded as PASS (local) above rather than claimed as a full
  live-CI PASS.
- **`gate_sensitivity.py` is not in `pre-commit`** (carried over from E25-S12's residual risk,
  unchanged by this story): it can still go stale locally between commits and be caught only in CI.

## Verifier verdict

REPAIRED — `E25-S16-VERIFIER-REVIEW.md` (Codex, 2026-09-15).
