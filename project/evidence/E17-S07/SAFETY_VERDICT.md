# Safety Verdict - E17-S07

- Change: Signed incident containment and authority downgrade
- Risk: CR4
- Commit/PR: `ae927b3` (implementation); review recorded on `review/e17-round2`
- Independent verifier: Codex
- Date: 2026-09-23
Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

## Verdict

`FAIL`

## Safety surface changed

A verified knowledge bundle can add provider/version/action/platform containment to a local ledger, and the authority calculation can cap the result at Observe or Recommend. The ledger is private-state, but its remote re-issue merge behavior is not monotonic across scope, and the evidence API accepts caller-supplied release identity.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Knowledge remains data, signatures/provenance are verified, and incident evidence accurately records release provenance. | Signature/digest/publisher refusal cases pass in the implementation suite. However, `ingest` accepts a public `ReleaseProvenance` value and records arbitrary caller-supplied version/channel. | FAIL |
| SI-029 | Replays, rollback, expiry, and remote updates cannot remove active containment; only local lift does. | Verifier regression: broad same-ID scope from `acme` is overwritten by a narrower same-ID scope from trusted `backup`; version `2.0.0` loses its binding without local lift. | FAIL |
| SI-030 | Release channel constrains effective authority. | The containment authority function combines base constraints and compiled-channel ceiling by minimum; existing authority tests cover channels and both permitted containment ceilings. | PASS for reviewed helper |

## Adversarial cases

- Both `observe` and `recommend` ceilings are structurally the only enum values; existing tests show contained authority is no higher than the kernel and remains below quarantine mutation threshold.
- Existing tests cover forged/invalid signature, unknown publisher, digest mismatch, malformed notice, expiry, replay, omitted incident, knowledge store rollback, offline service, and failed-update replay-counter behavior.
- Existing tests reject empty scope lists, empty/malformed/oversized identifiers, oversized list counts, duplicate IDs within a notice, schema drift and malformed envelopes. No distinct byte-size cap is present on raw bundle/payload input; bounded entry/list counts do not bound a single input's byte size.
- Scope matching is exact and case-sensitive for provider/version IDs; unknown installed version conservatively matches a version-scoped incident. Platform and action-class filters are exact typed enum matches. Empty optional selector lists are rejected.
- The new verifier regression fails: second trusted publisher reissues `INC-1` at the same `Observe` ceiling with a narrowed version list, and the old affected version becomes uncontained. Equal-ceiling same-publisher re-issue follows the same replacement branch.
- `ContainmentLedger` active and replay maps are private and external callers cannot construct a populated ledger directly. `IncidentEvidence` is publicly constructible but cannot be inserted into that private ledger. The material bypass is `ingest` accepting fabricated public `ReleaseProvenance`, which can corrupt returned audit evidence but does not itself change authority.
- Signed notice identifiers are bounded and no payload field is copied into evidence. Content is represented by a SHA-256 digest. Caller-supplied release provenance is a separate integrity flaw described above.
- Offline refresh preserves an existing ledger and leaves an empty ledger empty; no service outage changes authority by itself.

## Differential / compatibility evidence

No Python/Rust behavior comparison applies to this Rust safety API. The authority helper composes its constraints with the same minimum calculation. A live production caller is not yet present; cutover wiring is disclosed as E06-S04 work.

## Known residual risks

- Exact required repairs are listed in `project/evidence/E17-VERIFIER-REVIEW-ROUND2.md`: monotonic merge/rejection of remote re-issues across scope and publishers; internally derived opaque release provenance for evidence.
- The mechanism is not yet wired to a live mutation path or shipped distribution channel, as documented for E06-S04.
- The network-facing text path does not enforce a maximum byte size before parsing/verifying.

## Rollback / recovery

Do not close E17-S07 or rely on this mechanism for cutover until the two required repairs pass independent adversarial review. Existing local ledger state can be cleared only via local lift; preserve the ledger across update failure and rollback.

## Owner decision

`REJECT`

Owner note: Verifier recommendation is rejection pending the exact repairs above; owner acceptance remains required for CR4 closure.
