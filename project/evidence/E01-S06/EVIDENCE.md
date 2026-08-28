# Evidence Packet - E01-S06

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E01)
- Change Risk: CR0
- Spec version/commit: `AGENTS.md` "Python reference freeze" section + `scripts/check_process.py::check_reference_freeze_marker` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - AGENTS.md prohibits new product features in Python except parity/safety fixes | `AGENTS.md`'s "Current transition state" section is rewritten around a new "### Python reference freeze" heading that states the rule as a *standing* restriction ("does not relax once E01 closes"), replacing the prior "Until epic E01 is complete" framing. Four accepted categories are named explicitly (parity fixes against the committed characterization, safety/security fixes, migration-support tooling, runnability fixes); everything else, "most importantly... new product capability/features" and "a broad refactor... alone," is out of scope without exception. | PASS |
| AC2 - Migration gate and rollback strategy are documented | Both already existed in `docs/development/MIGRATION_PYTHON_RUST.md` before this story (M6 "Differential gate" and the "## Rollback" section) - not new to this change, but this story is the first to point to them from the freeze declaration itself and to make their presence machine-checked (see Verification below) rather than merely hoped to stay in sync. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story (CR0, docs/metadata only - no code path, mutation boundary, or test behavior of `cancellai.py` itself changed).

## Verification Commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/characterize.py check
python3 scripts/diff_harness.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All passed (165 tests, 22 subtests). `scripts/check_process.py check` is the "governance checker confirms reference-freeze marker" verification the story names directly: a new `check_reference_freeze_marker` function fails the build if `AGENTS.md` loses the `"python reference freeze"` heading (case-insensitive substring, matching the existing `check_generated_banners` precedent exactly), or if `docs/development/MIGRATION_PYTHON_RUST.md` loses the words "rollback" or "gate". `test_reference_freeze_marker_missing_is_flagged` and `test_migration_doc_missing_gate_or_rollback_is_flagged` (`tests/test_process.py`) prove the checker actually fails on a synthetic repo missing each piece, not only that it passes on the real one (`test_reference_freeze_marker_present_in_the_real_repo`).

## Compatibility

- Documentation/governance-script only. No behavior, platform, or schema change.

## Documentation updated

- `AGENTS.md` - "Current transition state" rewritten around the new, standing "Python reference freeze" section (the story's declared documentation impact).
- `docs/development/MIGRATION_PYTHON_RUST.md` - M2 now names the concrete enforcement mechanism (`scripts/check_process.py check`) and restates that the freeze is not time-boxed to this epic (the story's other declared documentation impact).

## Residual risks

- The marker check is a case-insensitive substring match, the same mechanism `check_generated_banners` already uses for the same reason (simple, robust to prose editing, impossible to silently satisfy by accident given the specific phrase chosen). It does not parse structure or verify the four accepted-change categories are still listed accurately - a future edit could keep the heading while quietly weakening its content, and only a human reviewer (or `check_docs.py`'s broader reachability check, which still requires the section's own cross-links to resolve) would catch that.
- This story does not itself close epic E01 or run the epic-scope review; it is the last executor-side story in the epic. Review still runs once across all six E01 stories together, per ADR-0014, once all are `ready_for_review` - which is now the case.

## Verifier verdict

PENDING - epic E01 review runs once every story in E01 is `ready_for_review` (at most twice per epic, per ADR-0014). All six E01 stories now meet that condition.
