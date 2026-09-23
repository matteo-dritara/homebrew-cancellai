# Evidence Packet - E17-S07

- Commit/PR: the commit carrying this packet on `main`
- Executor: Claude
- Independent verifier: Codex (pending, E17 round 2)
- Change Risk: CR4
- Spec version/commit: `project/epics/E17.json` E17-S07; epic dependency corrected by PD-026

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "A compromised provider/version/capability can be downgraded to Observe or Recommend through trusted signed knowledge without granting any new mutation authority." | `rust/crates/cancellai-safety/src/incident.rs` tests: `a_signed_containment_caps_the_provider_at_its_ceiling`, `scoping_narrows_to_versions_actions_and_platforms`, `an_unknown_installed_version_is_treated_as_affected`, `the_ceiling_vocabulary_cannot_express_a_mutating_level` (quarantine/govern/autopilot/delete/restore refused as ceilings), `a_notice_cannot_smuggle_an_authority_restore_or_lift_field`, `a_reissued_incident_can_narrow_but_never_relax_its_ceiling`, `containment_never_raises_and_always_caps_below_any_mutation` (exhaustive over 5 user levels x 3 channels x 2 ceilings: result <= kernel result and < Quarantine), `containment_is_named_when_it_binds`, `a_containment_freezes_trust_promotion_for_its_provider_only` | PASS |
| AC2 "Offline operation remains available under the installed local safety kernel when the knowledge service is unavailable." | `an_unreachable_service_is_offline_and_keeps_the_existing_ledger`, `an_empty_ledger_is_exactly_the_installed_kernel` (exhaustive: `effective_authority_under_containment` with an empty ledger equals `effective_authority_for_channel`), `refresh_refuses_garbage_and_applies_a_valid_bundle` | PASS |
| AC3 "Incident evidence records affected release/knowledge provenance, capability, provider, platform, and invariant references without collecting provider payload content." | `evidence_records_provenance_capability_provider_platform_and_invariants` (publisher, sequence, issued_at, payload digest, release version+channel, provider versions, action classes, platforms, SI ids, affected releases; serialized record has no payload/content/path/message/reason key), `identifiers_that_could_carry_content_are_refused`, `release_provenance_comes_from_the_build_not_the_caller` | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Notice claims a mutating ceiling, or adds `authority`/`tier`/`lift`/`restore`/`command` fields | refused by the two-member ceiling enum and `deny_unknown_fields` | PASS |
| SI-022 | Forged signature, unknown publisher, payload tampered after signing | `an_invalid_signature_unknown_publisher_or_tampered_payload_changes_nothing` | PASS |
| SI-029 | Replay of the same sequence and of an older sequence | `a_replayed_bundle_is_refused_and_changes_nothing` | PASS |
| SI-029 | Knowledge-store rollback to the pre-incident bundle | `rolling_the_knowledge_store_back_does_not_lift_a_containment` | PASS |
| SI-029 | Later bundle omitting the incident; expiry passing after ingestion; expired bundle presented | `a_later_bundle_that_omits_an_incident_does_not_lift_it`, `an_expired_containment_bundle_is_refused_but_an_ingested_one_outlives_its_expiry` | PASS |
| SI-029 | Refused bundle advancing the replay counter and locking out a later valid one | `a_refused_bundle_does_not_advance_the_replay_counter` | PASS |
| SI-029 | One malformed entry among valid ones; malformed envelopes (empty, >64 entries, duplicate id, wrong schema, wrong kind) | `one_bad_entry_rejects_the_whole_notice`, `malformed_notice_envelopes_are_refused_and_nothing_is_recorded`, `empty_or_oversized_scopes_and_lists_are_refused_rather_than_guessed` | PASS |
| SI-030 | Containment under every channel | `containment_never_raises_and_always_caps_below_any_mutation` runs all three channels; the containment function composes the channel constraint rather than replacing it | PASS |

## Verification Commands

```text
cargo fmt --check                                                         -> ok
cargo clippy --workspace --all-targets --all-features -- -D warnings      -> ok
cargo clippy ... --target x86_64-pc-windows-gnu -- -D warnings            -> ok
cargo check --workspace --all-targets                                     -> ok
cargo test --workspace                                                    -> 1104 passed, 0 failed
cargo test -p cancellai-safety incident                                   -> 27 passed
cargo deny check                                                          -> advisories ok, bans ok, licenses ok, sources ok
python3 scripts/check_coverage.py check                                   -> cancellai-safety 98.79% (floor 97.78%)
pre-commit run --all-files                                                -> all hooks passed
python3 -m pytest tests -q                                                -> all passed
```

## Compatibility

- No new dependency: `serde`, `serde_json`, `sha2`, `ed25519-dalek` were already in `cancellai-safety`.
- Additive public API; no existing signature changed. `effective_authority` and
  `effective_authority_for_channel` are untouched.
- Wire format: `ContainmentNotice` schema_version 1, `kind: "capability_containment"`, carried in
  a knowledge bundle payload.

## Performance / operability

- Matching is linear in active containments (bounded by entries x publishers ingested); notices
  are capped at 64 entries and 32 items per list.

## Documentation updated

