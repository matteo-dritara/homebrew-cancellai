# Evidence Packet - E31-S01

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: none - the owner waived independent review for this story
- Change Risk: CR1
- Spec version/commit: project/epics/E31.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `check_docs.work_item_reference_errors` reads every markdown file the gate already walks and refuses an id no epic or story defines, naming file, line and id. Shown failing on a reference planted in a real document. `tests/test_work_item_references.py::TheCommittedCorpus::test_an_invented_id_is_refused` | PASS |
| AC2 | `project/retired_work_items.json` records an identifier with `became`, `reason` and `recorded`; a recorded id passes wherever it is cited. `tests/test_work_item_references.py::RetiredIdentifiers::test_every_record_names_what_it_became_and_when` and `::test_a_retired_id_passes_wherever_it_is_cited` | PASS |
| AC3 | A record whose `became` does not resolve is refused. `tests/test_work_item_references.py::RetiredIdentifiers::test_a_successor_that_does_not_resolve_is_refused` | PASS |
| AC4 | `GENERATED_DOCS` excludes the four documents produced from the control plane. `tests/test_work_item_references.py::WhatIsDeliberatelyNotChecked::test_generated_documents_are_excluded` | PASS |
| AC5 | The seven documents citing the cancelled `E07-S07` pass unchanged. `tests/test_work_item_references.py::TheCommittedCorpus::test_a_cancelled_work_item_may_still_be_cited` | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | The change adds a documentation check. It touches no mutation path, no runtime authority and no shipped code. | The mutation-boundary gate (SI-019) is unchanged and still passes. | PASS |

## Verification Commands

```text
python3 scripts/check_docs.py check
python3 -m pytest tests -q
pre-commit run --all-files
```

## Compatibility

- Platforms/providers/schemas exercised: documentation only. `project/retired_work_items.json` is a new file read by one gate; nothing else depends on it.

## Performance / operability

- One extra pass over the markdown corpus the gate already reads. `check_docs.py check` stays under a second.

## Documentation updated

- `docs/development/ENGINEERING_SYSTEM.md`

## Method defects

- **What happened**: the measurement that sized this story scanned `docs/` and reported one unresolvable id. The gate, once written, walks everything `markdown_files()` returns - which includes `project/evidence/` - and found a second one cited seven times. The story was scoped against a number that was wrong because the measurement used a narrower corpus than the thing it was measuring for. It did not change the design, but it could have: a story sized at "one historical reference" reads as smaller than one sized at "two, one of them in the evidence ledger". **Prevented by**: none exists; nothing requires a measurement taken to size a gate to use the same file set the gate will use. **Disposition**: proposed 2026-09-15

## Residual risks

- **This checks existence, not currency**, and the defect that motivated the story was a currency defect: RELEASE_GATES.md called `E16-S05` an outstanding dependency long after it closed, and this gate would not have objected because the id existed. E30-S01 closed that one by moving the claim into a machine-read field. Free prose about work items is still unverifiable, and the story closes a neighbouring defect rather than the one that prompted it.
- A retired identifier passes everywhere once recorded, including in a document that cites it wrongly. The record says what it became; nothing checks that the citing sentence says so too.
- The regex matches `E##` and `E##-S##` anywhere in a line, including inside code fences and URLs. No current document is affected, and a future one containing such a token would fail for a reason its author would not expect.
- The owner initially waived independent review. A retrospective independent review subsequently
  repaired the retirement validator so an incomplete or duplicate record cannot silently pass;
  see `E31-S01-VERIFIER-REVIEW.md` (Codex, 2026-09-15).

## Verifier verdict

REPAIRED — `E31-S01-VERIFIER-REVIEW.md` (Codex, 2026-09-15).
