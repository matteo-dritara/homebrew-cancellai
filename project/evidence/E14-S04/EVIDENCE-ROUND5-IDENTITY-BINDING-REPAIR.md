# Evidence Packet - E14-S04 (round 5, identity-binding repair)

- Commit/PR: repairs the finding in `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5.md` (`FAIL`)
- Executor: Claude
- Independent verifier: Codex (pending)
- Change Risk: CR4
- Spec version/commit: `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-
  observation-type.md` ("Round 5 self-correction" section)

## Outcome

PARTIAL (repair complete, awaiting independent review - this is the second of at most two review
rounds authorized for this specific repair task; a further FAIL requires a fresh decision on how
to proceed, not a third patch attempt)

## Context

Independent review of the round-5 (ADR-0036) design found `BoundLayoutObservation::observe`'s
public `identity_observer: &dyn IdentityObserver` parameter still forgeable: an external caller
could pass `cancellai_platform::SyntheticIdentityObserver` (itself legitimate public API) to bind
a real, unrelated, favorably-shaped directory's genuinely-observed markers to a fabricated
`IdentityToken` equal to some other, genuinely drifted root's real identity. The resulting
`ProviderExecutionPermit`'s `root_identity()` would equal the real drifted root's identity while
its authority level was computed from the unrelated, safe directory's markers - defeating the
root-binding a future mutation-boundary consumer would rely on.

## Repair

`rust/crates/cancellai-platform/src/provider_layout.rs`: `BoundLayoutObservation::observe` no
longer takes an observer parameter. Identity is now read internally, via
`crate::identity::SystemIdentityObserver`, from the same call that reads the markers
(`std::fs::read_dir(root)`), so there is no longer a parameter through which the two facts could
be supplied from different, caller-chosen sources. A `compile_fail` doctest pins that an external
caller cannot supply an observer at all - the parameter does not exist to pass one to.

All call sites (platform's own tests, `cancellai-safety::authority`'s test helper) updated to the
new one-argument signature; no production call site existed to update (ADR-0036's disclosed
residual: nothing consumes this API in production yet).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| Round-5-review finding closed | `BoundLayoutObservation::observe(root)` has no observer parameter; `compile_fail` doctest (`provider_layout.rs` line ~65) proves no external construction path exists to substitute one | PASS |
| Existing round-5 guarantees preserved | Full workspace test suite re-run after the signature change: 0 failures, including all `e14s04_*` tests and the four `provider_layout::tests::*` | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                                    -> PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings -> PASS
cd rust && cargo test --workspace                                               -> PASS (0 failed, includes the new compile_fail doctest)
python3 scripts/check_rust_workspace.py check                                   -> PASS
python3 scripts/check_mutation_boundary.py check                                -> PASS
python3 scripts/project_os.py check                                             -> PASS
```

## Documentation updated

- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
  ("Round 5 self-correction" section added; the `observe(root, identity_observer)` reference in
  the main decision text corrected to `observe(root)`)
- Module doc in `rust/crates/cancellai-platform/src/provider_layout.rs`

## Method defects

- **What happened**: the first round-5 implementation reasoned that a capability-typed parameter
  (`&dyn IdentityObserver`) was non-forgeable by the same logic that makes `MutationExecutor`/
  other capability parameters elsewhere in this codebase safe - but those capabilities are
  invoked by code that already holds the authority to choose which implementation is wired in
  (production vs. test); here, the *caller of a library function* controlled which
  implementation was passed, which is a different trust boundary. **Prevented by**: no existing
  document states this distinction explicitly as a design rule (a capability parameter is safe
  only when the code selecting the implementation is itself trusted, not merely when the
  parameter is a trait rather than a bare value) - worth adding to `docs/architecture/
  PLATFORM_MODEL.md` if a future story revisits capability-parameter design, but not actioned
  here (out of this repair's scope). **Disposition**: proposed.

## Residual risks

- Unchanged from ADR-0036: `known_signatures` has no trusted source yet, and no mutation-boundary
  call site consumes a permit yet. Both remain accurately disclosed and unaffected by this repair.

## Verifier verdict

PENDING - awaiting independent review by Codex.
