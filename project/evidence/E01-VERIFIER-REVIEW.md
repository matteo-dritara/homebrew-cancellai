# E01 Independent Verifier Review - Round 1

- Review target: `0a0773d..4b9a755` (E01 executor checkpoints)
- Verifier: Codex
- Date: 2026-08-28

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E01-S01 | PASS | Canonical domain terms and legacy mapping are present in `docs/architecture/DOMAIN_MODEL.md`; docs validation passes. |
| E01-S02 | PASS | Ten synthetic fixtures cover all required categories; fixture validation and adversarial layout tests pass. |
| E01-S03 | PASS | Versioned inventory, plan, explanation, and result contracts specify destructive-action evidence, authority, reversibility, and preconditions; golden contract tests pass. |
| E01-S04 | PASS | All ten fixture characterizations regenerate byte-identically and each has a reviewed taxonomy classification. |
| E01-S05 | FAIL | The cross-engine comparator keys explanation and result records by opaque `action_id`, so semantically identical records with engine-specific IDs diverge. |
| E01-S06 | PASS | The Python reference freeze marker, M6 migration gate, and rollback strategy are documented and enforced by the process check. |

## Reproduction

`scripts/diff_harness.py::_action_id_key` returns `record["action_id"]` for both
`explanation.explanations` and `result.action_results`, while the same module and the
contract describe `action_id` as engine-assigned/ignored for cross-engine comparison.

Starting from each committed golden document, changing only `plan_id` and every contained
`action_id` to a Rust-prefixed value produces unmatched records:

```text
explanation opaque action-id only divergence:
  explanations: record with key 'action-0002' present only in side A
  explanations: record with key 'rust-action-0002' present only in side B

result opaque action-id only divergence:
  action_results: record with key 'action-0001' present only in side A
  action_results: record with key 'rust-action-0001' present only in side B
```

This violates E01-S05 AC1: comparison must ignore only explicitly documented
nondeterministic fields, and E01-S05's contract that records are paired by natural key,
never an opaque engine-assigned ID. It would cause false failures at M6 for conformant Python
and Rust engines.

## Required repair

Define and serialize a stable action correlation key (or propagate the plan action's
identity-token-resolved natural key) into explanations and results, use it for pairing, and
add adversarial tests that rename all engine-assigned IDs while preserving semantics. Update
the JSON and differential-contract documentation together. The existing statement that this
is a residual limitation is not acceptable under E01-S05's acceptance criteria.

## Gate status

- `python3 -m pytest tests -v`: PASS — 165 tests, 22 subtests.
- Documentation, governance, workflow, process, release, fixture, schema, characterization,
  and harness checks: PASS.
- `ruff` and `mypy`: not run; the system Python lacks both modules and no repository virtual
  environment is present. This environment limitation does not affect the reproduced semantic
  failure above.
- `git diff --check`: PASS.

## Verifier verdict

FAIL
