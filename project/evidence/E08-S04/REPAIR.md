# E08-S04 - Repair (independent verifier review round 1 finding)

- Story: E08-S04
- Round: repair after `project/evidence/E08-VERIFIER-REVIEW.md` (round 1, FAIL on E08-S04 only)
- Date: 2026-09-11

## Verdict this repairs

Round 1 verdict: FAIL. `cancellai-policy::views::by_session` followed exactly one `ChildOf`
edge - the artifact's *immediate* parent, which is all `cancellai-policy::retention::classify`
ever records per artifact (E08-S01) - instead of resolving a chain to its ultimate root. A
three-level Codex tree (grandparent <- parent <- child) therefore produced two buckets instead
of one. The same function also trusted a `ChildOf` target verbatim as the bucket key even when
that target was absent from the slice `by_session` was actually called with (a real, foreseeable
case: a caller may pass a filtered view - e.g. "delete candidates only" - that excludes a tree's
own root), which round 1 reproduced concretely:

```text
cargo test -p cancellai-policy by_session_groups_a_transitive_child_chain_under_the_tree_root -- --nocapture
# failed: left bucket count 2, right 1

cargo test -p cancellai-policy by_session_falls_back_to_own_id_when_parent_is_absent_from_the_input_slice -- --nocapture
# failed: left ArtifactId("not-present"), right ArtifactId("child")
```

This violated E08-S04's own outcome ("session" view over one inventory model) and AC2
("Totals reconcile ... over the input inventory"): a target absent from the slice can never
reconcile against that same slice.

## Repair

`by_session` now resolves the *complete* `ChildOf` chain against an index (`HashMap<&ArtifactId,
&ClassifiedArtifact>`) built from the exact slice it was called with, via a new private helper,
`session_root`, that mirrors `cancellai_provider_codex::graph::root_id_for`'s own fail-safe rules
precisely:

- a target not present in the index -> stop at the current position (never propagate the
  unbacked id);
- an artifact with no `ChildOf` relationship -> it is the root;
- a cycle (malformed/adversarial metadata) -> isolate the id the walk actually started from, not
  wherever the cycle was detected.

`docs/architecture/TARGET.md`'s "Engine / Query API (E08-S04)" section and the `CHANGELOG.md`
entry are updated to describe the corrected behavior; the module's own doc comments on
`by_session` explain the fix and cite the finding directly.

## New/extended tests

All in `rust/crates/cancellai-policy/src/views.rs`:

- `by_session_groups_a_transitive_child_chain_under_the_tree_root` - the exact round-1
  reproduction (grandparent/parent/child), now asserts one bucket of three.
- `by_session_falls_back_to_own_id_when_parent_is_absent_from_the_input_slice` - the exact
  round-1 reproduction (a `ChildOf` target absent from the slice), now asserts the artifact's own
  id is the bucket key.
- `by_session_isolates_a_cycle_at_its_starting_point` (new) - a two-cycle (A <-> B) must not
  merge into one bucket or loop; each member isolates as its own single-member session.
- `a_duplicate_source_row_is_preserved_not_collapsed_by_reconciliation` (new) - two
  `ClassifiedArtifact` entries sharing one `ArtifactId` must both survive `by_provider`/
  `by_session`'s reconciliation count, proving multiplicity is actually distinguished from mere
  presence.
- `every_dimension_reconciles_to_the_same_total` (rewritten) - now compares counted id
  occurrences (`BTreeMap<ArtifactId, usize>`) rather than a `BTreeSet`, per the review's own
  requirement ("Reconciliation assertions must compare counted ID occurrences ... so a duplicate
  or dropped source row is observable").

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-policy: 33 passed (4 new for this repair); 0 failed
$ cargo deny check                # advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ pre-commit run --all-files
```

## Outcome

PASS - AC1/AC2 both hold under the corrected implementation; the exact round-1 reproductions now
pass, and the two new adversarial cases (cycle, duplicate id) the review's own repair
instructions asked for are covered.
