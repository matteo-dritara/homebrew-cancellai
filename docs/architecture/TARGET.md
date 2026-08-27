# Architecture: Target

## Architectural style

cancellAI combines a local control plane with a constrained mutation kernel. It borrows useful patterns from operating systems and distributed control planes without pretending the workstation is a cluster.

- **Model plane**: artifact facts, evidence, lifecycle, risk, authority.
- **Decision plane**: provider mapping, policy compilation, planning, explanations.
- **Safety/mutation plane**: the only authority allowed to mutate provider state.
- **Experience plane**: CLI, TUI, Guardian, Desktop.
- **Knowledge plane**: signed/versioned provider intelligence; informative, never directly destructive.

```text
                    +----------------------+
                    |  CLI / TUI / Guardian|
                    +----------+-----------+
                               |
                               v
                    +----------------------+
                    | Engine / Query API   |
                    +----------+-----------+
                               |
             +-----------------+-----------------+
             |                                   |
             v                                   v
 +-------------------------+         +------------------------+
 | Inventory + Providers   |         | Policy + Explanation   |
 +------------+------------+         +-----------+------------+
              |                                  |
              +----------------+-----------------+
                               v
                    +----------------------+
                    | Sealed Plan Builder  |
                    +----------+-----------+
                               v
                    +----------------------+
                    |   SAFETY KERNEL      |
                    | revalidate + mutate  |
                    +----------+-----------+
                               |
                               v
                        local filesystem /
                        vendor-native API
```

## Target Rust workspace

Names may be refined through ADRs, but dependency direction is normative.

```text
crates/
  cancellai-model/            # pure domain types and invariants
  cancellai-safety/           # authority lattice, root capabilities, sealed plans
  cancellai-inventory/        # filesystem observations and scan completeness
  cancellai-provider-api/     # provider capability contract and manifest model
  cancellai-provider-claude/  # Claude adapter
  cancellai-provider-codex/   # Codex adapter
  cancellai-policy/           # typed policy and deterministic resolver
  cancellai-store/            # SQLite current state, ledger, analytical rollups
  cancellai-platform/         # OS capability interfaces and implementations
  cancellai-cli/              # headless/scriptable client
  cancellai-tui/              # terminal experience
  cancellai-guardian/         # later user-service runtime
```

Forbidden dependency direction:

- model/safety may not depend on UI/provider implementations;
- provider adapters may not bypass the safety executor;
- UI crates may not access raw provider roots for mutation;
- network/knowledge code may not receive direct mutation authority.

## Core loop

The engine behaves as an evidence-driven reconciliation loop:

```text
OBSERVE
  inventory provider/filesystem facts
      |
      v
CLASSIFY
  map facts to AgentArtifacts + evidence/confidence
      |
      v
RESOLVE
  lifecycle + policy + trust + authority ceilings
      |
      v
PLAN
  immutable actions + preconditions + explanations
      |
      v
REVALIDATE
  identity/activity/root/provider capability
      |
      v
EXECUTE
  reversible first; irreversible only when authorized
      |
      v
RECONCILE
  re-observe outcome + ledger event + metrics
```

Execution never trusts the original observation blindly.

## No hidden AI authority

Machine-learning or LLM features may eventually improve explanation, anomaly summarization, or research. They are not part of the authority path. Destructive eligibility is deterministic and reproducible from structured evidence/policy.

## Data flow boundaries

### Filesystem/provider data

Raw contents remain at the provider/filesystem edge. The core receives metadata/facts unless an adapter explicitly requires parsing a small structured provider metadata file.

### Persistent local data

The store persists contentless identity/lifecycle/policy/audit facts. Current state is rebuildable.

### Network knowledge

Signed provider/layout knowledge can enter through a verification boundary. Invalid or unknown-trust knowledge is ignored or inspection-only.

## Repository evolution

The current repository name `homebrew-cancellai` reflects its origin as a Homebrew-first CLI. Once Rust becomes canonical and cross-platform release automation exists, the preferred topology is:

- canonical source repository: `cancellai`;
- generated/dedicated Homebrew tap: `homebrew-cancellai`.

Do not perform this repository split during P0. It is a packaging/repository migration after canonical Rust cutover planning, with redirects and release continuity documented.
