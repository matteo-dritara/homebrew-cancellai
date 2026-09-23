<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S07
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: 109e9df668259845181d2c5e79b6ffad84a4575702665fd3e292b266f5905350

<!-- end handoff header -->
# Verifier Brief - E06-S07 - The CLI's authority passes through the release channel and the local containment ledger

Status: ready_for_review | Change Risk: CR4
Outcome: ADR-0039 puts signed incident containment inside the cutover perimeter, fed by locally installed notices. Today `clean` seals every plan at a constant Govern, so neither the release-channel ceiling (E17-S05) nor containment (E17-S07) can reach a mutation; LocalTrustPolicy is never constructed outside tests; and the ledger lives in memory, so restarting the process would lift every containment. This story makes authority computed rather than asserted, compiles in the project incident-response key, adds the explicit containment install/list/lift commands, persists the ledger, and closes E17-S07's round-5 residual by capping a notice's bytes before parsing. It builds on E17-S07's ledger, which is done before this story starts; the edge is not declared because E17 sits in a later roadmap phase, the same inversion E06-S04's blocker already records.
Dependencies: none

## Acceptance Criteria
- The system shall compute every plan and clean action's authority with effective_authority_under_containment from the running build's compiled release channel and the persisted containment ledger, and seal the plan at that authority rather than a constant.
- If an action's effective authority is below the minimum its action class requires, then plan and clean shall both downgrade it to Observe and explain which constraint and which incident capped it.
- When the owner runs containment install on a notice signed by the compiled project key or an owner-trusted publisher, the system shall ingest it and persist the ledger atomically before reporting success.
- If a notice file exceeds the fixed byte cap, is unsigned, is signed by an untrusted publisher, is malformed, replays a sequence or is expired, then containment install shall refuse it, leave the persisted ledger byte-identical, and exit non-zero.
- If the ledger file exists but cannot be read, parsed or validated, then every action above Recommend shall be capped at Recommend and the CLI shall say why, because unknown state is not an empty ledger.
- If the ledger file is missing, then the ledger shall be empty and authority shall equal the release-channel-constrained kernel authority.
- If a persisted containment's bundle has expired, then it shall still bind, because only containment lift --confirm removes a containment.
- The system shall accept no trusted publisher, ceiling or lift from any notice content; trust comes only from the compiled key and the owner's private trust file.

## Verification Contract
- Integration tests drive the CLI binary against synthetic roots with a test-only signing key: a contained provider's deletions become Observe in plan and clean alike, and an uncontained one still deletes.
- Adversarial tests cover an oversized notice, a forged signature, an untrusted publisher, a replay, an expired notice, a corrupt ledger file, a truncated ledger file and a missing ledger file.
- A mutation check shows that removing the containment constraint, or reverting the seal to a constant Govern, fails the suite.

## Safety Obligations

### SI-019 One mutation boundary, evidence-gated

All filesystem/vendor mutations route through the safety executor. CR4 changes to this boundary require independent verification and owner-visible Safety Verdict.

Implemented at `rust/crates/cancellai-safety/src/mutation_executor.rs::execute` (E03-S05),
the sole production caller of `cancellai-platform::mutation::MutationExecutor`.
`scripts/check_mutation_boundary.py` statically enforces that the raw OS primitive and the
capability wrapping it are referenced only from those two files - E03 verifier review round 1
found the capability itself was `pub`, re-exported at `cancellai_platform`'s crate root, and
directly callable (with an unconstrained raw path) by any crate that imported it; repaired by
removing the re-export and extending the static check (`docs/architecture/TARGET.md`,
`docs/architecture/PLATFORM_MODEL.md`).

### SI-022 Knowledge is data, not executable authority

Remote/local knowledge bundles cannot inject arbitrary commands/code or raise local destructive authority. Invalid signatures/provenance are rejected.

