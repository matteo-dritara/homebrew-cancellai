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

E02-S01 created this skeleton at `rust/crates/` (workspace root `rust/Cargo.toml`, not the
repository root - see [ADR-0015](../adrs/0015-rust-workspace-toolchain-and-repository-layout.md)
for the toolchain/edition/MSRV/`unsafe`/CI/license decisions that apply to every crate here).
Every crate below exists today as a documented skeleton with the dependency edges shown;
none has real domain logic yet.

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
  cancellai-sealedfs/         # unsafe-isolated no-follow/handle-relative root capability
  cancellai-cli/              # headless/scriptable client
  cancellai-tui/              # terminal experience
  cancellai-guardian/         # later user-service runtime
```

Forbidden dependency direction:

- model/safety may not depend on UI/provider implementations;
- provider adapters may not bypass the safety executor;
- UI crates may not access raw provider roots for mutation;
- network/knowledge code may not receive direct mutation authority.

`cancellai-safety` may depend on `cancellai-platform` (not just `cancellai-model`) - domain
and policy code consuming an OS capability's *result* is not the same as bypassing the safety
executor (`docs/architecture/PLATFORM_MODEL.md`); `scripts/check_rust_workspace.py`'s
isolation check reflects this per-crate, not a blanket "model/safety depend on nothing but
each other" (E03-S02).

E03-S05 implements "provider adapters may not bypass the safety executor" for filesystem
deletion specifically, and statically: `rust/crates/cancellai-platform/src/mutation.rs` is
the *only* production source file in the workspace allowed to call
`std::fs::remove_file`/`remove_dir`/`remove_dir_all` directly (SI-019) -
`scripts/check_mutation_boundary.py` enforces this by scanning every other crate's
production source for those calls and fails if it finds one. `rust/crates/cancellai-safety/src/mutation_executor.rs`'s
`execute`/`execute_all` are the one production call path from a `SealedPlan` (E03-S02) to
that capability: verify the plan's root matches the target's bound root, verify authority/
reversibility actually permit the action class, revalidate identity immediately before
mutation (SI-013, `cancellai-safety`'s own `revalidate`), then delegate to `MutationExecutor`.
`execute_all` aggregates a batch via `Vec::map`/`collect`, which cannot silently drop or
short-circuit past a result the way a hand-written loop with an early return could (SI-020's
per-action explicitness).

E03 verifier review round 1 found the raw `MutationExecutor` capability itself (not merely
the bare `std::fs::remove_file` primitive) was reachable from any crate - `pub`, re-exported
at `cancellai_platform`'s crate root, directly callable with an unconstrained raw path,
bypassing every check the paragraph above describes. `scripts/check_mutation_boundary.py` was
extended to also forbid referencing `SystemMutationExecutor` or calling `.mutate(` anywhere
outside `mutation.rs` and `mutation_executor.rs`, and `cancellai_platform`'s crate root no
longer re-exports `SystemMutationExecutor` at all. `MutationExecutor::mutate` itself was also
strengthened (repaired in the same round): it now takes the plan's expected `IdentityToken`
and, for a plain file, confirms it via an open file descriptor both immediately before and
immediately after the actual unlink syscall - narrowing, though (a safe-Rust, no-`unsafe`,
no-new-dependency implementation cannot fully close) not perfectly eliminating, the race
between revalidation and the OS call itself. Directories and symlinks are refused rather than
deleted without that confirmation.

E07-S07's round-1 independent verifier review found the identical *shape* of race one layer
up, in `cancellai-cli`'s `configure` command (which does not go through `ApprovedRoot`/
`MutationExecutor` at all - see `docs/architecture/PLATFORM_MODEL.md`'s "Default-root
authority never rests on a lexical name alone" for why): a root confirmed not to be a symlink,
then read/written/renamed by raw path, could be atomically replaced with a symlink in the gap
between that check and the first path-based operation, redirecting every following read/write
outside the approved root. Unlike the `MutationExecutor` case above, this one *is* fully
closed, not merely narrowed: `cancellai-sealedfs` (ADR-0017) opens the root exactly once with
`O_NOFOLLOW` and performs every subsequent child operation via `openat`/`renameat` against that
one retained descriptor, which the kernel resolves independently of whatever the original path
now names - a rename/symlink-swap of the root's own path after that point cannot redirect
anything. This needed the `unsafe` FFI `MutationExecutor`'s own docs describe wanting and
explicitly did not have; ADR-0015 anticipated exactly this ("a future crate ... isolated in a
small, dedicated crate whose only job is that unsafe boundary") without naming it in advance -
`cancellai-sealedfs` is that crate, the only one in the workspace not carrying
`unsafe_code = "forbid"`, and does not participate in the `cancellai-safety`/`cancellai-
platform` mutation boundary above (`configure` is a vendor-settings write, not a
cancellAI-tracked artifact deletion, per SI-019's own scope). `MutationExecutor`'s own
narrower, unlink-specific race remains open and is unrelated to this fix (a different
operation, a different crate) - see that module's docs for its own residual.

E04-S01/E04-S02/E04-S03 implement `cancellai-inventory`'s share of the OBSERVE stage below:
`FileFacts`/`observe_file_facts` (per-path evidence composed from three `cancellai-platform`
seams), `scan_scope` (one recursive walk per scope, never re-walked by its report views), and
`derive_completeness`/`planning_view` (scope-level `Complete`/`Partial`/`Unknown`
classification that a planning-facing view cannot be handed without). See
[`DOMAIN_MODEL.md`](DOMAIN_MODEL.md#filefacts-the-observe-stage-evidence-agentartifact-is-built-from)
for the full account; this crate still has no `AgentArtifact`/classification logic of its
own - that is CLASSIFY-stage scope (E05/E06), not this epic's.

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
