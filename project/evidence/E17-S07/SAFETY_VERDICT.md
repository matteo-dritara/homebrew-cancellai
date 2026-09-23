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

## Round 4 - 2026-09-23

Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

### Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Only verified ingestion creates immutable incident evidence; release identity is derived internally. | Private `KnowledgeProvenance` and `IncidentEvidence` fields; compile-fail doctests cover construction and mutation. `ReleaseProvenance` remains internally derived. | PASS |
| SI-029 | Refusals, replay, rollback and capacity pressure cannot remove or weaken recorded containment; bounded state does not let redundant notices exhaust capacity. | At-capacity refusal is atomic and leaves the refused sequence available; existing records stay. However `same_containment` compares scope vectors order-sensitively although matching treats them as sets, allowing permutations/duplicate list values to consume capacity. | FAIL |
| SI-030 | Incident containment can only reduce channel-constrained authority. | `effective_authority_under_containment` adds only the matched Observe/Recommend ceiling before the common minimum calculation; no authority elevation counterexample found. | PASS |

### Adversarial cases

- The round-3 evidence sealing and cumulative bound changes are present. `cargo test -p cancellai-safety` passes 200 unit tests and seven doctests, including the compile-fail API guards.
- Capacity boundary logic uses `active.len().saturating_add(fresh.len()) > max_records`; equality fits, overflow refuses before sequence insertion, and the publisher bound likewise refuses before state mutation. Tests confirm a refused sequence can be retried with a smaller notice.
- Exact same-order/same-value re-issues deduplicate. But `Option<Vec<_>>` equality in `same_containment` differs for reordered or duplicated list elements, while `applies_to` uses membership semantics. A signed re-issue with the same versions/actions/platforms in a different order therefore consumes a slot without changing any authority binding. Required repair: normalize the three scope lists as sets before comparison/storage and test permutations plus duplicate members at capacity.
- Deduplication also ignores severity, invariant references, affected releases and knowledge provenance. A re-issue with the same containment but new affected-release/invariant evidence is returned by `ingest` but not retained by `active()`. Required repair: preserve newly verified evidence through a bounded monotonic merge, or distinguish bounded audit evidence from containment identity; add a regression asserting the added references remain inspectable.
- Publisher sequence tracking is keyed by publisher ID. A same-ID key rotation retains the previous sequence; adding a distinct publisher at capacity refuses rather than evicting tracking state. Previously tracked publishers can still submit updates.
- `effective_authority_under_containment` adds channel and containment ceilings to the same minimum-based computation. Empty, unmatched, Recommend and Observe cases preserve the expected bounds; no counterexample found.

### Gates

- Rust: `cargo fmt --check`, workspace Clippy, and workspace check PASS; `cargo deny check` PASS (with existing duplicate crate and unmatched allowance warnings). `cargo test --workspace` FAILS at five desktop API integration tests because this sandbox denies local socket creation (`Operation not permitted`). The focused safety package passes all 200 unit tests and seven doctests.
- Python: pytest 689 passed / 637 subtests passed; Ruff check/format, mypy, and the full AGENTS.md Python gate list PASS. Ruff and mypy were run from the repository venv because the active interpreter lacks their modules.
- `gh run list --branch main --limit 5`: UNKNOWN; API connection unavailable.

### Owner decision

`PENDING`

Owner note: This verifier round found unresolved issues and does not accept the CR4 change. This was the owner-authorized final E17 review round; no round 5 is authorized.

FAIL

## Round 5 - 2026-09-23

Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

### Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Knowledge remains inert and verified; incident evidence identifies its signed knowledge and running-release provenance without provider payload content. | Provenance/evidence fields remain private with compile-fail constructor/mutation doctests; release provenance is derived internally. Validation accepts only bounded identifier alphabets and invariant IDs. | PASS |
| SI-029 | Replays, rollback, expiry, refusal, and capacity pressure cannot remove or weaken containment; offline local-kernel operation remains available. | Append-only active records; only `lift_locally` removes records. Canonical scopes and evidence-aware dedup fixes confirmed; capacity checks precede sequence insertion and append. Existing tests exercise refusal atomicity and retry. Raw bundle bytes remain unbounded before parse (residual). | PASS_WITH_RESIDUALS |
| SI-030 | Containment cannot raise authority above local and compiled-channel constraints. | `effective_authority_under_containment` adds only the matched Observe/Recommend ceiling into the common minimum calculation; tests cover channel and containment combinations. No elevation counterexample found. | PASS |