- `docs/security/INCIDENT_RESPONSE.md` ("Signed capability containment"), `docs/security/SUPPLY_CHAIN.md`
  ("Incident containment"), `docs/security/THREAT_MODEL.md` (TM-11 delta), `docs/security/SAFETY_INVARIANTS.md`
  (SI-029 implementation note), `docs/development/RELEASE_GATES.md` (G4 status), `CHANGELOG.md`.
- `project/decisions.json` PD-026 (E17's epic dependency on E06 replaced by the epics its stories consume).

## Method defects

- **What happened**: E17 declared an epic-level dependency on E06 while E06-S04 depends on E17-S07, so every E17 story could finish and the epic still could not close; the executor only found it when `project_os.py check` refused the status change, and corrected it through PD-026. **Prevented by**: none exists - `project_os.py check` validates that an epic's dependencies are done, not that a story-level edge running the opposite way contradicts the epic-level one. **Disposition**: proposed 2026-09-23

## Residual risks

- No caller on a live mutation path: like `effective_authority_for_channel`, the containment-aware
  authority function is implemented and tested but not yet invoked by `cancellai-cli`. Wiring it,
  and choosing where the ledger persists, belongs to the cutover (E06-S04).
- No distribution channel: nothing fetches containment bundles yet; `refresh` takes the fetched
  text or `KnowledgeUnavailable` from its caller.
- The ledger is in memory. Persistence must preserve monotonicity (a restart must not lift a
  containment); that requirement is recorded here for the story that adds persistence.
- A compromised trusted publisher key can downgrade providers it should not. The impact is
  bounded to Observe/Recommend (availability, not data loss) and is visible in the evidence record.
- Any publisher in the local trust policy may issue a containment regardless of its tier: listing
  a publisher is the local trust act, and a containment can only reduce authority.

## Round-2 repairs (independent review FAIL, `project/evidence/E17-VERIFIER-REVIEW-ROUND2.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A same-id re-issue (same or other trusted publisher) with a narrower scope replaced the broader record, uncontaining affected versions | The ledger is append-only: each verified entry is a record of its own; scope in force is the union, ceiling the strictest; `lift_locally` removes every record of an incident | verifier's `a_reissued_incident_cannot_shrink_scope_or_be_replaced_by_another_publisher`; `a_same_publisher_reissue_cannot_shrink_scope_or_relax_the_ceiling` (versions x action classes x platforms); `a_second_publisher_cannot_relax_another_publishers_ceiling`; `only_a_local_lift_removes_a_containment` (multi-record lift) |
| `ingest` accepted caller-supplied `ReleaseProvenance` with public fields, so evidence could claim a release the build is not | `ReleaseProvenance` has private fields and no public constructor; the ledger reads it once from compile-time metadata (`CARGO_PKG_VERSION`, `BuildChannel::from_compiled_env`); `ingest`/`refresh` take no release argument | `compile_fail` doctest on `ReleaseProvenance`; `release_provenance_comes_from_the_build_not_the_caller`; `evidence_records_provenance_capability_provider_platform_and_invariants` |

Gates after repair: `cargo test --workspace` 1108 passed; clippy `-D warnings` clean;
`cancellai-safety` coverage 98.74% (floor 97.78%); pre-commit all hooks passed.

## Round-3 repairs (independent review FAIL, `project/evidence/E17-VERIFIER-REVIEW-ROUND3.md`)

Round 3 confirmed both round-2 repairs and found two further defects. Each is repaired to the
round's required repair:

| Finding | Repair | Evidence |
| --- | --- | --- |
| `KnowledgeProvenance` and `IncidentEvidence` had public fields: callers could construct or edit records presented as incident evidence | Fields private, read-only accessors, construction only inside `ContainmentLedger::ingest` | three `compile_fail` doctests (construct provenance, construct evidence by struct update, edit a field); `evidence_is_readable_through_accessors_only` |
| Repeated valid notices grew retained state without bound (offline availability) | `MAX_ACTIVE_RECORDS` 4096 and `MAX_TRACKED_PUBLISHERS` 256; a notice that would exceed either is refused whole with `LedgerFull`, nothing evicted or weakened, the refused sequence not consumed; identical re-issues take no capacity | `at_capacity_a_notice_is_refused_whole_and_every_containment_is_kept`, `repeating_the_same_containment_consumes_no_capacity`, `the_publisher_bound_refuses_a_new_publisher_but_not_a_known_one`, `the_default_ledger_uses_the_published_bounds`; the capacity, publisher-bound and dedupe branches were each disabled in turn and caught (2, 1, 1 failing tests) |
| `cargo deny check` not completed by the verifier (advisory database unreachable from its sandbox) | Run here | `advisories ok, bans ok, licenses ok, sources ok` |

Gates after repair: `cargo test --workspace` 1160 passed; clippy `-D warnings` clean; `cargo deny
check` ok; `cancellai-safety` coverage 98.86%.

## Verifier verdict

Round 2: FAIL. Round 3: FAIL. Both repaired above. These were the two independent reviews the
owner allowed for this story; the Safety Verdict's latest line is round 3's FAIL, so the story
cannot close without a further independent verdict, which only the owner can authorize.
