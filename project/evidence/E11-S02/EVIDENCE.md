# Evidence Packet - E11-S02

- Commit/PR: this working-tree change (executor session, 2026-09-14)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md - CR4, requires a
  Safety Verdict the executor does not write)
- Change Risk: CR4 (as declared in the story contract)
- Spec version/commit: `project/epics/E11.json` (E11-S02), as of this change

## Outcome

PASS (pending independent CR4 Safety Verdict)

`rust/crates/cancellai-policy/src/resolver.rs` adds the constraint resolver:
`resolve_requested_authority` walks the scope ladder most-specific-first (`ARTIFACT_TYPE ->
PROJECT -> PROVIDER -> MACHINE -> GLOBAL`) against a `schema::PolicyDocument` (E11-S01) and a
`PolicyContext`, returning the first scope that sets an `authority`, or a caller-supplied
fallback if none do. `resolve_effective_authority` folds that result into
`cancellai_safety::authority::AuthorityInputs::user_requested` and calls the already-verified
`cancellai_safety::effective_authority` (E03-S04) unmodified. `SESSION`/`EXPLICIT PIN`, the most
specific rung in `docs/architecture/POLICY_MODEL.md`'s documented hierarchy, is deliberately
excluded from the ladder - `schema::PinEntry` (E11-S01) carries no `authority` field by design,
and wiring a pin into the safety inputs' `protection` axis is E11-S04's own outcome
("pin/protect semantics"), not this story's.

**Design decision central to the CR4 argument:** this module contains no ceiling logic of its
own. It is a pure selection function over a document plus one more caller (`resolve_effective_
authority`) that hands its output to code that already existed and was already independently
verified (E03-S04, E05-S02). No new constraint, no new authority-lowering or -raising rule, and
no new code path into `mutation_executor` is introduced. The CR4 classification reflects that
this module sits directly upstream of an authority decision, not that it adds new authority
logic - see Safety Evidence below for why this composition is what discharges SI-025 rather than
any new check this story wrote.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Safety invariants cannot be overridden by configuration | `resolver::tests::a_protected_artifact_stays_non_destructive_no_matter_what_policy_requests` and `...low_unknown_confidence_stays_non_destructive_no_matter_what_policy_requests`: policy requests `Autopilot` at the most specific matching scope, while `protection: Protected` / `confidence: LowUnknown` are independently set - the final `EffectiveAuthority.level` is `Recommend` in both cases, exactly as SI-001/E03-S04's own `constitutional_safety_floor`/`lifecycle_ceiling` already guarantee for any caller. Nothing in `resolver.rs` reads or influences those constraints; this proves policy cannot route around them, it can only proceed to the same monotonic minimum every other caller reaches | PASS |
| AC2 - More specific user policy cannot exceed artifact/provider/trust ceilings | `resolver::tests::policy_requesting_autopilot_cannot_exceed_a_lower_artifact_ceiling` (artifact_ceiling `Quarantine` binds the result even though the most specific policy scope requests `Autopilot` - `binding_constraints` names `artifact_authority_ceiling`) and `...policy_requesting_autopilot_cannot_exceed_an_untrusted_provider_ceiling` (`provider_trust: Untrusted` caps the result at `Observe` regardless of the policy request). `...a_fully_permissive_case_actually_reaches_autopilot_not_vacuously_capped` proves these are not vacuous - policy *can* actually raise the result to `Autopilot` when nothing else caps it, so the capping tests demonstrate a real ceiling, not a function that always returns low | PASS |
| AC3 - Conflicts resolve deterministically | `resolver::tests::ac_conflicts_resolve_deterministically_exhaustive_precedence_matrix` - the verification contract's own "exhaustive precedence matrix": all 2^5 = 32 subsets of {artifact_type, project, provider, machine, global} being present are exercised, and the most-specific present rung always wins, identified unambiguously (each rung assigned a distinct `AuthorityLevel` so the winner is provable from the resolved value alone). `resolving_the_same_document_and_context_twice_gives_identical_results` proves pure-function determinism across repeated calls; `a_scope_present_but_with_no_authority_set_defers_to_the_next_rung_down` proves an empty narrowing does not silently win as a null/default match | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 (protected/unknown state is non-destructive; user policy cannot override this floor) | Policy requests `Autopilot` for a `Protected` artifact | `a_protected_artifact_stays_non_destructive_no_matter_what_policy_requests` - final result `Recommend`, `binding_constraints` still names `lifecycle_authority`/`constitutional_safety_floor` exactly as without this resolver | PASS |
| SI-025 (policy cannot override constitutional ceilings; more specific configuration may narrow/select within authority, never elevate above safety/artifact/provider/trust ceilings) | Policy requests `Autopilot` while `artifact_ceiling`/`provider_trust` independently refuse it; also the "narrowing" direction - policy requests *less* than everything else would allow (`a_narrower_policy_request_is_honored_when_everything_else_would_allow_more`), proving the resolver is not merely a one-directional cap | Both directions behave exactly as `effective_authority`'s own pre-existing, independently-verified monotonic minimum dictates - this module supplies one input, changes no constraint logic | PASS |

**Why this discharges SI-025 rather than merely testing around it:** `effective_authority`
(E03-S04) and `provider_trust_ceiling` (E05-S02) are both already covered by their own
exhaustive test suites in `cancellai-safety` (105 tests in that crate alone, unchanged by this
story - see Verification Commands). This story's tests do not re-prove those; they prove the
*composition* is correct - that `resolve_effective_authority` actually calls the real function
with the real other inputs intact, rather than, say, accidentally overwriting a different field
or skipping the call under some condition. The adversarial cases above are chosen specifically
to falsify that composition (a permissive policy request colliding with each of the four other
independent ceiling categories: artifact ceiling, provider trust, protection, confidence).