Partially implemented at `rust/crates/cancellai-safety/src/trust_promotion.rs` (E05-S02):
`TrustPromotionEvidence` carries only inert strings (a verifier name, fixture reference
identifiers) with no command/code field for anything to execute, and raising authority through
it requires passing through `TrustedTier::promote`'s fail-closed checks - the only public path
from which a `TrustedTier` above `Untrusted` can be obtained (see SI-021 above for the round-1
repair that made this actually true, not merely intended). E16-S02 implemented bundle signature/provenance verification in
`rust/crates/cancellai-safety/src/knowledge_bundle.rs`. E17-S08 independent review found that
its call used permissive verification despite ADR-0024 requiring `verify_strict`; the repair
uses strict Ed25519 verification and rejects weak local-policy keys/signatures before a bundle
can replace the current store. A regression proves refusal preserves the full current value.

### SI-029 Knowledge rollback/tamper fails closed

Invalid, expired, replayed, or unauthorized knowledge updates are rejected or downgraded to non-destructive use; a bad update cannot brick basic offline inspection.

Implemented for bundles at `rust/crates/cancellai-safety/src/knowledge_bundle.rs` (E16-S02) and
for incident containment at `rust/crates/cancellai-safety/src/incident.rs` (E17-S07): a refused
containment update - unreachable service, malformed text, bad signature, unknown publisher,
expiry, replay - leaves the containment ledger exactly as it was, and no refusal, rollback or
expiry can remove a containment already recorded. Only a local lift does.

Since E06-S07 the ledger persists as a history of raw signed bundles and local lifts that the
kernel re-verifies on every load, skipping only the expiry check (expiry never lifts). The
guarantee covers every input from outside the owner's account. It does not cover the owner's own
state: a process running as the same user can delete, truncate or append to the history, and
each of those lifts containment - an accepted limit (owner decision, 2026-09-23;
`docs/security/INCIDENT_RESPONSE.md`), not a gap in the verification.

### SI-030 Release channel bounds default authority

Experimental/nightly builds do not inherit stable-level autonomous destructive defaults merely because user configuration exists from a stable install.

Implemented at `rust/crates/cancellai-safety/src/authority.rs` and
`rust/crates/cancellai-safety/src/build_channel.rs` (E17-S05, [ADR-0023](../../../docs/adrs/0023-release-channel-authority-as-opt-in-function.md)):
`effective_authority_for_channel`'s `release_channel_authority` constraint caps the
monotonic-minimum result by channel (`Nightly` at `Recommend` - strictly below what
`ActionClass::Quarantine` requires, let alone `Delete` - `Beta` at `Govern`, `Stable` with no
additional cap; `docs/security/SUPPLY_CHAIN.md`'s "Release channels" table). It accepts only
`BuildChannel` - an opaque wrapper around `cancellai_model::ReleaseChannel` with a private field
and no `From<ReleaseChannel>` - not the bare, freely-constructible `ReleaseChannel` enum itself,
mirroring SI-021's `TrustedTier` split for the identical reason. `BuildChannel`'s only
production constructor, `from_compiled_env`, reads `CANCELLAI_CHANNEL` via `option_env!` at the
moment `cancellai-safety` is compiled - baked permanently into the resulting binary, never a
runtime value a user running that binary could set to claim a higher channel than it actually
is; an absent or unrecognized value resolves to `Nightly`, the fail-closed floor, matching
`BuildChannel::default()` exactly. `.github/workflows/release.yml`'s `build-artifacts` job
(E17-S02) sets `CANCELLAI_CHANNEL=stable` for every canonical tier-1 build.

`effective_authority_for_channel` is a second function alongside the pre-existing
`effective_authority`, not a new required field on `AuthorityInputs` - ADR-0023 records why
(the one production caller of `AuthorityInputs`, `cancellai-policy::retention::
reachable_authority`, predates release-channel awareness and has no honest channel value to
supply yet, since `cancellai-cli` remains a beta, source-built artifact with no packaged
release). No caller in this workspace invokes `effective_authority_for_channel` yet - this
constraint is implemented and adversarially tested (`cancellai-safety::authority`'s own test
suite) but not yet enforced on any real mutation path. Wiring `cancellai-cli`'s classification
pipeline to it is deferred to E06-S04, the cutover story that makes `cancellai-cli` the
canonical, packaged-release engine.

## Documentation Impact
- docs/CLI_RUST.md
- docs/security/INCIDENT_RESPONSE.md
- docs/development/RELEASE_GATES.md
- CHANGELOG.md
- docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
