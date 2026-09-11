# Evidence Packet - E09-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E09 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/architecture/TARGET.md` "Machine and project atlas (E09-S02)"

## Outcome

PASS

## Scope

Added `cancellai_policy::atlas` (`rust/crates/cancellai-policy/src/atlas.rs`), the second
occupant of `docs/architecture/TARGET.md`'s "Engine / Query API" layer after `views` (E08-S04):
`summarize(&[ProviderResolution], top_n) -> AtlasSummary` turns already-classified,
completeness-aware provider resolutions into total logical/reclaimable footprint, per-provider
totals (with each provider's own completeness), per-project totals (with an explicit
`Unattributed` bucket), and the `top_n` largest individual contributors.

Wired this into `cancellai-tui`'s Atlas screen: `ui::draw_atlas_summary` renders the summary,
`data::EngineData` is the one new seam the crate reads `cancellai-policy` types through, and
`format::format_bytes` mirrors `cancellai.py::format_bytes`'s exact unit convention (binary 1024
divisor, `B`/`KB`/`MB`/`GB`/`TB`) so the TUI reads consistently with the rest of the product.
`cancellai-tui`'s only new production dependency is `cancellai-policy` itself (plus a
dev-dependency-only `cancellai-model`, needed solely to construct an `ArtifactId` test fixture -
see `Cargo.toml`'s own comment); no provider adapter or filesystem crate is added.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Logical and estimated reclaimable values are visually distinct | `AtlasSummary::total_logical_bytes`/`total_reclaimable_bytes` are separate `u64` fields (`atlas.rs`'s own doc: "never merged... even by accident"); `atlas::tests::logical_and_reclaimable_totals_are_visually_distinct_fields` proves they differ on the fixture. Reclaimability reuses `ClassifiedArtifact::reachable_authority >= cancellai_safety::authority::minimum_authority_for(ActionClass::Delete)` - the identical test `plan`/`clean` already apply, not a new invented predicate. In the UI, `ui::tests::atlas_screen_shows_logical_and_reclaimable_as_visually_distinct_labeled_values` proves both differently-labeled values render; the labels differ regardless of color capability (`reclaimable_style` only adds color/underline on top of the always-different text). | PASS |
| AC2 - Unknown/incomplete scans are prominent and never hidden in totals | `atlas::tests::totals_sum_every_artifact_and_never_hide_an_incomplete_scan` proves the incomplete provider's own bytes still count in the total; `a_root_unavailable_scope_is_unknown_not_merely_partial_and_still_flags_incomplete` covers the `Unknown` (not just `Partial`) case via `ReasonLog::record_root_unavailable`. `ui::tests::atlas_screen_surfaces_an_incomplete_scan_prominently_without_hiding_the_totals` proves the rendered screen shows both the highlighted incomplete-scan line and the full (non-zeroed) total. | PASS |

## Safety Evidence

No safety obligations are listed for this story (`safety_obligations: []`) and none apply:
`atlas::summarize` only reads already-classified `ProviderResolution`s and an already-computed
`reachable_authority` field - it performs no new authority computation, no `Action`/`SealedPlan`,
and no mutation path. CR1 (observational) is the correct classification.

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-policy +7 (atlas), cancellai-tui +11 (format/ui); 0 failed anywhere
$ cargo deny check                # advisories ok (RUSTSEC-2024-0436 still ignored, see E09-S01), bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/check_rust_workspace.py check
$ python3 scripts/check_docs.py check
$ python3 scripts/project_os.py check
```

New tests:

- `cancellai-policy/src/atlas.rs` (7): totals-never-hide-an-incomplete-scan, logical/reclaimable
  distinctness, per-provider completeness carried through, explicit Unattributed project bucket,
  top-contributors ordering and truncation, `Unknown`-not-`Partial` root-unavailable handling,
  an empty resolution set producing zeroed (not absent) totals.
- `cancellai-tui/src/format.rs` (5): the `cancellai.py::format_bytes` unit-boundary cases (0 B,
  sub-KB, exact KiB rollover, MB/GB two-decimal formatting, a `u64::MAX` upper bound).
- `cancellai-tui/src/ui.rs` (+4 new, existing ones updated for the new `draw` parameter): the
  not-loaded placeholder, visually-distinct totals, incomplete-scan prominence, the explicit
  Unattributed bucket rendering.
- `ProviderResolution::for_test` (`retention.rs`, `#[cfg(test)] pub(crate)`): a minimal,
  crate-visible-only constructor so `atlas.rs`'s tests can build a resolution with a specific
  `ScopeCompleteness` without a real `resolve_claude`/`resolve_codex` filesystem round trip -
  `artifacts`/`observation` stay private outside the crate (E21-S04/ADR-0018), unaffected by this.

## Compatibility

- No wire-format change; `AtlasSummary` is an in-process view model, not yet exposed through
  `cancellai-cli`'s `--json` output (same kind of residual E08-S04 recorded for `views`).
- New production dependency: `cancellai-policy` in `cancellai-tui`'s `Cargo.toml` (path
  dependency, already in the workspace - no new external crate, no `Cargo.lock`/`cargo deny`
  impact beyond what E09-S01 already introduced).
- New dev-dependency: `cancellai-model` in `cancellai-tui` (test-fixture construction only,
  never linked into the shipped binary).

## Performance / operability

- `summarize` is one linear pass over every artifact plus a `BTreeMap` insert per artifact for
  project grouping and one `sort_by` over the artifact count for top-contributors - the same
  complexity class as `views`'s own grouping functions (E08-S04).

## Documentation updated

- `docs/architecture/TARGET.md`: new "Machine and project atlas (E09-S02)" subsection.
- `docs/PRODUCT.md`: a short paragraph on the Atlas screen's capability under "Interfaces".
- `CHANGELOG.md` Unreleased/Added.

## Residual risks

- Live-scan wiring into the running `cancellai-tui` binary remains deferred (documented in
  `main.rs` and `TARGET.md`) - the shipped binary's Atlas screen shows "No inventory scan loaded
  yet." until a follow-up story assembles a real `Vec<ProviderResolution>` and feeds it through
  `summarize`. This mirrors E08-S04's identical, already-accepted deferral for the CLI's own
  view wiring, and is explicitly out of this story's fixture-driven verification contract.
  Tracked as follow-up scope, not silently assumed done.
- `AtlasSummary`/screen layout has not been visually exercised on a real terminal beyond the
  automated `TestBackend` renders (S01's own residual about interactive/manual testing in this
  environment applies identically here).

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