### Adversarial cases

- Round-4 scope repair holds: versions, action classes, and platforms are sorted/deduplicated as sets before retention; the regression permutes and repeats each list, confirms no extra record is retained, then confirms a novel record fits at capacity.
- Round-4 evidence repair holds: severity, invariants, and affected releases participate in dedup identity. Tests prove each changed evidence field remains inspectable as its own active record.
- A semantically identical reissue with newer knowledge sequence/publisher does not consume another retained-record slot. Each successful `ingest` returns evidence containing that verified provenance; active ledger storage remains bounded by semantic containment/evidence identity.
- At-capacity updates refuse whole, keep existing bindings, and do not consume the publisher sequence; a smaller retry at the same sequence succeeds. Publisher tracking is separately bounded and does not evict prior sequence state.
- Same-ID narrower scopes and looser ceilings append alongside prior records; matching remains the union of records and the strictest ceiling. Remote omission, expiry, rollback, replay and failed verification cannot lift active records.
- Signature, publisher, schema, kind, identifier, invariant, duplicate-ID, expiry and malformed-payload checks fail closed. Evidence contains identifiers/digests, not provider payload content.
- Offline refresh does not alter the ledger. Exact matching treats an unknown installed version as affected by a version-scoped notice.
- Raw bundle text has no maximum byte-size validation before parsing/verifying. A caller that accepts arbitrarily large network responses can expose the process to memory/CPU exhaustion; this is retained as an explicit availability residual.
- No filesystem path, link, mount, or reparse-point operation is represented in or performed by the containment ledger. No async shared mutation path is present; Rust's exclusive mutable borrow serializes each ledger mutation.

### Differential / compatibility evidence

No Python/Rust behavior comparison applies to this Rust safety API. Full workspace Rust tests and doctests passed. The helper is not yet wired into a live mutation path; cutover wiring remains E06-S04 as disclosed in the story and runbook.

### Gates

- `cargo fmt --check`: PASS
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS
- `cargo check --workspace --all-targets`: PASS
- `cargo test --workspace`: PASS; workspace tests and doctests completed without failures.
- `cargo deny check`: PASS on retry with writable temporary `CARGO_HOME`; advisories, bans, licenses and sources OK, with existing duplicate crate and unmatched allowance warnings.
- `python3 -m pytest tests`: PASS; 689 passed.
- `python3 scripts/project_os.py check`: PASS before final evidence/status generation; rerun after generation.
- `python3 scripts/verifier_handoff.py check`: PASS before final evidence/status generation; rerun after generation.
- `python3 scripts/check_process.py check`: PASS before final evidence/status generation, with existing historical over-ceiling warnings; rerun after adding this round.

### Required repairs

None for this round. Round-4's required repairs are confirmed.

### Known residual risks

- The raw network-facing bundle text has no pre-parse byte-size cap. Add a bounded fetch/parser contract and adversarial oversized-input coverage before exposing this path to an untrusted network response.
- The containment helper is not yet wired to a live mutation path or shipped distribution channel; this remains E06-S04 cutover work.

### Owner decision

`PENDING`

Owner note: Round 5 confirms the two authorized repairs. This verifier records `PASS_WITH_RESIDUALS`; CR4 closure and acceptance of residual risk remain an owner decision.

PASS_WITH_RESIDUALS

## Owner decision - Round 5

`ACCEPTED`

Owner note (2026-09-23, recorded by the executor at the owner's direction): the round-5
`PASS_WITH_RESIDUALS` verdict is accepted and E17-S07 may close. Both residuals are assigned
rather than accepted as permanent: both move to E06-S04's cutover work, where the story that
wires the ledger into the CLI reads notices from local files and must cap their bytes before
parsing.
