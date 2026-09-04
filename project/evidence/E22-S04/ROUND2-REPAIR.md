# E22-S04 - Round 2 repair (independent verifier review round 1 findings)

- Story: E22-S04
- Round: repair after `project/evidence/E22-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04

## Verdict this repairs

Round 1 verdict: FAIL. `cancellai-policy::retention::resolve_codex` gated `keep_latest`
pinning on the tree's effective (max-of-members) mtime but classified each member's own
staleness from its own mtime alone. `cancellai.py::choose_codex_old_sessions` gates the
*entire* tree on `effective_mtime >= cutoff` before any per-member logic runs at all - a
recent child protects an old-looking root completely, not merely from `keep_latest`. The
round-1 direct test (`codex_tree_members_that_disagree_in_age_are_deleted_individually_
when_the_tree_is_not_kept`) pinned the opposite of the reference and passed green, which is
exactly the false-confidence failure mode E22-S04 exists to prevent.

## What changed

- `resolve_codex` (`rust/crates/cancellai-policy/src/retention.rs`) now computes
  `tree_recent = facts.effective_mtime.is_some_and(|m| m >= cutoff)` per tree and passes it to
  `classify`, which treats every member of a recent tree as non-stale regardless of that
  member's own mtime - mirroring `if root_id in protected_roots or effective_mtime >= cutoff:
  continue` in the Python reference.
- `classify`'s `activity` computation gained a `tree_recent` parameter; `resolve_claude`
  (which has no tree grouping) always passes `false`.
- The round-1 test was renamed and its assertion reversed:
  `codex_tree_members_that_disagree_in_age_are_all_protected_by_a_recent_sibling` now asserts
  zero delete actions for a tree whose root is stale but whose child is recent, matching the
  reference.
- Added `codex-mixed-age-tree` to the differential fixture corpus (`tests/fixtures/
  manifest.json`, `tests/fixtures/recipes.py`, classified `NORMATIVE` in
  `scripts/characterize.py`) so the M6 differential gate (`scripts/rust_python_parity.py
  check`) - not only the unit test - catches a future regression of this specific boundary
  case independently. `docs/development/VERIFICATION_STRATEGY.md` records why the unit test
  alone was insufficient evidence here.

## Verification

- `cargo test -p cancellai-policy --lib`: 16/16 pass, including the corrected mixed-age test.
- `python3 scripts/characterize.py generate` then `python3 scripts/characterize.py check`:
  the new `codex-mixed-age-tree` fixture characterizes to `actions: 0` under the Python
  reference (days=30, keep_latest=0) - confirming the whole tree is protected, not merely the
  recent child.
- `python3 scripts/rust_python_parity.py check`: 13 NORMATIVE fixtures (up from 12) match
  across engines in both root-origin scenarios, including the new fixture - the Rust engine no
  longer diverges from the reference on this case.
- `python3 scripts/check_fixtures.py check`: passes with the new fixture reusing the existing
  `subagent_tree` category (already declared codex-only via `category_asymmetry`), so no new
  asymmetry declaration was needed.

## Residual risk

None new. This closes the specific semantic divergence the round-1 review found; it does not
change coverage of any other retention rule.
