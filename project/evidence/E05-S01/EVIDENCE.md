# Evidence Packet - E05-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E05)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-provider-api/src/capability.rs` as added in this
  change

## Outcome

PASS

## Scope

Defines the provider capability contract (`docs/architecture/PROVIDER_MODEL.md` "Capability
contract"): the `CapabilityKind` enumeration (the nine named capabilities -
`detect`/`fingerprint_root`/`inventory_map`/`project_attribution`/`session_graph`/
`activity_state`/`native_delete_capability`/`retention_capability`/`explain`), the
evidence/confidence-bearing `CapabilityOutcome` envelope, and the `ProviderCapabilities` trait
every future adapter implements. Deliberately out of scope: the per-capability result
*payload* (a session graph's actual shape, a project attribution's actual fields) - those
belong to the adapter stories that produce real data (E05-S03 Claude, E05-S04 Codex) and to
the inventory/session-graph epics beyond E05, matching the precedent
`cancellai-safety::SealedPlan` and `cancellai-model::RootFingerprint` set for deferring fields
no real subsystem populates yet. `cancellai-provider-claude`/`cancellai-provider-codex` remain
unimplemented skeleton crates (E02-S01); this story does not touch them.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Capability absence is first-class and never inferred from provider identity | `ProviderCapabilities::capability` is the sole required trait method, has no default implementation, and takes no provider-identity input beyond `&self` - the crate defines no identity-keyed lookup table anywhere. `ac1_capability_absence_is_never_inferred_from_provider_identity` constructs two mocks sharing the same `provider_id` that disagree on `native_delete_capability`'s support, proving the contract does not tie support to identity. `ac1_a_provider_with_no_explicit_answer_reports_unsupported_not_a_guess` proves an unconfigured mock returns an explicit, evidenced `Unsupported` for every kind rather than panicking or defaulting to a positive claim. `ac1_one_capability_verified_does_not_imply_another_is` reproduces PROVIDER_MODEL.md's own worked example (verified for inventory, unsupported for native delete) on one provider instance. | PASS |
| AC2 - Capability responses carry evidence and confidence | `CapabilityOutcome`'s fields are private; the only public constructor, `CapabilityOutcome::new`, requires a `primary_evidence` string in addition to any further notes, so there is no way to construct a response with zero evidence entries - the same "invariant enforced by API shape" pattern `SealedPlan`/`PlanningView` use. `confidence` is a required, non-`Option` `KnowledgeConfidence` field. `ac2_every_capability_report_entry_carries_evidence_and_confidence` runs `capability_report` (all nine kinds) against a mixed mock and asserts every entry's evidence is non-empty and its confidence is one of the four defined variants. | PASS |

## Safety Evidence

None required - the story's declared Safety Obligations are `none`, and no Safety Invariant
is implemented here. `SupportState`/`CapabilityKind` use the same "stable, never renumbered or
repurposed" string-code idiom `cancellai-model::ErrorCategory` uses for SI-004
(`CompatibilityFailure`) so that a future story wiring capability outcomes into that
diagnostic taxonomy has a matching stable vocabulary to draw from - not a safety guarantee
made by this story itself.

## Verification Commands

```text
# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Python governance (repository-wide)
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check
```

`cargo test --workspace` runs the new `cancellai-provider-api` unit test module (7 tests, all
green) alongside every pre-existing crate's suite (54 tests total across the workspace plus
doctests), with no regression.

## Compatibility

- Pure data/trait definitions with no I/O, no platform-conditional code, and no provider-
  specific knowledge (`cancellai-provider-api` continues to have zero dependency on
  `cancellai-provider-claude`/`cancellai-provider-codex`, preserving the "this crate defines
  the contract and must not depend on a specific adapter" rule already stated in the crate's
  module doc).

## Performance / operability

- Not applicable - no runtime behavior beyond in-memory struct construction and an exhaustive
  match; `capability_report` allocates one `Vec` of nine entries per call.

## Documentation updated

- `docs/architecture/PROVIDER_MODEL.md` - new paragraph under "Capability contract" pointing
  at the Rust implementation and explaining how AC1/AC2 are enforced by type shape
  (the story's declared documentation impact).

## Residual risks

- The per-capability result payload types do not exist yet (see Scope above); adapter stories
  (E05-S03/E05-S04) will need to decide those shapes, and `CapabilityOutcome` may need a
  generic/associated payload slot at that point. This is a deliberately deferred design
  decision, not an oversight.
- `capability_report`/the mock conformance pattern is exercised only against a hand-written
  mock in this crate's own test module; no real adapter exists yet to run it against.

## Verifier verdict

PENDING - epic E05 review runs once every story in E05 is `ready_for_review` (at most twice
per epic, per ADR-0014).
