# Evidence Packet - E08-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E08 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/architecture/TARGET.md` "Engine / Query API (E08-S04)"

## Outcome

PASS

## Scope

Added `cancellai-policy::views` (`rust/crates/cancellai-policy/src/views.rs`), the first real
occupant of the target architecture diagram's "Engine / Query API" layer (above "Inventory +
Providers" / "Policy + Explanation", below "CLI / TUI / Guardian"). No dedicated crate exists for
that layer yet - `cancellai-cli` previously assembled `status`/`inspect` output ad hoc, per
command, with no shared grouping logic - so this lives in `cancellai-policy`, the crate directly
beneath the box that already produces the `ClassifiedArtifact` collections it reads.

Five grouping functions over `&[ClassifiedArtifact]`, each returning `Vec<Bucket<'a>>` where a
bucket holds `Vec<&'a ClassifiedArtifact>` (borrowed, never cloned):

- `by_provider` - groups by `provider_id`.
- `by_project` - groups by `project_attribution`, with an explicit `ProjectBucketKey::
  Unattributed` bucket for `None` (E08-S02).
- `by_session` - groups a Codex subagent tree under its `RelationshipKind::ChildOf` root
  (E08-S01), falling back to the artifact's own id (including an `Orphaned` artifact, E08-S03,
  which by definition never carries a `ChildOf` relationship). Claude's already-flat sessions
  each land in their own single-member bucket.
- `by_machine` - a single, fixed "local" bucket: `MachineId` does not exist on `AgentArtifact`
  because E18/E19's remote-target work has not landed, and every artifact this single-machine
  build observes is definitionally on the one machine running it. Not invented ahead of that
  evidence (same restraint as `KnownPath` in E08-S02).
- `by_artifact` - the degenerate one-bucket-per-artifact identity view, included for the story's
  own five-dimension outcome and as the baseline every reconciliation test compares against.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Views do not duplicate source data | Every bucket's `artifacts` field is `Vec<&'a ClassifiedArtifact>` (a borrow), the same `'a`-lifetime pattern `retention::ProviderPlanningView` already established for exactly this reason. No `AgentArtifact`/`ClassifiedArtifact` field is copied into a bucket; only grouping keys (`&str` project/provider names, `&ArtifactId` session roots) are derived, and those are themselves borrows into the source data, not new owned copies. | PASS |
| AC2 - Totals reconcile across dimensions with explicit Unattributed bucket | `views::tests::every_dimension_reconciles_to_the_same_total` (new) asserts, for a five-artifact mixed fixture (two providers, an attributed project, an unattributed artifact, a Codex root+child tree), that all five views' combined artifact-id sets exactly equal the input set - and that `by_project`'s buckets include an explicit `Unattributed` key. | PASS |

## Safety Evidence

No safety obligations are listed for this story (`safety_obligations: []`) and none apply: `views`
only reads an already-classified `&[ClassifiedArtifact]` and produces read-only groupings: no
`Action`/`SealedPlan`/mutation path, no new `AuthorityLevel` computation, no field that feeds
`cancellai-safety::authority::effective_authority`. CR1 (observational) is the correct
classification.

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-policy: 29 passed (5 new); 0 failed anywhere
$ cargo deny check                # advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ pre-commit run --all-files
```

New tests, all in `rust/crates/cancellai-policy/src/views.rs`:

- `every_dimension_reconciles_to_the_same_total` (AC2, the core reconciliation property).
- `by_project_groups_the_same_project_name_into_one_bucket`.
- `by_session_groups_a_codex_tree_under_its_root_but_leaves_claude_sessions_apart`.
- `by_machine_is_exactly_one_bucket_containing_everything`.
- `an_empty_inventory_produces_no_buckets_in_every_grouping_dimension` (boundary: `by_machine`
  still reports its one fixed bucket, now empty, rather than omitting the machine that ran an
  empty scan).

Fixtures are synthetic `ClassifiedArtifact`/`AgentArtifact` struct literals (all fields are
`pub`), not filesystem-backed - appropriate here since this story tests pure grouping logic over
already-classified data, not discovery/classification itself (that is `retention.rs`'s own,
filesystem-fixture-heavy test suite, unchanged by this story).

## Compatibility

- No wire-format change: `views` is an in-process query API, not (yet) exposed through
  `cancellai-cli`'s `--json` output. Wiring a command to it is future, separately-reviewable
  scope (e.g. an `inspect --by-project` flag) - this story's outcome is "expose ... views," which
  the public `by_*` functions satisfy as a programmatic API; a CLI surface is not implied.
- No new dependency; no change to any existing crate's public API beyond `cancellai-policy`'s new
  `views` module and its re-exports.

## Performance / operability

- Each `by_*` call is one linear pass over the input slice plus a `BTreeMap` insert per artifact
  (`by_machine`/`by_artifact` are `O(n)` with no map at all). No I/O; no new allocation beyond the
  bucket `Vec`s themselves.

## Documentation updated

- `docs/architecture/TARGET.md`: new "Engine / Query API (E08-S04)" subsection.
- `CHANGELOG.md` Unreleased/Added.

## Residual risks

- No CLI command exposes these views yet - `cancellai-cli`'s `status`/`inspect` still assemble
  their own ad hoc output. Wiring one is a natural next step but a distinct, reviewable UI change
  this CR1 domain-layer story does not bundle in.
- `by_machine`/full multi-machine support remains a placeholder pending E18/E19; this is
  documented, not silently glossed over.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
