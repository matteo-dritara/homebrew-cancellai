# Safety Verdict - E18-S03

- Change: OSS/commercial protocol boundary - `RemoteExecutionRequest` (E18-S02) named as the open
  local node protocol any coordinator, commercial or self-hosted, produces identically; offline
  conformance evidence that local functionality is unaffected by remote configuration
- Risk: CR4 (declared CR1 at planning; raised by the commit-time risk-floor gate because its own
  test touches `rust/crates/cancellai-safety/src/*`)
- Review target: `2fe937d..4dcab69` (round 3 scope; cumulative with rounds 1-2 back to `96f645e`)
- Independent verifier: Codex
- Verifier: Codex
- Date: 2026-09-19

Brief-Checksum: b1a9bb24614ecf62be2b39170d95ad793cb6ed9fcb967cc3a3e8a7a3c85e061a
Verifier: Codex

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

None of its own beyond E18-S02's: this story's AC2/SI-031 claim ("commercial services add
coordination, not local destructive capabilities") rests entirely on E18-S02's verified-result
type actually being non-bypassable. Round 2 found that type was not (F1); this story could not
honestly claim a closed boundary until E18-S02's own repair closed it, which round 3 independently
confirmed.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-031 (local authority independent of remote configuration) | A local authority decision must not depend on, or be weakened by, presence/absence of remote-controller configuration | `local_authority_is_unaffected_by_an_unconfigured_commercial_service` exercises real data flow: an empty policy refuses a remote request through the public `RemoteExecutionLog::verify_and_record` path, and a local `effective_authority` decision computed alongside it is unaffected | PASS |
| AC1/AC3 (single-machine functionality not reduced without commercial config) | No account/cloud/coordination dependency for core functionality | `cancellai-cli`/`cancellai-tui` `Cargo.toml`s carry no networking dependency; empty-policy refusal does not touch the local authority path | PASS |
| AC2/SI-031 (commercial services add coordination, never a privileged path) | No producer-specific privileged route into local authority | Inherits E18-S02's now-repaired opaque, immutable `VerifiedRemoteIntent` and its action/ceiling checks; static search finds no privileged producer branch anywhere in the workspace | PASS_WITH_RESIDUALS (inherits E18-S02's residuals) |

## Adversarial cases

Inherits every adversarial case from E18-S02's own Safety Verdict (replay, target confusion,
signature binding, wire-vocabulary rejection, forgery/mutation attempts against
`VerifiedRemoteIntent`) since this story's claim is built directly on that primitive. Independently
re-confirmed at round 3: real-data-flow local/remote independence test, no CLI/TUI networking
dependency, no production remote caller anywhere in the workspace.

## Differential / compatibility evidence

Same CI/gate evidence as E18-S02's own Safety Verdict - the exact reviewed commit (`4dcab69`) is
green across the full CI matrix. Documentation-only change to `docs/PRODUCT.md` plus one Rust
test; no dependency, schema, or platform surface touched by this story's own diff.

## Known residual risks

1. Inherits E18-S02's durable replay-state persistence and full-integration residuals in full -
   this story's "open protocol" claim is a documentation/conformance layer over E18-S02's
   primitive, not an independent implementation.
2. The "no networking dependency" claim is verified by direct `Cargo.toml` inspection today, not
   by an automated gate; a future PR adding a networking crate to `cancellai-cli`/`cancellai-tui`
   would not be caught by any check this story adds (accepted at this story's original CR1 scope).
3. Documentation/test-maintenance, non-blocking: this story's own AC table header is separated
   from its rows by an inserted explanatory paragraph, breaking the Markdown table rendering; its
   AC2/residual prose still refers to "CR1" in places despite the CR4 header.

## Rollback / recovery

Documentation-only change plus one test; reverting touches no runtime behavior. `RemoteExecutionRequest`
remains E18-S02's primitive to roll back if ever needed - see that story's own Safety Verdict.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS`

Owner note: Accepted 2026-09-19, alongside the identical decision for E18-S02 (this story's
boundary claim is not independently meaningful without it). Same residuals apply.
