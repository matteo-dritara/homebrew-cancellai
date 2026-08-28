# Evidence Packet - E01-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E01)
- Change Risk: CR2
- Spec version/commit: `docs/architecture/JSON_CONTRACTS.md` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Schemas include explicit version fields | Every document type (inventory/plan/explanation/result) starts with a fixed 4-key envelope (`schema_version`, `document_type`, `generated_at`, `generator`), in that exact order (`docs/architecture/JSON_CONTRACTS.md#common-envelope`). `scripts/check_schemas.py::check_envelope_order` checks both presence and order; `test_checker_flags_a_missing_schema_version`, `test_checker_flags_an_unrecognized_schema_version`, and `test_checker_flags_envelope_keys_out_of_order` prove the checker actually rejects the three ways this could be violated, not only that the golden corpus happens to have it. | PASS |
| AC2 - Unknown fields have a documented compatibility policy | `docs/architecture/JSON_CONTRACTS.md#compatibility-policy` states the policy precisely: `schema_version` is checked first and an unrecognized one means non-destructive/`OBSERVE`-only (C-03); unknown fields *outside* the safety-critical Action envelope are permitted and ignored (forward compatibility); unknown *enum values inside* the Action envelope (`action_class`, `authority`, `reversibility`) never fall back permissively - they collapse to the strictest value, never inferred upward (C-05, SI-016); removing/renaming a required field is a breaking change requiring a version bump and an ADR. `test_checker_flags_an_unrecognized_authority_value` and `test_checker_flags_an_unrecognized_result_status` exercise the enum half of this mechanically. | PASS |
| AC3 - Every destructive action carries reason, authority, reversibility, and preconditions | `check_action` in `scripts/check_schemas.py` requires all four fields (plus `action_id`, `target_artifact_ids`, `evidence_ids`) on every action, and separately requires at least one `execution_preconditions` entry for every `action_class` other than `OBSERVE` (SI-013/SI-016 - a plan proposing to mutate without stating what must still be true before it does is not a sealed plan). `test_checker_flags_a_mutating_action_with_no_preconditions`, `test_checker_allows_observe_action_with_no_preconditions`, `test_checker_flags_an_action_missing_reason`, and `test_checker_flags_an_empty_reason` prove this mechanically in both directions (rejects the violation, accepts the legitimate `OBSERVE` exception). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 (identity revalidated immediately before mutation) | A `QUARANTINE`/`ARCHIVE`/`DELETE` action with an empty `execution_preconditions` list | `test_checker_flags_a_mutating_action_with_no_preconditions` - the checker rejects it; `docs/architecture/JSON_CONTRACTS.md` names `root_identity_token`/`process_not_running`-shaped preconditions as the mechanism `STALE_PLAN` depends on | PASS |
| SI-016 (mutations require a sealed plan carrying artifact/root identity, policy explanation, authority, action class, reversibility, provider capability, and preconditions) | An action missing `reason`, an unrecognized `authority` value, and empty `evidence_ids` | `test_checker_flags_an_action_missing_reason`, `test_checker_flags_an_unrecognized_authority_value`, `test_checker_flags_missing_evidence_ids` - all rejected | PASS |

This is a CR2 story (documentation impact only touched `docs/architecture/{DOMAIN_MODEL,JSON_CONTRACTS}.md` and `docs/development/VERIFICATION_STRATEGY.md`; no runtime mutation path in `cancellai.py` changed), so a CR4 Safety Verdict does not apply; SI-013/SI-016 evidence above is the safety obligation this story's contract names.

## Verification Commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All passed (147 tests, 12 subtests; `schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md`). `scripts/check_schemas.py check` is the "golden JSON snapshot" validation named in the story's verification contract; it is wired into `.pre-commit-config.yaml` (new `schemas-check` hook) and `.github/workflows/tests.yml` (new lint-job step and mypy target) so it runs identically locally and in CI.

Run inside the same local virtualenv as E01-S01/E01-S02 (system Python 3.13 is externally managed per PEP 668); CI installs into its own ephemeral runner unchanged.

## Compatibility

- Pure specification + stdlib-only validator; no platform-specific behavior. `cancellai.py` is unmodified - this story defines the target-engine contract, it does not change what the frozen Python reference emits (that gap is explicitly out of scope here and tracked for E01-S04/E01-S05, per `docs/architecture/JSON_CONTRACTS.md`'s opening note).

## Performance / operability

- `scripts/check_schemas.py check` parses 4 small golden JSON files; negligible cost.

## Documentation updated

- `docs/architecture/JSON_CONTRACTS.md` - new document defining the four schemas and the compatibility policy (the story's core deliverable).
- `docs/architecture/DOMAIN_MODEL.md` - cross-links `SealedPlan`/`Results` to the new contract doc (documentation-impact target named by the story).
- `docs/development/VERIFICATION_STRATEGY.md` - links the golden-test layer to the new contract doc and fixtures (documentation-impact target named by the story).
- `docs/INDEX.md` - added the new document to the Architecture section so `scripts/check_docs.py`'s reachability check passes and a reader can find it.
- `AGENTS.md`, `.pre-commit-config.yaml`, `.github/workflows/tests.yml`, `pyproject.toml` - wired `scripts/check_schemas.py` into the standard gate set, mirroring how E01-S02 wired in `scripts/check_fixtures.py`.

## Residual risks

- The checker (`scripts/check_schemas.py`) is a hand-written validator against the specific rules this story's AC names, not a general JSON-Schema engine - it does not (yet) check every prose statement in `JSON_CONTRACTS.md` mechanically (for example, the inventory document's rule that a `PARTIAL`/`UNKNOWN` scan scope caps `knowledge_confidence` is documented but not yet enforced in code, since no golden inventory example currently exercises that case). This is consistent with the story's scope (define the contract + prove the AC-named requirements mechanically) rather than exhaustive schema enforcement, which is better done once E01-S04/E01-S05 need it against real characterized data.
- `docs/architecture/JSON_CONTRACTS.md` is a specification for the target engine; `cancellai.py` does not emit this shape today and is not being changed to. E01-S04 ("Characterize Python behavior") is the story that records actual Python output against the E01-S02 fixture corpus, and any gap between that output and this contract becomes explicit tracked work there, not something this story resolves.
- Golden documents are hand-authored examples illustrating the contract, not automatically generated from a running implementation (none exists yet that emits this shape) - a reviewer should read them as "does this match the prose," not as captured real output.

## Verifier verdict

PENDING - epic E01 review runs once every story in E01 is `ready_for_review` (at most twice per epic, per ADR-0014).
