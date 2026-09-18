# Evidence Packet - E18-S02

- Commit/PR: local checkpoint on `main`,
  `rust/crates/cancellai-safety/src/remote_execution.rs` + `lib.rs` +
  `docs/architecture/TARGET.md` + `docs/security/THREAT_MODEL.md` + `CHANGELOG.md` +
  `project/epics/E18.json`
- Executor: Claude
- Independent verifier: none yet - awaiting E18's epic-scope review round (ADR-0014/ADR-0025),
  once every story in E18 reaches `ready_for_review`
- Change Risk: CR4 (declared at planning; `rust/crates/cancellai-safety/src/*` is already
  floored at CR4 in `project/risk_floors.json` - no reclassification needed, unlike E18-S01)
- Spec version/commit: `project/epics/E18.json` as committed in this checkpoint;
  [RFC-0001](../../../docs/rfcs/0001-remote-execution-boundary.md) (accepted) /
  [ADR-0031](../../../docs/adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md)
  record the accepted design this implements

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Text (verbatim, `project/epics/E18.json`) | Evidence | Result |
| --- | --- | --- | --- |
| AC1 | "Remote control cannot bypass target safety invariants." | `VerifiedRemoteIntent` can only ever carry `target`/`requested_authority` - no plan, no `SealedPlan` reference, no other `AuthorityInputs` field; a request above the controller's `authority_ceiling` is refused, never clamped (`a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped`, `a_request_naming_exactly_the_ceiling_is_accepted`). `remote_and_local_user_requested_reach_identical_effective_authority` proves `effective_authority`'s output for a remote-originated `user_requested` is identical to the same value supplied locally - no hidden second authority path. | PASS |
| AC2 | "Every remote request is authenticated, authorized, and audit-linked." | Authenticated: `unknown_controller_is_rejected_before_any_crypto_check`, `malformed_signature_text_is_rejected_without_panicking`, `tamper_a_digest_updated_to_match_tampered_target_still_fails_signature_verification`. Authorized: `a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped`; replay-authorized: `a_replayed_equal_sequence_is_rejected`, `an_out_of_order_lower_sequence_is_rejected_even_from_the_same_controller`, `sequences_from_different_controllers_never_interfere`. Audit-linked: every `Result` (`Ok`/`Err`) together with the caller's own `request` carries controller id, sequence, target, requested authority, and (on rejection) the specific `RemoteExecutionError` - everything an `EventLedger` entry needs, per RFC-0001's corrected design (see Method defects). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-031 (remote/fleet requests are intents; target authenticates, resolves policy, retains final mutation authority) | A remote-originated `user_requested` reaching `effective_authority` through any path other than the exact same field a local caller uses | `remote_and_local_user_requested_reach_identical_effective_authority` builds `AuthorityInputs` once from a verified remote intent and once from the identical value supplied directly, and asserts `effective_authority` returns the same `EffectiveAuthority` both times | PASS |
| SI-031 / TM-17 (a remote party escalating its own request) | Request asking for `Autopilot` from a controller trusted only to `Quarantine` | `a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped` - rejected outright, `requested_authority` is never silently lowered to the ceiling and returned as if granted | PASS |
| C-03 (ambiguity never escalates privilege) / replay | Same-sequence and lower-sequence requests from an already-seen controller; a rejected request's sequence must not be "consumed" | `a_replayed_equal_sequence_is_rejected`, `an_out_of_order_lower_sequence_is_rejected_even_from_the_same_controller`, `a_rejected_request_never_advances_the_recorded_sequence` | PASS |

## Verification Commands

```text
$ cargo test -p cancellai-safety remote_execution
running 17 tests
test remote_execution::tests::a_correctly_signed_request_within_ceiling_verifies ... ok
test remote_execution::tests::unknown_controller_is_rejected_before_any_crypto_check ... ok
test remote_execution::tests::unsupported_schema_version_is_rejected_before_any_crypto_check ... ok
test remote_execution::tests::tamper_a_digest_updated_to_match_tampered_target_still_fails_signature_verification ... ok
test remote_execution::tests::tamper_a_payload_changed_after_signing_fails_content_digest_check ... ok
test remote_execution::tests::malformed_signature_text_is_rejected_without_panicking ... ok
test remote_execution::tests::expiry_at_exactly_the_boundary_is_rejected ... ok
test remote_execution::tests::a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped ... ok
test remote_execution::tests::a_request_naming_exactly_the_ceiling_is_accepted ... ok
test remote_execution::tests::a_first_request_from_a_new_controller_is_never_stale ... ok
test remote_execution::tests::a_replayed_equal_sequence_is_rejected ... ok
test remote_execution::tests::an_out_of_order_lower_sequence_is_rejected_even_from_the_same_controller ... ok
test remote_execution::tests::a_rejected_request_never_advances_the_recorded_sequence ... ok
test remote_execution::tests::sequences_from_different_controllers_never_interfere ... ok
test remote_execution::tests::malformed_json_is_rejected_by_parse_request_without_panicking ... ok
test remote_execution::tests::an_unrecognized_field_is_rejected_before_verification_is_ever_reached ... ok
test remote_execution::tests::remote_and_local_user_requested_reach_identical_effective_authority ... ok
test result: ok. 17 passed; 0 failed

$ cargo fmt --check
(clean, no output)

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s)   (no warnings)

$ cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test --workspace
(every crate) test result: ok ... 0 failed  (workspace total, no regression)

$ cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 87 Rust source files scanned; only
rust/crates/cancellai-platform/src/mutation.rs deletes anything, only
rust/crates/cancellai-platform/src/mutation.rs, rust/crates/cancellai-safety/src/mutation_executor.rs
reference the capability that does

$ python3 scripts/check_rust_workspace.py check
rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated

$ python3 scripts/check_schemas.py check
schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md

$ python3 scripts/check_fixtures.py check
fixtures OK: 13 fixtures cover all required categories

$ python3 scripts/check_docs.py check
docs OK: 404 Markdown files; local links and safety IDs are consistent

$ python3 scripts/project_os.py check
governance OK
```

