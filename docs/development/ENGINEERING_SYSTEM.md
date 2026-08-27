# cancellAI Engineering Operating System (cEOS)

cEOS is the project's development framework. It combines the parts of modern corporate software delivery that matter for cancellAI with a risk model appropriate to destructive local software.

It is deliberately not Scrum-by-template. Roadmap, work, verification, evidence, and release authority are one connected system.

## Design influences

- NIST SSDF: secure practices integrated into the SDLC rather than appended at release.
- SLSA/OpenSSF/CNCF software-factory guidance: provenance, policy, verification, automation, and supply-chain evidence.
- trunk-based development/small changes: short-lived branches and reviewable units.
- ADR/RFC practice: significant decisions are small, durable, version-controlled records.
- policy-as-code architecture: separate decision from enforcement and use structured inputs/outputs.
- safety engineering: distinguish functional success from safe release eligibility.

These are inputs, not compliance theater. cEOS is the project-specific implementation.

## Operating-system model

```text
+---------------------------------------------------------+
| CONSTITUTION LAYER                                      |
| Product Constitution + Safety Invariants + Threat Model |
+----------------------------+----------------------------+
                             |
+----------------------------v----------------------------+
| CONTROL PLANE                                           |
| Decisions -> Roadmap -> Epics -> Stories -> Specs       |
| project/*.json is machine-readable source of truth      |
+----------------------------+----------------------------+
                             |
+----------------------------v----------------------------+
| EXECUTION PLANE                                         |
| Executor agent/human implements one small work item     |
+----------------------------+----------------------------+
                             |
+----------------------------v----------------------------+
| VERIFICATION PLANE                                      |
| Independent verifier + automated/adversarial evidence   |
+----------------------------+----------------------------+
                             |
+----------------------------v----------------------------+
| EVIDENCE LEDGER                                         |
| Work summary + tests + Safety Verdict + CI provenance   |
+----------------------------+----------------------------+
                             |
+----------------------------v----------------------------+
| RELEASE FACTORY                                         |
| Functional + Safety + Compatibility + Operability gates |
+---------------------------------------------------------+
```

## The work loop

1. **Orient** - read current phase, story, dependencies, architecture, relevant invariants/threats.
2. **Specify** - confirm outcome/AC/safety obligations and write an RFC/ADR only if the story requires a significant new decision.
3. **Plan verification first** - define what would falsify the implementation before coding.
4. **Execute small** - one focused change; avoid unrelated refactors.
5. **Verify independently** - verifier attacks the result from spec and counterexamples.
6. **Reconcile evidence** - tests, docs, compatibility and residual risks become an evidence packet.
7. **Gate** - Definition of Done and Definition of Safe must both pass.
8. **Merge** - short-lived branch/squash PR to `main`; main remains releasable for the current phase.
9. **Release** - only when required phase/release gates and evidence are satisfied.

## Planning hierarchy

```text
Product Constitution
  -> Product Decision (PD-xxx)
    -> ADR/RFC when architecture/design is significant
      -> Roadmap Phase (P#)
        -> Epic (E##)
          -> Story (E##-S##)
            -> Implementation PR/commit(s)
              -> Evidence Packet
                -> Release Evidence
```

Every layer links downward/upward. No orphan feature work.

## Change Risk Levels

Risk controls scale with the authority of the change, not with line count. See `WORK_ITEM_MODEL.md` for exact gates.

- CR0: documentation/metadata only.
- CR1: observational/non-mutating behavior.
- CR2: classification/planning/state semantics.
- CR3: reversible or conditionally mutating behavior.
- CR4: irreversible mutation, safety kernel, authority/trust/supply-chain boundaries.

## Branch and change strategy

Default: trunk-based development with short-lived branches and small PRs. A story can require multiple PRs if each preserves a coherent, releasable state. Avoid long feature branches.

Rules:

- behavior change and large refactor should usually be separate;
- tests travel with the behavior they verify;
- migrations are additive/dual-read or otherwise rollbackable until cutover;
- hidden feature flags are used only if they reduce integration risk without creating untested code paths;
- main must not depend on a future PR to become safe.

## Automation principle

Anything deterministic and repeatable should be automated:

- project-model validation and generated documentation drift;
- code format/lint/type/test;
- fixture/schema validation;
- security/dependency scans;
- differential tests;
- compatibility matrices;
- SBOM/provenance/release manifests;
- install smoke tests;
- evidence completeness checks.

Human/owner attention is reserved for decisions, residual risk, product tradeoffs, and CR4 Safety Verdict acceptance.

## Definition of Ready

A story is Ready when:

- dependencies are satisfied or explicitly parallel-safe;
- outcome is understandable without hidden conversation context;
- AC are testable;
- Change Risk Level is assigned;
- safety obligations/threat references exist when relevant;
- verification approach is feasible;
- architecture ambiguity that would change public contracts has an ADR/RFC path.

## Definition of Done vs Definition of Safe

Done answers: "Does it work and is it integrated/documented?"

Safe answers: "Can it violate a constitutional invariant, lose data unexpectedly, misrepresent uncertainty, or create an unverified authority path?"

A story with Done=YES, Safe=NO is not merge/release eligible at the required gate.

## Ownership and transparency

The owner does not need to inspect every implementation detail to retain control. cEOS keeps owner-visible artifacts stable:

- product decisions;
- roadmap/status;
- story contract;
- architecture decision;
- evidence summary;
- Safety Verdict for CR4;
- release gate summary.

No agent is allowed to silently redefine these to make implementation easier.
