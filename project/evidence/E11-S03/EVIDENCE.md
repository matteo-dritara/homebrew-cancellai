# Evidence Packet - E11-S03

- Commit/PR: this working-tree change (executor session, 2026-09-14)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR3 (declared CR2 in the story contract; raised during execution - see "Risk
  reclassification" below)
- Spec version/commit: `project/epics/E11.json` (E11-S03), as of this change

## Risk reclassification: CR2 -> CR3

The story contract declares CR2. `project/risk_floors.json` sets a CR3 floor on the whole
`rust/crates/cancellai-policy/src/*` path ("Policy resolution decides eligibility. It cannot
mutate, but it decides what may be."), and the pre-commit risk-floor gate refused the CR2-declared
commit on exactly that pattern - `explanation.rs` lives under that path. Rather than route around
it or leave it as a pending-owner baseline entry (the treatment three earlier stories touching
this same crate received - E08-S02, E08-S03, E22-S04, all recorded in `project/risk_floors.json`'s
`baseline` section as "owner decision pending"), the story's `change_risk` is raised to CR3
directly, matching the same choice already made for E11-S01. `python3 scripts/
check_mutation_boundary.py check` (CR3's additional required gate) passes, unchanged from
baseline.

## Outcome

PASS

`rust/crates/cancellai-policy/src/explanation.rs` adds `explain_policy`, which calls
`resolver::resolve_effective_authority` (E11-S02) and reshapes its return value into one ordered
`PolicyExplanation`: `steps` (every named constraint `cancellai_safety::effective_authority`
evaluated, in the fixed order `base_constraints` builds it, each flagged `bound` when it is among
the constraint(s) tied at the final minimum), `requested_source`/`requested_level` (which policy
scope supplied the request and what it asked for), `final_level`, `binding_constraints`, and
`suppressed` (whether the request was granted in full). This module invents no new authority
logic and derives no fact `effective_authority`/`resolve_effective_authority` did not already
produce - see the module doc for why "return a trace showing every rule/evidence item that
contributed to the final result" (this story's outcome) is a reshape of `EffectiveAuthority::
trace`, not a new computation.

`docs/architecture/POLICY_MODEL.md`'s own worked example ("Requested: AUTOPILOT / Global policy:
AUTOPILOT / Project retention: eligible / ... / Final: QUARANTINE") includes two facts this
codebase does not yet model as real constraints - project retention eligibility and provider
quarantine capability - both explicitly deferred to E11-S04/E16-era provider-capability wiring.
This story implements the mechanism (an ordered, deterministic trace ending in a final level,
naming exactly what bound it) using the constraints that exist today; see Residual risks.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Explanation order is deterministic | `explanation::tests::ac_the_same_inputs_produce_byte_for_byte_identical_explanations_across_repeated_calls` (full `PolicyExplanation` equality across two calls with identical inputs) and `...ac_step_order_matches_the_fixed_base_constraints_order_every_time` (asserts the exact, named step order: `user_authority`, `artifact_authority_ceiling`, `confidence_authority`, `lifecycle_authority`, `provider_trust_authority`, `constitutional_safety_floor` - not merely "some order", the documented one) | PASS |
| AC2 - Suppressed higher-authority requests explain exactly which ceiling won | `explanation::tests::a_suppressed_request_names_exactly_the_one_ceiling_that_won` (a single binding constraint, both as the top-level `binding_constraints` field and as the one `steps` entry flagged `bound`) and `...a_suppressed_request_names_every_tied_ceiling_not_just_the_first` (two constraints tied at the same minimum - both named, mirroring `cancellai-safety::authority`'s own equivalent proof for the underlying mechanism). `...a_request_granted_in_full_is_not_reported_as_suppressed` proves the negative case is not vacuously always "suppressed" | PASS |

## Safety Evidence

This story declares no `safety_obligations` and adds no new authority-affecting logic (see
Outcome); CR3 here reflects the crate-wide policy-resolution floor rather than a new safety
mechanism this story itself introduces (see "Risk reclassification" above). The one property
worth stating directly: this module cannot itself become a second, diverging account of "what
was decided," because every
field on `PolicyExplanation` is copied from `EffectiveAuthority`/`ResolvedRequest` rather than
recomputed - `the_first_step_is_always_the_resolved_policy_request` and the suppression tests
above prove the reshape is faithful (the explanation's own `final_level`/`binding_constraints`
agree with what the resolver actually returned), not a parallel narrative that could drift from
it.

Falsification axes considered (`adversarial-cases` skill, run before implementation): axes 1-6
and 9-10 do not apply - this module performs no I/O, no mutation, holds no shared state, and
consumes only already-validated in-memory values (`PolicyDocument`, `AuthorityInputs`), not raw
untrusted input. Axis 7 (boundary values): an empty document with no matching scope at all
(`an_empty_document_still_produces_a_full_deterministic_explanation`) still produces the full
six-step trace. Axis 8 (policy/trust conflicts) is this story's core subject, covered by the
suppression tests above. Axis 11 (performance): the reshape is a single pass over a `Vec` already
bounded at six entries by `base_constraints`' own fixed shape - no measurable cost.

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

All PASS locally. `cargo test -p cancellai-policy --lib`: 88/88 (7 new in
`explanation::tests`). Full workspace `cargo test --workspace`: every crate green, no
pre-existing test's behavior changed.

## Compatibility

- No existing public API changed signature or behavior anywhere in the workspace;
  `explanation.rs` is purely additive.

## Documentation updated

- `docs/PRODUCT.md` - new paragraph after the Explain screen section (the story's declared
  documentation impact), distinguishing the artifact-level Explain screen (E09-S03) from the
  policy-level explanation graph (E11-S03).
- `docs/architecture/POLICY_MODEL.md` was already updated by E11-S01/S02 for the schema/resolver;
  this story's mechanism is documented in `explanation.rs`'s own module doc, which the "Rust
  constraint resolver" section already links to conceptually via the shared "Explanation
  contract" heading - no further edit needed there beyond what S02 already added.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **`docs/architecture/POLICY_MODEL.md`'s worked example is not yet byte-for-byte reproducible.**
  "Project retention: eligible" and "Provider quarantine capability: VERIFIED" are illustrative
  prose, not real constraints this codebase computes today - retention eligibility is E11-S04's
  outcome, and provider capability-as-an-authority-constraint does not exist yet
  (`cancellai_safety::authority`'s own module doc: "`ProviderCapabilityAuthority` is not wired
  in - no capability-classification subsystem exists yet"). This story implements the mechanism
  (ordered, deterministic, named-ceiling-wins trace) against the constraints that do exist; a
  future story wiring those two facts as real constraints gets the same explanation graph for
  free, without needing to touch this module.
- **No text/human-readable renderer.** `PolicyExplanation` is structured data
  (`docs/architecture/POLICY_MODEL.md`: "The engine exposes the same explanation graph to CLI,
  TUI, Guardian, and later fleet UI") - deliberately not a fixed string format, since choosing
  one is presentation-layer work for whichever surface consumes it first, not this story's.
- **No CLI/TUI/Guardian integration.** Same residual E11-S01/S02 already carry - nothing yet
  calls `explain_policy` from a command surface.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
