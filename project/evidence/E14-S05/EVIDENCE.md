# Evidence Packet - E14-S05

- Commit/PR: `c7f12bf` (round 1) + `414a9fc` (round 2 repair)
- Executor: Claude
- Independent verifier: Codex - round 1: `project/evidence/E14-S05-VERIFIER-REVIEW.md`, `FAIL`
  (F1/F2/F3 below); round 2 (same file, appended): `PASS_WITH_RESIDUALS`, CR4 Safety Verdict
  recorded, commit `4fe019e`
- Change Risk: CR4
- Spec version/commit: `project/epics/E14.json` (E14-S05), ADR-0037

## Outcome

PASS_WITH_RESIDUALS - independent round 2 verdict, `project/evidence/E14-S05-VERIFIER-REVIEW.md`

## Round 1 independent review: FAIL, three findings, all repaired here

`project/evidence/E14-S05-VERIFIER-REVIEW.md` (Codex, round 1) rejected the first committed
version with three concrete, reproduced findings. All three are repaired in this packet's
commit; none was dismissed.

- **F1 (SI-004, AC2)** - `mutation_executor::execute`/`execute_all` were `pub`, so an external
  Rust crate depending on `cancellai-safety` as a library could call `execute` directly with a
  fabricated `SyntheticProviderLayoutObserver` alongside the real `SystemMutationExecutor`,
  reaching a genuine, unconfirmed deletion `scripts/check_mutation_boundary.py` cannot see (it
  only scans this repository's own sources). Reproduced with a compiled external consumer.
  *Repair*: `execute`/`execute_all` are now `pub(crate)`; removed from `lib.rs`'s public
  re-exports. `execute_with_system_capabilities` is the only production entry point this crate
  exposes at all, and it hardcodes every capability. Verified: a standalone reproduction crate
  built against the repaired tree now fails to compile with `error[E0603]: function 'execute'
  is private` (not committed; verification-only, see Verification Commands).
- **F2 (SI-004, SI-013, AC1)** - the fresh layout observation was not held through the actual
  `executor.mutate` call; a concurrent write landing between the read and the mutation was not
  caught. Reproduced with an observer that plants a marker synchronously after reading.
  *Repair (narrowed, not closed - disclosed residual)*: the check now runs as the literal last
  statement before `executor.mutate`, removing operation-building time from the window. Fully
  closing it needs a handle-retained read through `cancellai-platform::mutation` itself
  (mirroring E21-S07's single-file identity confirmation) - scoped out as future work, the same
  way E21-S07 itself followed E03-S05's initial narrower version as a separate story.
- **F3 (SI-004, SI-013, AC1/AC3)** - `seal_restore` recorded its layout snapshot against `root`
  (the quarantine store), never against the real destination provider root the move actually
  writes into - Restore plans had no real SI-004 protection at all. Reproduced natively: an
  unchanged quarantine source with a drifted destination still succeeded.
  *Repair*: `MoveDestination` gained `root_path()`; `seal_restore` observes
  `destination.root_path()` instead of `root.path()`. `SealedPlan.provider_layout` is now a
  `ProviderLayoutSnapshot` (`root_path`/`root_identity`/`LayoutSignature`) bound explicitly per
  action class, and `revalidate_provider_layout` re-observes exactly the path the snapshot
  itself recorded - never `target`'s own bound root.

ADR-0037's "Round 2" section carries the same account in full, alongside the design rationale.

## Acceptance Criteria Evidence

| AC | Text | Evidence | Result |
| --- | --- | --- | --- |
| AC1 | "A destructive mutation whose SealedPlan's authority was computed while a provider layout was recognized, but which has since drifted, is refused at execute()/revalidate() time by a fresh observation obtained then - not merely permitted because the plan's own stale data still says recognized." | `revalidate_provider_layout_blocks_when_the_fresh_signature_has_drifted`, `revalidate_provider_layout_ignores_marker_order_and_duplicates` (`sealed_plan.rs`); `execute_never_calls_mutate_on_a_provider_layout_that_drifted_since_seal` (synthetic, proves `mutate()` is never reached), `execute_refuses_a_real_delete_when_the_provider_root_gained_a_marker_between_seal_and_execute` (real filesystem, end to end), `seal_restore_binds_its_provider_layout_to_the_destination_root_not_the_quarantine_source` (F3 regression) (`mutation_executor.rs`/`sealed_plan.rs`) | PASS (F2's narrower, disclosed residual: the read-to-mutate window itself is narrowed, not eliminated - see Residual risks) |
| AC2 | "No caller can reach a destructive authority level for a real mutation by constructing a separate AuthorityInputs/SealedPlan that omits or contradicts the layout observation cancellai-platform's own observer capability would produce for the same provider root at the same time - the exact counterexample E14-S04 rounds 3 and 4 demonstrated must not reproduce against execute() itself." | `SealedPlan::seal*` derive `provider_layout` internally from a real `ProviderLayoutObserver` observation - no parameter accepts a pre-built value. `execute`/`execute_all` are `pub(crate)` (F1 repair): `execute_with_system_capabilities` is the only externally reachable mutation path and hardcodes every capability, including the layout observer. `execute_refuses_a_real_delete_when_the_provider_root_itself_was_swapped` reproduces a real root-identity swap isolated from the pre-existing artifact-identity check (hard-linked inode). External-crate F1 reproduction now fails to compile (`error[E0603]`) | PASS |
| AC3 | "If the provider root cannot be observed (no filesystem access, an unprobed provider, an I/O error), the result fails closed to the same ceiling a drifted layout would, never to an unconstrained default." | `revalidate_provider_layout_blocks_when_the_fresh_observation_itself_fails`, `revalidate_provider_layout_blocks_when_the_plan_never_recorded_a_baseline` (`sealed_plan.rs`) - both return `StalePlan`. F3 repair also closes AC3's silent gap for Restore (no root was observed there at all before this repair) | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 | Real provider root gains a marker between seal and (start of) execute; a fabricated external observation; a Restore whose real destination drifted while the quarantine source did not | `execute_refuses_a_real_delete_when_the_provider_root_gained_a_marker_between_seal_and_execute`; external F1 reproduction now refuses to compile; `seal_restore_binds_its_provider_layout_to_the_destination_root_not_the_quarantine_source` | PASS |
| SI-013 | Provider root identity swapped (hard-linked artifact preserves artifact identity, isolating the new check); read-to-mutate TOCTOU window (F2) | `execute_refuses_a_real_delete_when_the_provider_root_itself_was_swapped`, `revalidate_provider_layout_blocks_when_the_root_identity_itself_changed` | PASS, with F2's narrowed-not-closed window disclosed as residual |
| SI-004/SI-013: unknown-to-authority promotion | Unobservable root, no seal-time baseline, external fabricated observer | `revalidate_provider_layout_blocks_when_the_fresh_observation_itself_fails`, `revalidate_provider_layout_blocks_when_the_plan_never_recorded_a_baseline`, F1 compile-time closure | PASS |
| SI-019 (one safety boundary) | `scripts/check_mutation_boundary.py` re-run after F1's visibility repair | `mutation boundary OK: 96 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs deletes anything, only rust/crates/cancellai-platform/src/mutation.rs, rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does` | PASS |

## Verification Commands

```text
cd rust
cargo fmt --check                                                                    -> pass
cargo clippy --workspace --all-targets --all-features -- -D warnings                 -> pass
cargo clippy -p cancellai-platform -p cancellai-safety --all-targets --all-features \
  --target x86_64-pc-windows-gnu -- -D warnings                                      -> pass
cargo clippy -p cancellai-cli --all-targets --all-features \
  --target x86_64-pc-windows-gnu -- -D warnings                                      -> UNAVAILABLE
                                                                                          (no x86_64-w64-mingw32-gcc installed locally - pre-existing
                                                                                          local gap, confirmed unrelated to this change; CI's real
                                                                                          windows-latest runner is unaffected.)
cargo clippy -p cancellai-platform -p cancellai-safety --all-targets --all-features \
  --target x86_64-unknown-linux-gnu -- -D warnings                                   -> pass
cargo check --workspace --all-targets                                                -> pass
cargo test --workspace                                                               -> pass (every crate "0 failed"; cancellai-safety 165 passed)
cargo deny check                                                                     -> pass (advisories ok, bans ok, licenses ok, sources ok)

# F1 verification: a standalone external crate depending on cancellai-safety/cancellai-platform
# as path dependencies, attempting `cancellai_safety::mutation_executor::execute` directly.
# Not committed to this repository - built once in a scratch directory to confirm the fix.
cargo build --offline --quiet (in the scratch reproducer)                            -> fails to compile:
                                                                                          error[E0603]: function `execute` is private

cd ..
python3 scripts/project_os.py check                                                  -> pass
python3 scripts/check_docs.py check                                                  -> pass
python3 scripts/gen_docs.py --check                                                  -> pass
python3 scripts/check_mutation_boundary.py check                                     -> pass
python3 scripts/check_platforms.py check                                             -> pass (docs/PLATFORMS.md regenerated, round 1)
python3 scripts/check_ears.py check                                                  -> pass
python3 scripts/check_evidence.py check                                              -> pass
python3 scripts/check_process.py check                                               -> pass
python3 scripts/check_risk_classification.py check                                   -> pass
python3 scripts/release.py check                                                     -> pass
pre-commit run --all-files                                                           -> pass (32/32 hooks)
```

## Compatibility

Unchanged from round 1: Windows/non-Unix Delete remains refused via the same disclosed,
fail-closed consequence (`BoundLayoutObservation::observe` is Unix-only). Round 2's repairs
change nothing about that - `check_platforms.py`'s Windows entry note and ADR-0037 already
cover it.

## Performance / operability

Unchanged from round 1: one additional real directory read per destructive mutation, now
positioned immediately before the mutation call itself rather than before operation-building
(F2's narrowing) - marginally later, not materially more expensive.

## Documentation updated

- `docs/adrs/0037-...md` - new "Round 2: independent review (Codex) found three real defects"
  section (F1/F2/F3, repairs, and what remains residual); "Decision" section corrected to match
  the repaired design (`ProviderLayoutSnapshot`, per-action-class root binding, check placement);
  "Neutral / follow-up" gained F2's own named residual.
- `docs/architecture/DOMAIN_MODEL.md` - "SealedPlan" section corrected for `ProviderLayoutSnapshot`
  and per-action-class root binding.
- `docs/architecture/GUARDIAN_MODEL.md` - notes round 2's three findings and repairs.
- `docs/architecture/PLATFORM_MODEL.md` - notes the `pub(crate)` visibility repair (F1).
- `CHANGELOG.md` - `[Unreleased]` "Security" entry extended with the round-2 account.

## Method defects

- **What happened**: my own pre-submission verification (the `adversarial-cases` pass before round 1) did not probe "can a different Rust crate call this function directly with a fabricated capability" (F1), did not verify the observation is held through the actual mutation syscall rather than merely read beforehand (F2), and did not cross-check that `seal_restore`'s `root` parameter - already documented in its own doc comment as "the quarantine store... not the original provider root" - was actually the root my own new code observed (F3, a plain reading-comprehension gap against my own prior sentence). **Prevented by**: the `adversarial-cases` skill's eleven axes do not include "is this capability's visibility actually restricted to its intended caller" or "does the exact root named in this function's own preceding doc comment match the root the new code touches" as explicit prompts - both are general enough to belong there, not specific to this story. **Disposition**: proposed 2026-09-22 - add both as explicit checks to the `adversarial-cases` skill (a "second-path check" style question for capability visibility, and a "does new code match the plan/doc comment's own stated referent" self-consistency check).
- none otherwise - independent review round 1 caught these before merge, working as the process is designed to.

## Residual risks

- **F2 (SI-013): the fresh layout observation is not held through the mutation itself.**
  Narrowed (the check is now the last statement before `executor.mutate`) but not closed - a
  real concurrent drift landing in the remaining window (between this read and the OS mutation
  syscall) is not caught. Closing it fully needs a handle-retained provider-layout read inside
  `cancellai-platform::mutation`, mirroring E21-S07's single-file identity confirmation -
  future, separately-scoped work (ADR-0037 "Neutral / follow-up").
- **Windows (and every other non-Unix platform) cannot perform any real destructive mutation
  through `cancellai-cli clean`** until a verified `cancellai-sealedfs` handle-bound
  `SealedRoot::metadata`/`list_child_names` implementation exists for it. Unchanged from round 1.
- **ADR-0036's residual (1) remains fully open**: `known_signatures` still has no trusted source.
  Unchanged from round 1.
- The coverage ratchet (`project/coverage_baseline.json`) remains deliberately unrecorded - see
  round 1's note (unrelated-crate drift, not attributable to this story).

## Verifier verdict

`PASS_WITH_RESIDUALS` - Codex, round 2, `project/evidence/E14-S05-VERIFIER-REVIEW.md` (commit
`4fe019e`). Independently reproduced F1 (external `execute` call fails `E0603`; a system-wrapper
fabricated-seal attempt is safely blocked) and F3 closed (native Restore-destination-drift
reproduction refused, quarantine source retained). F2 confirmed narrowed-not-closed and accepted
as a disclosed residual, on the same precedent this repository already applies to single-file
identity before E21-S07. CR4 Safety Verdict recorded in the same file: SI-004/SI-013 both
`PASS_WITH_RESIDUALS`. Verifier recommendation: `ACCEPT_WITH_RECORDED_RESIDUALS`; owner
acceptance obtained via this session's explicit direction to close the story on this verdict.
