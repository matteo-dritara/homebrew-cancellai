# Evidence Packet - E03-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E03)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-safety/src/authority.rs`, `rust/crates/cancellai-model/src/vocabulary.rs` (lifecycle axes) as added in this change

## Outcome

PASS

## Scope: which of the nine documented Effective Authority inputs are wired up

`docs/architecture/DOMAIN_MODEL.md`'s formula names nine inputs. This story implements the
lattice mechanism generically (`compute_effective_authority` takes any number of named
constraints) and wires up five of the nine for real: `UserAuthority`,
`ArtifactAuthorityCeiling` (caller-supplied - deriving a ceiling from `RiskClass` is a
classification decision out of this story's scope), `ConfidenceAuthority`,
`LifecycleAuthority`, and an explicit `ConstitutionalSafetyFloor`. `Reversibility`,
`ProviderCapabilityAuthority`, `ProviderTrustAuthority`, and `ReleaseChannelAuthority` are not
wired in - no provider adapter or release-channel subsystem exists yet to supply them (E05 and
a later release story). Adding them is supplying more named constraints to
`compute_effective_authority`, not a redesign - this is stated in both the module doc comment
and `docs/architecture/DOMAIN_MODEL.md`'s updated "Effective Authority" section, not left
implicit.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Raising user authority cannot raise an artifact above its ceiling | `ac1_effective_authority_is_exhaustively_the_minimum_of_user_and_ceiling_when_all_else_is_permissive` - table-driven over all 5x5=25 combinations of `user_requested` x `artifact_ceiling` with every other input maximally permissive, asserting the result is always exactly `user.min(ceiling)`. `ac1_raising_user_authority_never_raises_the_result_above_the_artifact_ceiling` sweeps `user_requested` across all 5 levels against a fixed low ceiling and asserts the result never exceeds it, plus proves the ceiling is actually reachable (not vacuously always-lower). | PASS |
| AC2 - Unknown/active/protected/partial states collapse to non-destructive authority | Six targeted tests (`ac2_unknown_activity_...`, `ac2_active_activity_...`, `ac2_protected_state_...`, `ac2_pinned_state_...`, `ac2_partial_integrity_...`, `ac2_unknown_integrity_...`, `ac2_low_unknown_confidence_...`) each set exactly one lifecycle/confidence axis to its "bad" value while every other input (including `user_requested`/`artifact_ceiling`) is `Autopilot`/`Verified`, and assert the result is `Recommend` - proving each axis alone is sufficient to collapse authority, not merely a co-occurring set of bad axes. `ac2_a_fully_permissive_artifact_is_not_vacuously_capped` proves the collapse is selective (a genuinely permissive artifact reaches `Autopilot`), not a function that always returns `Recommend`. | PASS |
| AC3 - Every authority result has a deterministic explanation trace | `ac3_the_trace_is_deterministic_across_repeated_calls_with_the_same_inputs` (same inputs -> identical `EffectiveAuthority`, trace included). `ac3_binding_constraints_names_the_single_actual_bottleneck` proves `binding_constraints` correctly names the one constraint that produced the minimum. `ac3_binding_constraints_names_every_tied_bottleneck_not_just_the_first` proves a tie (two constraints independently landing on the same minimum) names *both*, not just whichever the iteration order encounters first - a naive "first minimum wins" implementation would fail this test. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 (protected/unknown/insufficient-confidence is non-destructive; user policy cannot override) | Protection `Protected`; confidence `LowUnknown`; both at maximum user/ceiling authority | `ac2_protected_state_collapses_to_non_destructive`, `ac2_low_unknown_confidence_collapses_to_non_destructive` - both driven independently through `constitutional_safety_floor`, which no `user_requested` value can outrank in a minimum. | PASS |
| SI-006 (protection is defense in depth, checked more than once) | Protection `Protected` | `ac3_binding_constraints_names_every_tied_bottleneck_not_just_the_first` shows `Protected` binds *both* `lifecycle_authority` and `constitutional_safety_floor` simultaneously - the same fact is genuinely checked twice by two independent constraints, not once with two names. | PASS |
| SI-007 (ambiguous CLI/configuration is non-destructive) | A hypothetically-buggy CLI layer resolves ambiguity to a high `user_requested` | Structural, not directly tested here: `user_authority` is one of five independent minimum inputs, so `ac1`'s tests already show a high `user_requested` alone cannot raise the result past what the *other* four constraints independently allow. Full closure (refusing to resolve genuinely ambiguous input to any non-`Observe` value at all) is explicitly out of this story's scope - see `docs/security/SAFETY_INVARIANTS.md`'s updated SI-007 note. | PASS (structural contribution; not full closure) |
| SI-008 (partial scan is non-destructive) | Integrity `Partial` | `ac2_partial_integrity_collapses_to_non_destructive` | PASS |
| SI-009 (unknown scan state is non-destructive) | Integrity `Unknown`; Activity `Unknown` | `ac2_unknown_integrity_collapses_to_non_destructive`, `ac2_unknown_activity_collapses_to_non_destructive_even_at_maximum_everything_else` | PASS |
| Fail-closed on degenerate input | No constraints supplied at all | `compute_effective_authority_fails_closed_on_empty_input` - returns `Observe` (weakest level), never treated as "no limit." | PASS |

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Cross-platform compile verification (see E03-S01's evidence for why)
cargo check -p cancellai-model -p cancellai-safety --target x86_64-pc-windows-gnu --all-targets
cargo check -p cancellai-model -p cancellai-safety --target x86_64-unknown-linux-gnu --all-targets
cargo clippy -p cancellai-model -p cancellai-safety --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings
```

All passed. `cargo test -p cancellai-safety` now runs 30 tests (16 prior + 14 new `authority`
tests), all green - a genuinely exhaustive table (25 cases) for the core monotonic-minimum
property (AC1), plus seven independent single-axis collapse cases (AC2) and three
explanation-trace cases (AC3). No platform-conditional code was added in this story (unlike
E03-S01/E03-S03); the cross-target checks confirm the new vocabulary/lattice compile
identically everywhere.

## Compatibility

- No platform-specific behavior. `AuthorityLevel`/`ActivityState`/`ProtectionState`/
  `IntegrityState` are plain enums; the lattice computation is pure.

## Performance / operability

- `effective_authority` builds a 5-element `Vec` and takes one `min()` over it; no I/O, no
  allocation beyond that vector and the trace it returns.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md` - "Effective Authority" section states the Rust
  implementation and which of the nine documented inputs are wired up vs. deferred (the
  story's declared documentation impact).
- `docs/security/SAFETY_INVARIANTS.md` - SI-001, SI-007, SI-008, SI-009 each gained an
  implementation pointer; SI-007's note is explicit that this story only structurally
  supports it, not closes it (the story's other declared documentation impact).

## Residual risks

- `ArtifactAuthorityCeiling` is a caller-supplied raw `AuthorityLevel`, not derived from
  `RiskClass` by this story. A classification engine (E04 inventory / a later story) must
  supply a real, reviewed `RiskClass` -> ceiling mapping before this input is anything more
  than "whatever the caller happened to pass in."
- `Reversibility`, `ProviderCapabilityAuthority`, `ProviderTrustAuthority`, and
  `ReleaseChannelAuthority` are not wired into `effective_authority` (see "Scope" above) -
  recorded as scope for the stories that build those subsystems (E05, a release-channel
  story), not created unilaterally now (AGENTS.md: "Do not silently create product scope in
  code").
- `lifecycle_ceiling`'s exact mapping (which specific `ActivityState`/`ProtectionState`/
  `IntegrityState` values collapse authority, and to exactly `Recommend` rather than some
  other non-destructive level) is this executor's interpretation of DOMAIN_MODEL.md's prose
  and SI-001's wording ("cannot receive destructive authority"), not a value independently
  specified field-by-field anywhere in the docs before this story. It is a defensible,
  documented modeling decision (see the module's own doc comments), but a reviewer should
  treat it as a design choice open to dispute, not a transcription of an existing spec.
- `effective_authority`/`AuthorityInputs`/`EffectiveAuthority` are not wired into `SealedPlan`
  (E03-S02) yet - `SealedPlan.authority` is still a caller-supplied value, not one computed by
  this lattice. Wiring the two together (so a `SealedPlan`'s recorded authority is provably
  the lattice's own output) is natural follow-up scope, most likely alongside E03-S05, not
  created here since no caller/orchestration layer exists yet to do that wiring meaningfully.

## Verifier verdict

PENDING - epic E03 review runs once every story in E03 is `ready_for_review` (at most twice per epic, per ADR-0014).
