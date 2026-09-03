# Documentation Map

cancellAI treats documentation as part of the executable product system. This page is the canonical entrypoint for humans and coding agents.

## Start here

1. [PRODUCT.md](PRODUCT.md) - product vision, users, positioning, boundaries, and value sequence.
2. [CONSTITUTION.md](CONSTITUTION.md) - non-negotiable product and safety principles.
3. [DECISION_REGISTER.md](DECISION_REGISTER.md) - the accepted product decisions that created the target direction.
4. [ROADMAP.md](ROADMAP.md) - capability roadmap generated from the machine-readable project control plane.
5. [BACKLOG.md](BACKLOG.md) - complete epic/story contracts with acceptance and verification requirements.

## Architecture

- [ARCHITECTURE.md](ARCHITECTURE.md) - architecture index and transition state.
- [architecture/AS_IS.md](architecture/AS_IS.md) - current Python v1 architecture and verified defects.
- [architecture/TARGET.md](architecture/TARGET.md) - target Rust/control-plane architecture.
- [architecture/DOMAIN_MODEL.md](architecture/DOMAIN_MODEL.md) - AgentArtifact, evidence, lifecycle, plans, authority.
- [architecture/JSON_CONTRACTS.md](architecture/JSON_CONTRACTS.md) - versioned inventory/plan/explanation/result document schemas.
- [architecture/PROVIDER_MODEL.md](architecture/PROVIDER_MODEL.md) - provider capabilities, manifests, adapters, trust.
- [architecture/PLATFORM_MODEL.md](architecture/PLATFORM_MODEL.md) - macOS/Linux/Windows/WSL safety abstractions.
- [architecture/PERSISTENCE_MODEL.md](architecture/PERSISTENCE_MODEL.md) - local state, event ledger, analytical memory, quarantine.
- [architecture/POLICY_MODEL.md](architecture/POLICY_MODEL.md) - policy hierarchy and deterministic constraint resolution.
- [architecture/GUARDIAN_MODEL.md](architecture/GUARDIAN_MODEL.md) - predictive signals, pressure states, bounded remediation.
- [PLATFORMS.md](PLATFORMS.md) - platform support levels and what "supported" is allowed to mean.
- [PROVIDERS.md](PROVIDERS.md) - provider compatibility posture and capability scoping.

## Security and trust

- [security/THREAT_MODEL.md](security/THREAT_MODEL.md) - assets, actors, trust boundaries, misuse cases, mitigations.
- [security/SAFETY_INVARIANTS.md](security/SAFETY_INVARIANTS.md) - constitutional runtime invariants with stable IDs.
- [security/SUPPLY_CHAIN.md](security/SUPPLY_CHAIN.md) - release provenance, SBOM, signing/attestation, knowledge trust.
- [security/INCIDENT_RESPONSE.md](security/INCIDENT_RESPONSE.md) - containment, recovery, kill-switch hierarchy, and post-incident obligations.

## Engineering system

- [development/ENGINEERING_SYSTEM.md](development/ENGINEERING_SYSTEM.md) - cancellAI Engineering Operating System (cEOS).
- [development/WORK_ITEM_MODEL.md](development/WORK_ITEM_MODEL.md) - epic/story/spec/evidence lifecycle and Change Risk Levels.
- [development/AGENT_PROTOCOL.md](development/AGENT_PROTOCOL.md) - Claude/Codex executor-verifier protocol.
- [development/VERIFICATION_STRATEGY.md](development/VERIFICATION_STRATEGY.md) - test pyramid, adversarial and differential verification.
- [development/MIGRATION_PYTHON_RUST.md](development/MIGRATION_PYTHON_RUST.md) - spec-first migration sequence and cutover gates.
- [development/RELEASE_GATES.md](development/RELEASE_GATES.md) - Definition of Done/Safe and release gate matrix.
- [development/REPOSITORY_GOVERNANCE.md](development/REPOSITORY_GOVERNANCE.md) - GitHub rulesets, Actions permissions, tag/release controls, and settings drift policy.
- [RELEASING.md](RELEASING.md) - the release runbook and versioning scheme.
- [CLI.md](CLI.md) - generated command reference for the current Python CLI.
- [CLI_RUST.md](CLI_RUST.md) - hand-maintained command reference for the target-engine Rust CLI (beta, E06).

## Evidence, research, and decisions

- [audits/2026-08-27-CODE_REVIEW.md](audits/2026-08-27-CODE_REVIEW.md) - baseline code review and P0 findings.
- [audits/2026-09-03-CODE_REVIEW.md](audits/2026-09-03-CODE_REVIEW.md) - target-engine review: scan-completeness authority, gate integrity, and the CR-TE findings carried by E21/E22.
- [research/MARKET_AND_STANDARDS_2026-08.md](research/MARKET_AND_STANDARDS_2026-08.md) - market/standards research used to shape the system.
- [adrs/](adrs/) - architecture decision records. Accepted ADRs are never deleted; superseded ADRs link forward.
- [rfcs/README.md](rfcs/README.md) - when a design change requires an RFC before implementation.

## Machine-readable control plane

The source of truth for project planning lives under [`project/`](../project/):

- `project/decisions.json` - product decisions.
- `project/roadmap.json` - phases and exit gates.
- `project/epics/*.json` - story contracts.
- [`project/generated/PROJECT_STATUS.md`](../project/generated/PROJECT_STATUS.md) - generated project summary.
- `project/templates/` - see [the template index](../project/README.md#templates).

Run:

```sh
python3 scripts/project_os.py check
python3 scripts/project_os.py status
python3 scripts/project_os.py next
python3 scripts/project_os.py brief E00-S01 --role executor
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

Generated Markdown (`DECISION_REGISTER.md`, `ROADMAP.md`, `BACKLOG.md`, `PROJECT_STATUS.md`) must never be edited manually.
