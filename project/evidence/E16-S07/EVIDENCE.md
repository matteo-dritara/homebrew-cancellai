# Evidence Packet - E16-S07

- Commit/PR: the MSRV-compatible rewrite of the bundle verifier's conditions on `main`
- Executor: Claude
- Independent verifier: **required and not yet performed.** This is a CR4 change to the safety
  kernel; `AGENTS.md` and `docs/development/AGENT_PROTOCOL.md` do not allow the executor's own
  review to close one, and the owner's waiver of the Codex round for this session's work does not
  reach a CR4 Safety Verdict. The story stays at `ready_for_review`.
- Change Risk: CR4, the floor `project/risk_floors.json` sets for `rust/crates/cancellai-safety/src/*`
- Spec version/commit: `project/epics/E16.json` at this commit

## Outcome

IMPLEMENTED - awaiting independent verification

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - identical accept/reject behaviour | Three sites, each a mechanical de-sugaring. `if let Some(x) = opt && cond` becomes `opt.is_some_and(\|x\| cond)`; the staleness chain becomes `self.current.as_ref().is_some_and(\|current\| same_publisher && lower_or_equal_sequence)`. `is_some_and` is `false` on `None`, which is what the let-chain's failed pattern match produced, and the guard order inside the closure is the order the `&&` chain had. 101 tests in `cancellai-safety` pass, 2 more than before. | PASS |
| AC2 - the exact boundary rejects | `expiry_a_bundle_past_its_expires_at_is_rejected` already ran with `now == expires_at`, so verification's `>=` was pinned before this change. Rollback's copy of the same comparison was not: `rollback_at_exactly_the_expiry_boundary_refuses` and `rollback_just_before_the_expiry_boundary_succeeds` now pin it, and the first also asserts the store is unchanged after a refused rollback (SI-029). | PASS |
| AC3 - no expiry is not an expiry | `is_some_and` returns `false` for `None`. Exercised throughout: most bundles in these tests carry `None` and verify. | PASS |
| AC4 - a different publisher is not stale | `a_first_bundle_from_a_different_publisher_is_never_stale` applies `acme` at sequence 100, then `other` at sequence 1 over it, and expects success. This is precisely the guard the `&&` chain provided, and it fails if the rewrite loses the publisher check. | PASS |
| AC5 - compiles on the promised toolchain | **Not verified locally.** No 1.85 toolchain is installed here and installing one is not the executor's decision, so the only evidence is the MSRV legs of `.github/workflows/rust.yml` on ubuntu, macOS and Windows at this commit. Recorded as the CI result rather than claimed. | DEFERRED TO CI |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-029 | A failed rollback bricking offline inspection by clearing `current` | `rollback_at_exactly_the_expiry_boundary_refuses` asserts `current` still holds the newer bundle after the refusal, and `rollback_fails_closed_with_no_prior_bundle` covers the other refusal path. | PASS |
| Authority | An expired bundle verifying because the rewrite weakened the comparison | Both directions of the boundary are pinned at both sites: reject at `now == expires_at`, accept at `now == expires_at - 1`. | PASS |
| Authority | A bundle from an untrusted publisher gaining authority through the staleness path | Unchanged by this edit - the staleness check runs *after* `verify_bundle`, which is where signature and publisher trust are decided. The rewrite does not move it. | PASS |

## Verification Commands

```text
cargo fmt --check                                                     -> clean
cargo clippy --workspace --all-targets --all-features -- -D warnings  -> clean
cargo check --workspace --all-targets                                 -> clean
cargo test --workspace                                                -> 0 failed; cancellai-safety 101 passed
cargo deny check                                                      -> advisories ok, bans ok, licenses ok, sources ok
```

## Compatibility

- No behaviour change intended and none observed. No public API, schema or wire-format change.
- The compatibility promise this restores is ADR-0015's MSRV of 1.85.0.

## Performance / operability

- `is_some_and` on a `Copy` option is the same comparison the let-chain performed. Not measured;
  not material.

## Residual risks

- **The MSRV is not verifiable on the executor's machine.** Local `rustc` is 1.94 and no 1.85
  toolchain is installed. Every claim about 1.85 in this packet rests on CI.
- **Nothing prevents the next let-chain.** It is stable, idiomatic 2024-edition Rust, and the only
  thing that catches it is the MSRV leg - which is now working, and which is exactly how this was
  found. A lint would be better and does not exist here.
- **The promise itself may be the wrong one.** The code has in fact required 1.88 since
  2026-09-09. Restoring 1.85 is the conservative reading; deciding that 1.88 is the real minimum
  is an owner decision about a published promise and is deliberately not taken here.
- **This packet has no independent verdict.** It is a CR4 change to the safety kernel and the
  story stays at `ready_for_review` until one exists.

## Safety Verdict

**Not issued.** A CR4 Safety Verdict requires an independent verifier, and this repository's
executor may not write its own. What the executor can state is what was checked and what was not,
which is above.

## Verifier verdict

pending - genuinely pending, unlike the other packets in this session, which record a waiver
