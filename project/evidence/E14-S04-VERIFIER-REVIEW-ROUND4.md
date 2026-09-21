# E14-S04 Independent Verifier Review — Round 4

Review-Scope: epic
Round: 4
Review-Target: `0de6d8d`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This owner-authorized exceptional round reviews E14-S04 only. E14-S01, E14-S02, and E14-S03
remain closed and were not re-judged. `scripts/check_process.py`'s
`REVIEW_ROUND_EXCEPTIONS["E14"]` authorizes this fourth review specifically for ADR-0035's
structurally different raw-observation design.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | FAIL | A downstream-style integration probe obtained a real public `Drifted` `LayoutDriftFinding`, converted the same raw facts through `authority_inputs_with_layout_observation`, and correctly got `Observe`. While retaining that finding, it separately built the same destructive-capable `AuthorityInputs` with public `ProviderLayoutAssessment::NotAssessed`; public `effective_authority` returned `Autopilot`. This is round 3's discard counterexample with raw facts in place of a derived ceiling. |

## Independent adversarial reproduction

A temporary `cancellai-guardian` integration test, compiled as a downstream crate using only
public APIs and removed before this record, established all of the following:

1. `assess_layout("recognized-name", ["sessions/"], ["unrecognized/"])` yields a real
   `LayoutSupport::Drifted` finding.
2. Public `TrustedTier::promote` with non-empty `TrustPromotionEvidence` produced a genuinely
   destructive-capable baseline. Passing that baseline and the raw Guardian signatures through
   `authority_inputs_with_layout_observation`, then public `effective_authority`, returned
   `Observe`.
3. Retaining the same real finding, a fresh public `AuthorityInputs` literal with every other
   input unchanged and `provider_layout: ProviderLayoutAssessment::NotAssessed` returned
   `Autopilot` through the same public `effective_authority`.
4. `ProviderLayoutAssessment::Observed` and `cancellai_safety::LayoutSignature::new` are public,
   so a caller can also assert a self-serving recognized observation by including its observed
   signature in `known_signatures`; it returned `Autopilot` even while the independently held
   Guardian signature was drifted. Private fields prevent malformed representation of a single
   signature, but do not attest that either raw fact came from the real provider root.
5. `compute_effective_authority(&[AuthorityConstraint { ceiling: Autopilot, .. }])` remains a
   public generic calculator and returns `Autopilot` without any `AuthorityInputs` at all.

The probe passed by reproducing these public API behaviors. Its source was deleted; no production
code or tests were changed by the review.

## Assessment against AC1 and SI-004

ADR-0035 correctly makes an `Observed` value deterministic *if that exact value reaches*
`effective_authority`: `layout_ceiling` independently derives `Observe` for a mismatch, empty
known set, or malformed/empty non-matching signature. `LayoutDriftFinding` also remains
non-forgeable outside `structural.rs`; its compile-fail doctest passes. These are genuine local
improvements, but not the property round 3 found missing.

The round-3 exploit did not alter the correct conclusion within one `AuthorityInputs`; it held a
real observation and constructed another authority input that omitted it. Replacing the omitted
`Option<AuthorityLevel>` with `NotAssessed` changes the data shape but preserves that operation
exactly. `NotAssessed` is expressly public, has no provenance, and is constraint-free. A caller
that has assessed drift can therefore assert it has not assessed and reach `Autopilot`. The raw
facts themselves are public caller assertions as well: an `Observed` value containing `observed`
in `known_signatures` is structurally valid and reads as recognized, regardless of a real
Guardian result the caller may be holding.

The two production `cancellai-policy` sites' explicit `NotAssessed` values are locally honest:
neither has a provider-root observation at its scope. That judgment is unchanged from round 3;
the new spelling is clearer than `None`, but it neither makes the missing observation available
nor prevents a caller that has one from selecting `NotAssessed`. Thus ADR-0035's stated residual
is not merely the accepted future-orchestrator gap found in E12/E13 or release-channel wiring.
It repeats the very narrower claim ADR-0035 says it closes: a real assessment can still be
discarded before the public authority construction.

The direct `compute_effective_authority` probe is an additional bypass surface, but the ADR's
scoping rationale is defensible for that primitive alone: it is explicitly documented as a
generic named-constraint calculator and restricting its public surface would be a broader
Effective Authority redesign. It cannot rescue E14-S04, however, because the direct
`effective_authority` reproduction already violates AC1/SI-004. The story cannot claim a
mandatory authority-construction boundary while the public, supposedly mandatory input remains
freely replaceable at a separate construction.

## Required owner disposition — no fifth patch cycle

ADR-0035 fails for the same structural reason as ADR-0034: the actual authority call is not bound
to an assessment of the provider root it governs. Moving the conclusion into the safety crate
does not create that binding while callers may freely choose `NotAssessed` or fabricate raw facts.
This is not repairable by another E14-S04 field/test adjustment within the current approach.

A further owner decision must choose a different authority boundary: the real authority resolver
must own or require an identity/scope-bound, non-discardable assessment for the provider root it
resolves, with the destructive execution path consuming that bound result. It must also decide
the appropriate public-surface treatment for generic authority calculation. No fifth
patch-and-repeat cycle on ADR-0035 is authorized by this review.

## CR4 Safety Verdict — E14-S04

## Verdict

`FAIL`

The CR4 classification remains correct. The reviewed diff changes
`rust/crates/cancellai-safety/src/authority.rs` and its provider-capability constraint at the
safety-kernel authority boundary; it purports to constrain destructive authority and is covered
by the CR4 floor. A caller holding a real layout-drift observation can still obtain `Autopilot`,
violating SI-004, C-02, C-05, and TM-05. AC1 is not met.

## Owner decision

E14-S04 is returned to `in_progress`; E14 remains `in_progress` and cannot close or coordinate
PD-021 release evidence. The owner must decide the next, structurally different authority-boundary
design. No release action is appropriate while this CR4 verdict is FAIL.

## Gate status

| Command | Result |
| --- | --- |
| Temporary `cargo test -p cancellai-guardian --test e14_round4_independent -- --nocapture` | PASS — reproduced three public API bypass surfaces; temporary source removed. |
| `python3 scripts/project_os.py check` | PASS before review-state update. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS |
| `cd rust && cargo deny check` | PASS after an approved rerun outside the sandbox could acquire Cargo's read-only advisory lock; existing warnings only. |
| `python3 -m pytest tests -v` | PASS — 668 tests, 585 subtests. |
| AGENTS.md Python checker list | PASS for all repository checkers. Ruff check, Ruff format, and Mypy could not run: the active `python3` has none of those modules installed. |
| `gh run list --branch main --limit 5` | UNKNOWN — this session could not connect to `api.github.com`; unknown is not treated as green. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E14.json`
- `docs/adrs/0035-provider-layout-authority-is-derived-from-raw-observation-not-a-caller-supplied-ceiling.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/DOMAIN_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `project/evidence/E14-S04/VERIFIER_BRIEF.md`
- `project/evidence/E14-S04/EVIDENCE.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND3.md`
- `scripts/check_process.py`
- changed Guardian, policy, and safety authority sources and their public consumers
