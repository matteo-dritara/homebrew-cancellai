# Evidence Packet - E07-S08

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: none (owner-authorized closure - see "Closure" below)
- Change Risk: CR2
- Spec version/commit: `scripts/rust_python_parity.py`, `docs/development/
  MIGRATION_PYTHON_RUST.md` (M6 section)

## Outcome

PASS

## Scope

Implements this story's outcome, "Make the Python/Rust parity gate prove approved,
fixture-specific semantic equivalence beyond deletion UUIDs and root state" - the carry-forward
backlog item E06 verifier review round 2 opened for `E06-S02`'s two surviving findings
(`project/evidence/E06-VERIFIER-REVIEW-ROUND2.md`), closed here rather than left in `planned`.
The repair itself (root cause, mechanism, and code) is recorded in full in `project/evidence/
E06-S02/EVIDENCE.md`'s "Repair for the round-2 finding" section - not duplicated verbatim here.
This packet states the story-level AC/verification mapping and closure decision.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - A divergence allow-list is structured and validates that its cited accepted ADR/RFC specifically authorizes the fixture and semantic difference being suppressed | `ApprovedDivergence(fixture_id, scenario, fields, citation)` replaces the free-text `dict[fixture_id, str]`; `_citation_covers` resolves the citation to a real `Status: Accepted` ADR/RFC under `docs/adrs/`/`docs/rfcs/` whose own text names this exact `fixture_id` - not merely any accepted document. `self_test`'s `unrelated_citation` case reproduces round 2's exact repro (an accepted-but-unrelated ADR) and proves it no longer suppresses. | PASS |
| AC2 - The comparison projects discovered identity records, protection/unknown coverage, scan completeness, root authority, and all proposed actions for every NORMATIVE fixture | `semantic_projection` grew from 6 to 8 fields: `candidates` + `non_delete_identities` together are every discovered session UUID on both engines ("discovered identity records"/"every proposed action" - an artifact is always either a delete candidate or not); `protected_count` is protection coverage; `scan_complete` is this codebase's existing unknown-coverage vocabulary (SI-008/SI-009); `root_origin`/`root_confidence`/`mutation_eligible` is root authority. `check()` runs this for all 10 `NORMATIVE` fixtures in both `default`/`custom` root-origin scenarios (20 comparisons). | PASS |
| AC3 - Injected divergences for every projected field, including an unrelated accepted ADR citation, fail the gate | `self_test` injects a mismatch for every one of the 8 fields (`extra_candidate`, `missing_candidate`, `withheld_mismatch`, `root_origin_mismatch`, `scan_complete_mismatch`, `non_delete_mismatch`, `protected_count_mismatch`, plus the pre-existing `identical`/`uncited` baseline cases), the unrelated-citation case, and a field-scoping case (an approval naming one field must not silently approve a different diverging field on the same fixture/scenario, and a `default`-scoped approval must not cover `custom`). | PASS |

## Verification Commands

```text
.venv/bin/python scripts/rust_python_parity.py self-test
.venv/bin/python scripts/rust_python_parity.py check
.venv/bin/python -m ruff check .
.venv/bin/python -m ruff format --check .
.venv/bin/python -m mypy scripts/rust_python_parity.py
```

All green: `self-test` - "the comparator correctly catches every injected divergence class";
`check` - "10 NORMATIVE fixture(s) match across engines, in both root-origin scenarios".

## Compatibility

- Runs against the built `cancellai-cli` binary (native target) in both root-origin scenarios
  the corpus already exercises; no platform-specific behavior in the comparator itself.

## Performance / operability

- `self-test` runs in milliseconds (pure `_compare_results`/`_citation_covers` calls, no engine
  invocation). `check` builds and runs both engines against 10 fixtures x 2 scenarios; unchanged
  order-of-magnitude runtime from before this repair (a handful of seconds).

## Documentation updated

- `docs/development/MIGRATION_PYTHON_RUST.md` M6 section: describes the structured
  `ApprovedDivergence` mechanism and the 8-field projection in place of the prior free-text/
  6-field description.

## Residual risks

- A full per-artifact `knowledge_confidence`/`integrity_state`/`risk_class` diff remains out of
  scope - Python's `Action` model has no equivalent per-artifact vocabulary to compare against
  today. This is a materially larger undertaking than this repair and is not silently claimed as
  covered by the 8-field projection above.
- The allow-list mechanism is exercised by `self_test`'s synthetic cases; it has never actually
  suppressed a real divergence in `check()` (`INTENTIONAL_DIVERGENCES` is empty - all 10
  fixtures currently match exactly), so its behavior against a real, intentionally-approved
  future divergence is proven only synthetically, not by a live example.

## Closure - 2026-09-01, owner-authorized

`AGENTS.md`'s standing process is executor -> independent verifier -> owner. For this specific
carry-forward item, the owner (chat session `session_01UHbEhSMb1QWc7gNTJnGeu2`, 2026-09-01)
reviewed the round-2 FAIL finding this item tracks, the repair recorded in `E06-S02`'s evidence
packet, and this packet's AC table, and explicitly instructed closing this story to `done`
without opening an E07-scoped independent review round first ("non serve un'altra review, hai
la mia approvazione"). This is CR2 - `AGENTS.md`'s CR4-only independent-verification/Safety
Verdict requirement does not apply (contrast `E07-S07`, the CR4 sibling item from the same
review round, left at `ready_for_review` under this same instruction - see its own evidence
packet for why).

## Verifier verdict

None independent - see "Closure" above. Owner decision: ACCEPT.
