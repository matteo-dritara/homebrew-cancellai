# Evidence Packet - E14-S05

- Commit/PR: committed on `main`, parent `24d0dd11b270d19476bf27b5a2152d847dd806b8`
- Executor: Claude
- Independent verifier: Codex (pending)
- Change Risk: CR4
- Spec version/commit: `project/epics/E14.json` (E14-S05), ADR-0037

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Text | Evidence | Result |
| --- | --- | --- | --- |
| AC1 | "A destructive mutation whose SealedPlan's authority was computed while a provider layout was recognized, but which has since drifted, is refused at execute()/revalidate() time by a fresh observation obtained then - not merely permitted because the plan's own stale data still says recognized." | `revalidate_provider_layout_blocks_when_the_fresh_signature_has_drifted`, `revalidate_provider_layout_ignores_marker_order_and_duplicates` (`sealed_plan.rs`); `execute_never_calls_mutate_on_a_provider_layout_that_drifted_since_seal` (synthetic, proves `mutate()` is never reached), `execute_refuses_a_real_delete_when_the_provider_root_gained_a_marker_between_seal_and_execute` (real filesystem, end to end) (`mutation_executor.rs`) | PASS |
| AC2 | "No caller can reach a destructive authority level for a real mutation by constructing a separate AuthorityInputs/SealedPlan that omits or contradicts the layout observation cancellai-platform's own observer capability would produce for the same provider root at the same time - the exact counterexample E14-S04 rounds 3 and 4 demonstrated must not reproduce against execute() itself." | `SealedPlan::seal`/`seal_with_process_guard`/`seal_quarantine`/`seal_restore`/`seal_archive` derive `provider_layout` internally from a real `ProviderLayoutObserver` observation of `root.path()` - there is no parameter through which a caller supplies a pre-built signature (mirrors how `root_identity`/`artifact_identity` are already derived, not accepted). `seal_derives_root_and_artifact_identity_from_real_capabilities` extended to assert the recorded signature (`sealed_plan.rs`). `execute_with_system_capabilities` hardcodes `SystemProviderLayoutObserver` - no production caller can inject a synthetic observation (`mutation_executor.rs` doc comment + `check_mutation_boundary.py`, unaffected surface). `execute_refuses_a_real_delete_when_the_provider_root_itself_was_swapped` reproduces a real root-identity swap (hard-linking the original artifact's inode into a freshly recreated root at the same path, isolating the refusal to the new root-identity/layout check rather than the pre-existing artifact-identity check) | PASS |
| AC3 | "If the provider root cannot be observed (no filesystem access, an unprobed provider, an I/O error), the result fails closed to the same ceiling a drifted layout would, never to an unconstrained default." | `revalidate_provider_layout_blocks_when_the_fresh_observation_itself_fails` (fresh observation `Err`), `revalidate_provider_layout_blocks_when_the_plan_never_recorded_a_baseline` (seal-time `None`, e.g. non-Unix) (`sealed_plan.rs`) - both return `StalePlan`, never `Proceed` | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 (unknown/drifted provider layout reduces capability) | A plan sealed under one real layout, executed after the real provider root gained a marker | `execute_refuses_a_real_delete_when_the_provider_root_gained_a_marker_between_seal_and_execute` | PASS |
| SI-013 (identity revalidated immediately before mutation, now extended to the provider root itself) | The provider root's own identity changes between seal and execute while the target artifact's own identity is held constant (hard-linked), isolating the new root-freshness check from the pre-existing artifact-identity check | `execute_refuses_a_real_delete_when_the_provider_root_itself_was_swapped`, `revalidate_provider_layout_blocks_when_the_root_identity_itself_changed` | PASS |
| SI-004/SI-013 combined: unknown-to-authority promotion | Neither an unobservable root nor a plan with no seal-time baseline is ever treated as "no constraint" | `revalidate_provider_layout_blocks_when_the_fresh_observation_itself_fails`, `revalidate_provider_layout_blocks_when_the_plan_never_recorded_a_baseline` | PASS |
| SI-019 (one safety boundary; no second mutation path) | `scripts/check_mutation_boundary.py` re-run after this change | `mutation boundary OK: 96 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs deletes anything, only rust/crates/cancellai-platform/src/mutation.rs, rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does` | PASS |

## Verification Commands

```text
cd rust
cargo fmt --check                                                                    -> pass
cargo clippy --workspace --all-targets --all-features -- -D warnings                 -> pass
cargo clippy -p cancellai-platform --all-targets --all-features \
  --target x86_64-pc-windows-gnu -- -D warnings                                      -> pass
cargo clippy -p cancellai-safety --all-targets --all-features \
  --target x86_64-pc-windows-gnu -- -D warnings                                      -> pass
cargo clippy -p cancellai-cli --all-targets --all-features \
  --target x86_64-pc-windows-gnu -- -D warnings                                      -> UNAVAILABLE
                                                                                          (no x86_64-w64-mingw32-gcc installed locally; cancellai-cli
                                                                                          pulls cancellai-store's bundled-sqlite C build, which needs a
                                                                                          real cross C compiler to link for any cross target, Linux
                                                                                          included - pre-existing local gap unrelated to this change,
                                                                                          confirmed by the identical failure with zero code changes
                                                                                          checked out. CI's real windows-latest runner is unaffected.)
cargo clippy -p cancellai-platform -p cancellai-safety --all-targets --all-features \
  --target x86_64-unknown-linux-gnu -- -D warnings                                   -> pass
cargo check --workspace --all-targets                                                -> pass
cargo test --workspace                                                               -> pass (every crate "0 failed"; cancellai-safety 164 passed,
                                                                                          up from 155 before this story)
cargo deny check                                                                     -> pass (advisories ok, bans ok, licenses ok, sources ok)

cd ..
python3 scripts/project_os.py check                                                  -> pass
python3 scripts/check_docs.py check                                                  -> pass
python3 scripts/gen_docs.py --check                                                  -> pass
python3 scripts/check_schemas.py check                                               -> pass
python3 scripts/check_fixtures.py check                                              -> pass
python3 scripts/check_workflows.py check                                             -> pass
python3 scripts/check_rust_workspace.py check                                        -> pass
python3 scripts/check_mutation_boundary.py check                                     -> pass
python3 scripts/check_provider_compatibility.py check                                -> pass
python3 scripts/check_provider_trust.py check                                        -> pass
python3 scripts/check_platforms.py check                                             -> pass (docs/PLATFORMS.md regenerated)
python3 scripts/check_repository_topology.py check                                   -> pass
python3 scripts/check_agent_skills.py check                                          -> pass
python3 scripts/check_risk_classification.py check                                   -> pass (no new story below its floor)
python3 scripts/check_skill_content.py check                                         -> pass
python3 scripts/check_ears.py check                                                  -> pass
python3 scripts/check_evidence.py check                                              -> pass
python3 scripts/characterize.py check                                                -> pass
python3 scripts/diff_harness.py check                                                -> pass
python3 scripts/rust_python_parity.py self-test                                      -> pass
python3 scripts/rust_python_parity.py check                                          -> pass
python3 scripts/check_process.py check                                               -> pass
python3 scripts/release.py check                                                     -> pass
python3 scripts/release_manifest.py check                                            -> pass
python3 scripts/process_metrics.py check                                             -> pass
python3 scripts/check_agent_toolchain.py check                                       -> pass (no unmanaged component)
python3 scripts/verifier_handoff.py check                                            -> pass
python3 scripts/safety_oracle.py check                                               -> pass
python3 scripts/gate_sensitivity.py check                                            -> pass
python3 scripts/check_coverage.py report                                             -> ran; cancellai-safety 98.52% (was 97.78%), cancellai-platform
                                                                                          95.40% (was 94.39%) - both above their recorded floor. NOT
                                                                                          re-recorded: the same report also raised every OTHER crate's
                                                                                          number, including several this story never touched
                                                                                          (cancellai-guardian 0.0% -> 95.55%, a new cancellai-store row) -
                                                                                          toolchain/environment drift the gate's own docs warn about
                                                                                          ("the same unchanged workspace measures cancellai-platform at
                                                                                          95.83% on stable and 63.85% on nightly"), not an improvement
                                                                                          this story should claim credit for. Reverted the record so this
                                                                                          diff carries only what this story actually changed.
pre-commit run --all-files                                                           -> pass (32/32 hooks)
```

## Compatibility

- Platforms exercised: macOS (this session, real filesystem end-to-end via `TempDir` fixtures),
  Linux (`cargo test`/`cargo clippy` cross-target for the touched crates), Windows (clippy
  cross-target for `cancellai-platform`/`cancellai-safety`; `cancellai-cli` could not be
  cross-compiled locally - see UNAVAILABLE note above, CI covers it).
- `cancellai_platform::provider_layout::BoundLayoutObservation::observe` (the primitive this
  story wires in) only succeeds on Unix (E14-S04/ADR-0036's own disclosed residual, unchanged by
  this story). **This story's wiring therefore makes every destructive mutation through
  `mutation_executor::execute` refuse on non-Unix platforms, Windows included** - a real,
  verified Windows Delete pipeline (E20-S01/E20-S05) is narrowed by this change. Disclosed in
  ADR-0037, `docs/architecture/PLATFORM_MODEL.md`, `project/platforms.json`'s Windows entry
  notes, and `CHANGELOG.md`. No Rust source change is `#[cfg]`-gated for this - the refusal
  follows structurally from `ProviderLayoutObserver::observe` returning `Err` on that platform,
  the same fail-closed pattern the rest of this codebase already uses, so no separate Windows CI
  run was needed to predict it; a future story bringing a verified Windows
  `SealedRoot::metadata`/`list_child_names` implementation restores the capability.
- No schema/JSON-contract changes; `SealedPlan`'s `serde::Serialize` output gains one new
  optional field (`provider_layout`), backward-compatible for any consumer that ignores unknown
  fields (none currently parses this struct's serialized form outside this crate's own tests).

## Performance / operability

- One additional real directory read (`ProviderLayoutObserver::observe`, itself one `openat`
  walk plus one `readdir` pass) per destructive mutation, immediately before it - the same order
  of cost the existing identity revalidation and process-guard checks already pay at the same
  point. No new persistent state, no new background work.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md` - "SealedPlan" section: new `provider_layout` field and
  `revalidate_provider_layout` behavior.
- `docs/architecture/GUARDIAN_MODEL.md` - extends the ADR-0036 residuals paragraph with how
  E14-S05/ADR-0037 closes residual (2) narrowly, and what remains open.
- `docs/architecture/PLATFORM_MODEL.md` - new "Provider-root layout observation" section
  documenting the `ProviderLayoutObserver` seam and its disclosed Windows consequence.
- `docs/adrs/0037-provider-layout-is-revalidated-fresh-immediately-before-mutation.md` - new ADR.
- `project/platforms.json` (Windows entry notes) / `docs/PLATFORMS.md` (regenerated) - discloses
  the narrowing against the specific evidence that entry cites, without changing any `state`
  field (the cited platform-level tests are unaffected and still pass on Windows CI).
- `CHANGELOG.md` - `[Unreleased]` "Security" entry.

## Method defects

- none

## Residual risks

- **Windows (and every other non-Unix platform) cannot perform any real destructive mutation
  through `cancellai-cli clean` until a verified `cancellai-sealedfs` handle-bound
  `SealedRoot::metadata`/`list_child_names` implementation exists for it.** This is the
  deliberate, disclosed cost of closing ADR-0036's residual (2) with a fail-closed freshness
  check rather than leaving the mutation boundary able to proceed without any live observation.
  Tracked as future work in ADR-0037's "Neutral / follow-up" and PLATFORM_MODEL.md; not silently
  deferred.
- **ADR-0036's residual (1) remains fully open**: `known_signatures` still has no trusted source,
  so this mechanism only proves "the layout has not changed since the plan was sealed," never
  "the layout is a recognized-good one." A future story wiring a trusted provider-layout-manifest
  source into `resolve_provider_execution_authority` is the natural next step and is out of this
  story's scope.
- **`revalidate_provider_layout`'s root-identity comparison applies uniformly to every action
  class**, including `Restore`/`Archive` plans sealed against the quarantine/archive store's own
  root rather than a real provider root. This is harmless (those stores are not concurrently
  written by anything else in normal operation) but is a slightly imprecise fit conceptually -
  noted, not fixed, since narrowing it by action class would be unrequested scope for this story.
- The coverage ratchet (`project/coverage_baseline.json`) was deliberately left unrecorded despite
  every ratcheted crate improving - see the Verification Commands note above.

## Verifier verdict

(pending independent review)
