# Evidence Packet - E01-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E01)
- Change Risk: CR2
- Spec version/commit: `scripts/diff_harness.py` + `docs/development/VERIFICATION_STRATEGY.md#differential-comparison-contract` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Comparison ignores only explicitly documented nondeterministic fields | `docs/development/VERIFICATION_STRATEGY.md#differential-comparison-contract` names the exact ignored set: envelope (`generated_at`, `generator`), top-level opaque ids (`inventory_id`, `plan_id`), and per-record opaque ids consumed by natural-key matching (`artifact_id`, `action_id`, `target_artifact_ids`). `scripts/diff_harness.py`'s `ENVELOPE_IGNORED_FIELDS`/`TOP_LEVEL_IGNORED_FIELDS`/per-list `dropped` sets are the single implementation of that list - nothing else is ever excluded from comparison. `test_top_level_set_field_ignores_order` / `test_a_changed_set_field_member_is_caught` prove the one field class compared as an unordered set (`notes`/`safety_invariant_refs`) still catches an actual content change, not just a reorder. | PASS |
| AC2 - Any semantic divergence fails unless whitelisted by an accepted ADR/RFC | Every non-ignored field difference on a matched record, and every record present on only one side, is reported by `compare_documents` (`selftest()` cases 3-5). Nothing in `diff_harness.py` silently accepts a divergence; the whitelist mechanism is the existing `INTENTIONAL_DIVERGENCE` classification (`docs/development/VERIFICATION_STRATEGY.md`'s Python reference contract, reused rather than inventing a second taxonomy) - the contract document states explicitly that a whitelist entry is a recorded ADR/RFC decision, not a code path the comparator special-cases. | PASS |

## Safety Evidence

SI-019 (one mutation boundary, evidence-gated) is this story's declared safety obligation, though this CR2 change touches no mutation code path itself - `diff_harness.py` only reads and compares JSON documents, and `characterize.py`'s calls remain `build_plan`/read-only helpers as established in E01-S04. The connection is indirect but real: `docs/development/MIGRATION_PYTHON_RUST.md`'s M6 gate ("every normative fixture runs Python and Rust... an unexplained semantic divergence blocks cutover") is what stands between an unverified Rust mutation path and being trusted as the single safety executor SI-019 requires. This story is the mechanism that gate depends on; it does not itself decide when Rust's mutation boundary is trustworthy, because no Rust engine exists yet to run it against. A CR4 Safety Verdict does not apply here - it will apply to the Rust safety executor itself, at M8, using this harness as one input.

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

All passed (162 tests, 22 subtests; `diff harness OK: self-test cases all behave as documented`). `scripts/diff_harness.py check` running its own `selftest()` is the "harness self-test catches intentionally injected divergence" verification the story names. Wired into `.pre-commit-config.yaml` (new `diff-harness-selftest` hook) and `.github/workflows/tests.yml`/`AGENTS.md`, mirroring the prior three E01 stories.

Run inside the same local virtualenv as the prior E01 stories (system Python 3.13 is externally managed per PEP 668); CI installs into its own ephemeral runner unchanged.

## Compatibility

- Pure stdlib, pure data comparison; no platform-specific behavior.

## Performance / operability

- `scripts/diff_harness.py check` runs seven small in-memory comparisons; negligible cost.

## Documentation updated

- `docs/development/VERIFICATION_STRATEGY.md` - new "Differential comparison contract" subsection under Differential tests (the story's declared documentation impact).
- `docs/architecture/JSON_CONTRACTS.md` - added `identity_token` to the inventory artifact projection (see Residual risks/cross-story correction below), and cross-linked to the new contract subsection.
- `AGENTS.md`, `.pre-commit-config.yaml`, `.github/workflows/tests.yml`, `pyproject.toml` - wired `scripts/diff_harness.py` into the standard gate set.

## Residual risks

- **A schema gap this story's own design work found and fixed before the epic's review round, recorded because a future reviewer should re-check it independently rather than trust this description:** E01-S03's `docs/architecture/JSON_CONTRACTS.md` never gave `inventory.artifacts[]` a content-derived identity field, only the opaque, engine-assigned `artifact_id`. Attempting to design a differential comparator against that schema showed the gap directly: two independent engines observing the *same* fixture have no reason to assign the same `artifact_id`, so nothing in that document could be used to pair records across engines at all. `identity_token` (already named as a conceptual `AgentArtifact` field in `DOMAIN_MODEL.md`, but dropped from the JSON projection) was added back to `docs/architecture/JSON_CONTRACTS.md`, `scripts/check_schemas.py`, and the golden `inventory.golden.json`, with a falsification test (`test_checker_flags_an_artifact_missing_identity_token`). This is a within-epic, pre-review correction (E01 has not been reviewed yet), not a breaking change to an already-accepted contract - see `docs/development/WORK_ITEM_MODEL.md#story-changes-during-implementation`.
- `explanation.explanations` and `result.action_results` still match by the plan's own opaque `action_id`, not by a fully content-derived key (documented explicitly in both `VERIFICATION_STRATEGY.md` and `diff_harness.py`'s `_action_id_key`). Threading the same `target_artifact_ids` -> `identity_token` resolution `plan.actions` uses through these two document types is straightforward but was left for when a real second engine exists to justify and validate it against, rather than speculatively building it now.
- The harness has never been run against two *different* engines' real output, because only one engine (`cancellai.py`) exists. Its self-test proves the comparison logic behaves as documented against synthetic, deliberately-varied copies of the golden documents; it does not yet prove anything about actual Python-vs-Rust drift, which is M6's job once E02+ produce a Rust candidate.
- `compare_documents`'s duplicate-natural-key detection (`_compare_list`'s "cannot pair unambiguously" branch) is implemented but has no dedicated test in this story - none of the fixture-derived golden documents currently produce a natural-key collision. It is exercised only indirectly (never triggered) by the existing self-test cases.

## Verifier verdict

PENDING - epic E01 review runs once every story in E01 is `ready_for_review` (at most twice per epic, per ADR-0014).
