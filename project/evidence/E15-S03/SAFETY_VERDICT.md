# Safety Verdict - E15-S03

- Change: bounded Guardian remediation planner
- Risk: CR4
- Review target: `b9409e7..dae7c32`
- Independent verifier: Codex (GPT-5)
- Date: 2026-09-22

## Verdict

`FAIL`

## Safety surface changed

`plan_remediation` turns pressure and an asserted candidate authority into a Guardian plan that
can mark an item `Quarantine`. This is an authority boundary: it must preserve the Effective
Policy that the shared engine computed for the actual artifact at action time.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-027 | Detection severity does not create authority. | `pressure_authority_ceiling` is bounded at `Quarantine` and the 20-cell matrix confirms `min` for values handed to the function. However, an external caller can construct a `RemediationCandidate` directly with `reachable_authority: Autopilot` for an otherwise unrepresented unknown/protected artifact, then RED produces `Quarantine`. | FAIL |
| SI-028 | Guardian actions are no stronger than the shared engine's actual Effective Policy. | `RemediationCandidate` exposes public `artifact_id`, `reachable_authority`, and `binding_constraints`; `plan_remediation` accepts this forgeable carrier rather than a `ClassifiedArtifact` or opaque shared-engine permit. The temporary external integration probe compiled and produced `ActionClass::Quarantine`/`AuthorityLevel::Quarantine` from a fabricated candidate without constructing a `ClassifiedArtifact`. | FAIL |

## Adversarial cases

- Exhaustively ran all four pressure states by all five `AuthorityLevel` values. For a trusted
  input, the planner grants exactly `min(pressure_authority_ceiling, reachable_authority)` and
  never emits `Delete`, `Archive`, or `Restore`; `pressure_authority_ceiling` never exceeds
  `Quarantine`.
- Compiled and ran an external temporary integration test that directly constructed public
  `RemediationCandidate { artifact_id: "unknown-artifact", reachable_authority: Autopilot,
  binding_constraints: vec![] }`. `plan_remediation(PressureState::Red, ...)` returned a
  `Quarantine` item at `Quarantine` authority. The probe was removed after reproduction.
- Inspected all production references in `remediation.rs`: the only local authority operation is
  `std::cmp::min`, and the module does not seal or execute a plan. The defect is provenance, not
  the ordering of `AuthorityLevel` or a Delete path.

## Differential / compatibility evidence

- No Python-reference behavior is changed by this new Guardian-only API.
- `cargo fmt --check`, workspace clippy, and workspace check passed. The full workspace test
  gate failed separately in E15-S01's macOS service smoke test.

## Known residual risks

- The documented plan/audit flow still has no live orchestrator or sealed plan. That is not the
  rejection: the public planner can already label a forged candidate as pre-authorized
  `Quarantine`, contrary to its CR4 authority contract.

## Rollback / recovery

Do not close E15-S03 or expose this planner to an orchestrator. Repair its public input boundary
so callers cannot supply a fabricated authority ceiling: accept the actual classified artifact
at the planning boundary, or introduce an opaque candidate/permit constructible only by the
shared classification pipeline. Add an external-crate compile/runtime regression proving an
unknown/protected artifact cannot be forged into a quarantine candidate, then repeat the complete
pressure x policy x artifact matrix and independent review.

## Owner decision

`REJECT`

Owner note: Round 1 found an authority-provenance bypass. A repaired Round 2 is required; it is
the owner-capped final review round for E15.