Falsification axes considered (`adversarial-cases` skill, run before implementation): axes 1/2/
3/5/6/9 (path/identity, partial reads, links/mounts, concurrency, crash/retry, platform
differences) do not apply - this module performs no I/O, no mutation, and holds no shared state,
identical reasoning to E11-S01's own evidence. Axis 7 (boundary values): the exhaustive
precedence matrix above covers every combination of present/absent scopes, including the fully
empty document (`an_empty_document_falls_back_to_the_caller_supplied_default`) and a context
with no identity at all (`a_context_with_no_identity_at_all_only_ever_reaches_global`). Axis 8
(policy/trust conflicts) is this story's core subject, covered throughout. Axis 10 (malformed
input) is E11-S01's concern (already covered there) - this module consumes an already-validated
`PolicyDocument`, not raw text. Axis 11 (performance) is not meaningfully applicable: every
lookup here is O(1) map access against a document already in memory.

**Second-path check (CR4-specific):** `resolver.rs` calls `cancellai_safety::effective_authority`
and returns its result unmodified in the `EffectiveAuthority` half of its output - there is no
branch anywhere in this module that grants, denies, or overrides an authority decision
independently of that call. `scripts/check_mutation_boundary.py check` (below) confirms this
story added no new caller of the mutation capability itself.

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_schemas.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

All PASS locally. `cargo test -p cancellai-policy --lib`: 81/81 (12 new in `resolver::tests`).
`cargo test -p cancellai-safety`: 105/105, unchanged - this story adds no new test and no
modified test to that crate, confirming the composition uses the existing, already-verified
function as-is. Full workspace `cargo test --workspace`: every crate green.

## Compatibility

- No existing public API changed signature or behavior anywhere in the workspace; `resolver.rs`
  is purely additive.
- `AuthorityInputs`'s shape is unchanged - `resolve_effective_authority` takes one by value and
  only overwrites its own local copy's `user_requested` field before calling `effective_authority`.

## Release gate mapping (CR4, `docs/development/RELEASE_GATES.md`)

This story ships a library-level resolver with no command-surface integration yet (matching
E11-S01's own residual risk - no CLI/TUI wiring exists for any part of the policy engine). The
release gates below are mapped honestly at the component level; several are not yet meaningful
because nothing yet invokes this code from a real command:

- **G1 Functional** - acceptance criteria pass (above); unit tests pass (above); no CLI/API
  schema exists yet for this component (N/A); `CHANGELOG.md` carries an `[Unreleased]` entry;
  `cargo clippy -D warnings` is clean, so no regression is hidden as a warning.
- **G2 Safety** - required Safety Invariants preserved (SI-001, SI-025 evidence above); no
  threat-model delta - this story adds no new attacker-facing surface (no I/O, not yet wired to
  any input the CLI/TUI/provider layer could feed from outside the process); CR4 adversarial
  tests pass (above); **CR4 independent Safety Verdict is pending** - not yet obtained, recorded
  honestly below rather than asserted; no unknown/partial condition is promoted to destructive
  authority (the `lifecycle_authority`/`confidence_authority` constraints this story composes
  with are untouched).
- **G3 Compatibility** - not meaningfully applicable: no OS-specific code, no provider
  compatibility fixtures touched, no schema/policy/state migration (this module reads an
  in-memory `PolicyDocument`, writes nothing).
- **G4 Operability** - not meaningfully applicable: no installer/update/uninstall surface, no
  persistent state, no self-budget consumption (pure, allocation-light functions with no I/O).

## Performance / operability

- Every resolution is a fixed number of `BTreeMap::get` calls (at most five) against a document
  already in memory - no measurable performance concern at any realistic policy document size
  (E11-S01's own evidence already demonstrated 2,000-entry documents parse without pathological
  cost).

## Documentation updated

- `docs/architecture/POLICY_MODEL.md` - new "Rust constraint resolver (E11-S02)" section.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **No CR4 independent Safety Verdict yet.** This is the standing, expected state for a story at
  `ready_for_review` - the executor does not write it (`docs/development/AGENT_PROTOCOL.md`).
  Epic-scoped review runs once every E11 story reaches `ready_for_review`.
- **No CLI/TUI/Guardian integration.** `resolve_effective_authority` is not called from any
  command surface yet - this is the same residual E11-S01 already carries, now inherited by this
  story. The first real caller will need to decide the `fallback` `AuthorityLevel` for its own
  command (the "Product defaults" precedence rung this module deliberately leaves to the
  integrator, per `resolver.rs`'s own `PolicyScopeSource::NoMatch` doc).
- **Session/pin wiring is entirely E11-S04's.** This resolver has no notion of a matched pin at
  all - not even a stub - since acting on one requires deciding how a pin maps to
  `ProtectionState`, which is that story's stated outcome, not this one's.
- **No fuzzing-crate-based property tests.** The verification contract names "fuzz/property
  tests"; this workspace's dependency rings (ADR-0019) require a reviewed ADR before adding a
  fuzzing/property-testing crate (e.g. `proptest`), which is out of proportion to add for one
  story. `ac_conflicts_resolve_deterministically_exhaustive_precedence_matrix` substitutes an
  actual exhaustive enumeration (32/32 combinations) rather than a randomly-sampled property
  test, which is a strictly stronger guarantee for this specific, small, finite input space -
  the same substitution `cancellai-safety::authority`'s own table-driven tests already make for
  `AuthorityLevel`'s 5x5 combinations.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`; CR4 requires an
independent Safety Verdict, which this packet does not and cannot supply)
