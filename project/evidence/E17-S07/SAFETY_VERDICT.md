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

## Independent review - round 3

Verdict: FAIL
Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

### Safety surface and invariants

| Invariant | Evidence | Result |
| --- | --- | --- |
| SI-022 | Release provenance is now derived inside `ContainmentLedger` from compile-time package/channel metadata; `ingest` and `refresh` accept no caller-supplied provenance. But exported `KnowledgeProvenance` has public fields and `IncidentEvidence` has public fields, so any dependent crate can construct a purported incident record with invented knowledge provenance or alter a cloned record before serializing/reporting it. | FAIL |
| SI-029 | Re-issued records append, and `binding_for` computes the lowest ceiling over all matching records. Narrower scope, looser ceiling, omission, expiry, replay and rollback do not remove an existing record. However, repeated fresh sequences can append 64 records per notice without any lifetime bound; a trusted but compromised publisher can exhaust memory and make offline inspection unavailable. | FAIL |
| SI-030 | `effective_authority_under_containment` adds only the minimum of channel and containment constraints; its result cannot exceed the local channel-constrained kernel result. | PASS |

### Adversarial evidence

- The round-2 scope/ceiling repair holds by inspection of append-only `active.extend` and minimum matching ceiling in `binding_for`; existing tests cover narrower same-publisher reissue across version/action/platform and a second publisher's attempted ceiling relaxation.
- Release provenance repair holds: the caller-supplied parameter is removed from both entry points, fields are private, no public constructor/deserializer exists, and `of_this_build` reads `CARGO_PKG_VERSION` plus `BuildChannel::from_compiled_env`.
- Evidence remains forgeable as a public value: downstream code may construct `KnowledgeProvenance { publisher_id, sequence, issued_at, content_digest }` directly and `IncidentEvidence` directly because all fields are public. Required repair: make provenance/evidence fields private (or expose immutable read accessors), keep construction confined to the verified ingestion path, and ensure any exported evidence representation cannot be mistaken for a ledger-verified record after caller editing. Add compile-fail/API regression coverage.
- Resource bound reproduction: `validate_notice` admits up to `MAX_ENTRIES` (64) per signed notice, but `ingest` appends every successful notice to `active` and has no total record/byte cap. A publisher can sign monotonically increasing sequences with 64 fresh incident IDs on each notice indefinitely. Required repair: place a bounded resource limit on retained records and publisher sequence state; at capacity refuse the new notice atomically and preserve every existing containment (never evict/relax records remotely). Add boundary and repeated-ingestion tests demonstrating refusal preserves authority and inspection availability.
- Duplicate IDs within one notice remain rejected. Across notices and publisher key rotation, records are append-only; sequence tracking per publisher can reset on key change but does not remove prior records. Large sequence gaps do not weaken monotonicity.
- No refusal path mutates the ledger before signature, replay, and notice validation; remote refusal/offline paths do not call `lift_locally`. That method remains the sole explicit local removal path.

### Gate summary

- `cargo fmt --check`: PASS
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS
- `cargo check --workspace --all-targets`: PASS
- `cargo test --workspace`: PASS (workspace tests and doctests; one scheduled heavy benchmark ignored by its test annotation)
- `cargo deny check`: NOT COMPLETED. Initial run could not acquire `/Users/matteo.peo/.cargo/advisory-dbs/db.lock` on the read-only Cargo home. Retry using a writable temporary Cargo home did not finish because it attempted an advisory database refresh; stopped after waiting. No pass is claimed.
- Python test suite: PASS, 688 passed and 633 subtests passed (`python3 -m pytest tests -v`).
- Python tooling and gates: PASS using pinned local Ruff 0.16.5 and mypy 2.3.1 binaries, followed by the complete AGENTS.md Python gate list; final result `gate sensitivity OK: 11 mutants, 11 killed`.
- `gh run list --branch main --limit 5`: UNKNOWN; GitHub API connection unavailable.

### Required repairs

1. Seal caller construction/mutation of `KnowledgeProvenance` and `IncidentEvidence`; only successfully verified ingestion may create ledger-verifiable evidence, and callers must not be able to serialize forged or edited values as authentic incident evidence.
2. Bound cumulative retained containment records and per-publisher sequence tracking. At capacity reject atomically and retain all existing records; never solve resource pressure by remotely evicting or weakening containment.
3. Rerun the complete Rust and Python gates, including a successful `cargo deny check`, and independently review the repairs.

The first two round-2 repairs are confirmed, but E17-S07 does not pass this round. No story status closure is authorized by this verdict.
