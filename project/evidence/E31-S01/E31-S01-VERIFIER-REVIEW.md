# Independent Verifier Review — E31-S01

Verifier: Codex
Brief-Checksum: de65386747804f8bee70013ff00471bb814887858f90e5753a2f1aa465266999
Date: 2026-09-15

## Verdict

REPAIRED

The story honestly limits itself to identifier existence, not the currency of prose claims; E30 is
the adjacent state-semantics repair, so this separation is coherent rather than a convenient claim
of solving stale prose. The documented regex/code-fence/URL and retired-citation limitations also
remain real residuals.

The retirement mechanism nevertheless missed its own AC. `retired_work_items()` converted records
to a dictionary, silently collapsing duplicate ids despite "recordable once"; an entry without
`became` passed because validation only checked a truthy successor; the date and reason were not
validated by the checker. The repair validates the file shape, unique retired ids, non-current ids,
resolving successors, date format, and non-empty reason. Invalid records no longer whitelist their
citations.

## Reproductions and verification

- Before repair, a duplicate retirement id collapsed into one dictionary entry, and removing
  `became` from a record produced no retirement-record error.
- New adversarial tests mutate the committed registry to reproduce both omissions.
- `python3 -m pytest tests/test_work_item_references.py -q` — passes with the committed registry.
- `python3 scripts/check_docs.py check` passes, and continues to reject an invented work-item id.

## Method defects

- none.

## Residual risks

- A retired id remains accepted wherever cited; the checker cannot determine whether the sentence
  correctly describes its replacement. It also validates existence, not whether a true id is
  current in its prose context.
- This is retrospective review of a closed `done_no_release` epic. It does not reopen E31; the
  owner should reopen it if the repaired finding changes that decision.
