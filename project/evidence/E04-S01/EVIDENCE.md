# Evidence Packet - E04-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E04)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-inventory/src/file_facts.rs`,
  `rust/crates/cancellai-platform/src/allocation.rs`,
  `rust/crates/cancellai-platform/src/identity.rs` (new `IdentityToken::kind`/`device`
  accessors) as added in this change

## Outcome

PASS

## Scope

`FileFacts` is the OBSERVE-stage evidence record (`docs/architecture/DOMAIN_MODEL.md`'s
`AgentArtifact` "Minimum conceptual fields" slice: `LogicalSize`, `AllocatedSize?`, observed
timestamps, `ArtifactType`, `IdentityToken`) - deliberately not the full `AgentArtifact`
itself, since `RiskClass`/`Reversibility`/`KnowledgeConfidence`/lifecycle axes/
`AuthorityCeiling` require provider/policy knowledge that does not exist yet (E05/E06). This
story adds a new platform seam, `AllocationObserver` (allocated/physical size,
`rust/crates/cancellai-platform/src/allocation.rs`), rather than folding it into the existing
`FsObserver` - it mirrors that seam's `Absent`/`Unreadable`/`Unsupported` split, is additive
(no existing type's shape changed), and keeps allocated size an independently-observed fact
never derived from logical size.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Facts distinguish logical from reclaimable/allocated estimates when supported | `ac1_a_fully_observed_file_distinguishes_logical_from_allocated_size` constructs a sparse-file-like fixture (10MB logical, 4KB allocated) and asserts both metrics are independently correct and unequal - proving they are genuinely two separate observations, not one derived from the other. | PASS |
| AC2 - Unsupported metrics are explicit null/unknown values | `ac2_unsupported_allocation_is_an_explicit_value_never_a_fabricated_zero_or_logical_copy` asserts an unsupported allocation observation produces `SizeMetric::Unsupported{reason}`, not `Known(0)` and not a copy of `logical_size`. `unsupported_and_absent_states_serialize_explicitly_never_as_null_or_zero` (golden test) proves this holds at the JSON serialization boundary too - `{"state":"unsupported","reason":...}`, never `null` or `{"state":"known","bytes":0}`. `unsupported_identity_still_produces_a_usable_fact_with_degraded_confidence` proves the same for the identity axis: an `Unsupported` identity still yields a `Present` fact (not silently dropped), with `FactConfidence::Partial` naming why. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008/SI-009 (generalized to per-fact evidence; full scope-level closure is E04-S03) | Identity `Unsupported`/`Unreadable`/raced-`Absent`; allocation `Unsupported`/`Unreadable` | Every degraded sub-observation is recorded as a named reason in `FactConfidence::Partial`, never silently dropped or upgraded to `Complete`. `boundary_unknown_when_identity_cannot_be_established_never_assumed_within_scope` additionally proves an unestablished identity is never treated as "same device as scope" by default (SI-017 applied to the new `ScopeBoundary` field). | PASS |
| SI-013/SI-017 (identity revalidation / platform-native identity semantics) | `IdentityObservation::Absent` returned where `FsObserver` still reported `Metadata` (a TOCTOU race between the two independent stat calls) | `identity_observation raced` reason path in `observe_file_facts` - exercised indirectly by the confidence tests above; the fact is still produced (SI-010: never silently dropped) but marked `Partial`, and `kind` falls back to `FsObserver`'s coarser classification rather than trusting a stale identity. | PASS |
| SI-010 (scan errors are visible) | `Observation::Unreadable`, `AllocationObservation::Unreadable` | `unreadable_path_is_reported_not_collapsed_to_absent_or_empty` - a `FsObserver`-level failure short-circuits to `FactObservation::Unreadable{reason}`, never `Absent` or an empty `FileFacts`. | PASS |

## Verification Commands

```text
# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test -p cancellai-platform -p cancellai-inventory
cargo deny check

# Python governance (repository-wide)
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check
```

`cargo test -p cancellai-platform` runs 30 tests (28 prior + 2 new `allocation` tests, all
green). `cargo test -p cancellai-inventory` runs 9 unit tests plus 2 golden tests in
`tests/facts_golden.rs`, all green on the first run (no fixes needed after initial
compilation, other than correcting `SizeMetric::Known` from a tuple variant to a
`{ bytes: u64 }` struct variant once `serde` rejected an internally-tagged newtype variant
containing a bare integer - a `serde` representation constraint, not a design defect).

## Compatibility

- `AllocationObserver`'s real implementation is Unix-only (`st_blocks * 512`); non-Unix
  targets get `Unsupported` today, matching `IdentityObserver`'s existing Windows posture
  (E03-S01) - not a new gap this story introduces.
- No platform-conditional code path in `file_facts.rs` itself; it is pure composition over
  the three seams' trait objects, so it is portable everywhere those seams already compile.

## Performance / operability

- `observe_file_facts` performs at most three observations (`FsObserver`, `IdentityObserver`,
  `AllocationObserver`) per path, each a single `symlink_metadata`-class syscall on the real
  implementations - no additional filesystem access beyond what E02-S04/E03-S01 already do
  per path.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md` - new "FileFacts: the OBSERVE-stage evidence
  `AgentArtifact` is built from" subsection (the story's declared documentation impact).
- `docs/architecture/PLATFORM_MODEL.md` - new "Allocated-size observation" section
  documenting the `AllocationObserver` seam this story adds (documentation impact expanded
  beyond the story's original single-file declaration, since a new platform capability was
  introduced - AGENTS.md: "add more if implementation changes more contracts").

## Residual risks

- `AllocationObserver` is Unix-only; a verified Windows/other-platform allocated-size
  implementation is deferred, matching E03-S01's existing Windows-identity residual - not a
  new gap.
- `provider_hint`/`category_hint` on `FileFacts` are always `None` - populated by a future
  provider-adapter/classification epic (E05/E06), not by this story (documented in both the
  module doc comment and `DOMAIN_MODEL.md`).
- `IdentityToken::kind()`/`device()` are exhaustive matches over the enum's single `Unix`
  variant today; adding a Windows variant later is a compile-time-forced update to both
  accessors, not a silent gap.

## Verifier verdict

PENDING - epic E04 review runs once every story in E04 is `ready_for_review` (at most twice per epic, per ADR-0014).