Not run in this session (unavailable/not required for this change - CI runs the full matrix):
cross-platform clippy (`--target x86_64-pc-windows-gnu`/`-unknown-linux-gnu`) - not required per
`AGENTS.md`'s own rule (this change touches neither `cancellai-platform` nor the workspace lint
surface). `cargo +nightly miri` - weekly-only per `AGENTS.md`, not per-PR. Python checks
(`ruff`/`mypy`/etc.) - no Python files touched. No CR4 release gates (G1-G4,
`docs/development/RELEASE_GATES.md`) were run as a formal gate pass - this story ships as a
library-level primitive with no production entry point, matching E13/E18-S01's own precedent;
G1-G4 apply at the point a real caller wires this to a release-eligible surface.

## Compatibility

- Pure Rust, kernel-ring only (`cancellai-safety`). No OS/filesystem/platform interaction beyond
  what `ed25519-dalek`/`sha2` already did for `knowledge_bundle` (ADR-0024, no new dependency).
  No CLI/TUI/Guardian wiring; nothing in this crate changed its dependency edges
  (`check_rust_workspace.py` confirms 13 crates, still acyclic, model/safety still isolated).

## Performance / operability

- `TrustedRemoteControllers::find` is a linear scan over an operator-configured list, mirroring
  `LocalTrustPolicy::find`'s identical, deliberate choice - a locally-trusted-controller list is
  expected to be small (an operator-curated set, not thousands of entries).

## Documentation updated

- `docs/architecture/TARGET.md`: new "Remote execution boundary (E18-S02)" subsection.
- `docs/security/THREAT_MODEL.md`: TM-17 updated to name the concrete implementation (the
  story's own declared `documentation_impact`).
- `CHANGELOG.md`: `[Unreleased] > Added` entry.
- `docs/rfcs/0001-remote-execution-boundary.md` / `docs/adrs/0031-...md`: corrected before any
  code was written (see Method defects) and left as the accepted design record.

## Method defects

- **What happened**: RFC-0001/ADR-0031's first draft said the request verifier "writes one `cancellai_store::EventLedger` event" directly - but `cancellai-safety` is kernel ring and `cancellai-store` (which owns `EventLedger`) is outer ring specifically so the kernel stays free of `rusqlite` (ADR-0019), so that dependency edge would have been refused had it ever been attempted in code. **Prevented by**: nothing prompted the check automatically at RFC-drafting time - I found it by deliberately reading both crates' `Cargo.toml` dependency lists before writing any code, not because a gate or template asked for it; `docs/rfcs/README.md`'s RFC template has no prompt to verify a proposed mechanism against `ADR-0019`'s ring assignments before recommending it. **Disposition**: proposed 2026-09-18 - whether the RFC template or the `rust-kernel-guard` skill should explicitly prompt "check the ring of every crate a recommendation would touch" before a design is accepted is an owner call, not made here. The design itself was corrected in the same session, before implementation, at no cost to this story.
- none otherwise.

## Residual risks

- `RemoteExecutionLog` is pure in-memory and does not survive a process restart - a caller that
  needs replay protection to survive restarts must persist and reload it themselves; this crate,
  by design (kernel-ring, no I/O), does not do so. Documented in `docs/architecture/TARGET.md`'s
  own "Residual" note.
- No production entry point calls any of this yet - it is a verified-but-unwired library
  primitive, identical in kind to E13's ledger and E18-S01's `RemoteTarget` at their own
  `ready_for_review`. The CR4 release gates (G1-G4) and an end-to-end exercise of TM-17's control
  apply once a real transport (E18-S03 or later) wires a caller to it - this story's own
  verification is necessarily unit-level until that caller exists.
- `TrustedRemoteControllers`/`RemoteExecutionLog` construction and configuration (how an operator
  actually lists a trusted controller and its ceiling) has no CLI/config-file surface yet - purely
  a Rust API today, matching this story's own CR4 scope (the authority boundary itself, not its
  operator-facing configuration).
- Signature verification uses `ed25519_dalek::VerifyingKey::from_bytes`/`verify_strict` exactly as
  `knowledge_bundle` already does (ADR-0024) - no new cryptographic review surface beyond what
  that ADR already accepted.

## Verifier verdict

(blank - awaiting E18's epic-scope independent review round; no CR4 Safety Verdict is included
here, as it is the independent reviewer's output, gated at `done`)
