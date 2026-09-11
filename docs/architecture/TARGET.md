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

Scope completeness is a **shared type, not a shared traversal** (E21-S04,
[ADR-0018](../adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md)). The
provider adapters keep their own layout-specific walks - Claude's flat project/session shape and
Codex's date-partitioned rollout tree are different problems - but every one of them must return
`cancellai-inventory`'s `ScopeCompleteness`, and `cancellai-policy` can only obtain planning
candidates through a value that carries it. `scan_scope` remains the reference implementation of
that model rather than the shipped traversal; `scripts/check_rust_workspace.py` fails if
`cancellai-cli` stops being able to reach the crate at all, because a crate nothing ships is a
crate whose guarantees nothing has.

E04-S01/E04-S02/E04-S03 implement `cancellai-inventory`'s share of the OBSERVE stage below:
`FileFacts`/`observe_file_facts` (per-path evidence composed from three `cancellai-platform`
seams), `scan_scope` (one recursive walk per scope, never re-walked by its report views), and
`derive_completeness`/`planning_view` (scope-level `Complete`/`Partial`/`Unknown`
classification that a planning-facing view cannot be handed without). See
[`DOMAIN_MODEL.md`](DOMAIN_MODEL.md#filefacts-the-observe-stage-evidence-agentartifact-is-built-from)
for the full account; this crate still has no `AgentArtifact`/classification logic of its
own - that is CLASSIFY-stage scope (E05/E06), not this epic's.

### Engine / Query API (E08-S04)

The target diagram's "Engine / Query API" box, above "Inventory + Providers" and "Policy +
Explanation," gets its first real occupant here: `cancellai-policy::views` groups one classified
inventory (`&[ClassifiedArtifact]`) into machine/project/provider/artifact/session views. No
dedicated crate backs the box yet - `cancellai-cli` previously assembled `status`/`inspect`
output ad hoc, per command, with no shared grouping logic between them - so `views` lives in
`cancellai-policy`, the crate directly beneath the box that already produces the
`ClassifiedArtifact` collections it reads. A dedicated crate split is a future, separately
reviewed architectural step if the query surface grows enough to need its own dependency
boundary.

Every view is a grouping of borrowed references (`Vec<&ClassifiedArtifact>`), never a clone of
the underlying `AgentArtifact` data, mirroring `retention::ProviderPlanningView`'s own
`'a`-borrowing pattern - and every grouping function partitions its input exactly (proven
generically, not by per-function inspection, by `views::tests::
every_dimension_reconciles_to_the_same_total`, which compares *counted* id occurrences rather
than a `BTreeSet`, so a duplicate or dropped source row is observable rather than silently
absorbed into a set). `by_project` carries an explicit `Unattributed` bucket rather than
dropping or silently merging E08-S02's `None` attribution. `by_session` walks a `RelationshipKind::
ChildOf` chain to its ultimate root (E08-S01), not merely one edge - round-1 independent
verifier review (`project/evidence/E08-VERIFIER-REVIEW.md`) found the first version followed
only the immediate parent, splitting a three-level Codex tree into one bucket per generation,
and trusted a `ChildOf` target that was absent from the given slice as the bucket key verbatim.
`session_root` now walks the full chain against an index of the slice actually supplied,
mirroring `cancellai_provider_codex::graph::root_id_for`'s own fail-safe rules: a target absent
from that index, an artifact with no `ChildOf` relationship, or a cycle all isolate the walk at
its current position rather than looping or propagating an id nothing in the view can back up.
Claude's own sessions are already flat, so its session view coincides with the degenerate
`by_artifact` view. `by_machine` is honestly a single bucket today - `MachineId` does not exist
on `AgentArtifact` because E18/E19's remote-target work has not landed, and every artifact a
single-machine build observes is definitionally on the one machine running it.

### Atlas TUI shell (E09-S01)

The target diagram's "CLI / TUI / Guardian" experience-plane box gets its first real TUI
occupant: `rust/crates/cancellai-tui` is now a keyboard-first navigation shell built on
`ratatui`/`crossterm` (outer-ring dependencies pre-approved for this epic by name in
[ADR-0019](../adrs/0019-dependency-rings-per-crate.md)), rather than the E02-S01 skeleton that
printed a placeholder and declared unused `cancellai-inventory`/`cancellai-platform`/
`cancellai-provider-claude`/`cancellai-provider-codex`/`cancellai-store` dependencies.

**AC1 - no direct filesystem/provider access from the TUI crate**: `cancellai-tui`'s
`Cargo.toml` now depends on `ratatui`/`crossterm` only - zero `cancellai-*` crates. This story
has no data source to render yet (E09-S02's own verification plan is "golden view models from
fixture inventory" - that wiring is its job), so adding `cancellai-policy` now, with nothing
real to call, would repeat the "dependency ahead of the evidence that would justify it" pattern
this codebase otherwise avoids (e.g. `by_machine` above). E09-S02 reintroduces
`cancellai-policy` once it has real fixture-inventory view data for the Atlas screen this story
only stubs.

**Crate layout**: a library (`app`, `capability`, `ui`, `event`) plus a thin `main.rs` binary -
a deliberate departure from `cancellai-cli`'s no-lib, spawn-the-real-binary test style
(`tests/cli_behavior.rs`), because a TUI's raw-mode terminal initialization cannot run
headlessly in CI at all. `app::App::handle_key` is a pure reducer over `Screen` (`Home`,
`Atlas`, `Explain`, `Plan` - the latter three are `E09-S02`/`E09-S03`/`E09-S04`'s screens,
rendered here only as "Coming in E09-S0X" placeholders, so AC2 - "all displayed actions derive
from engine plans" - holds vacuously until those stories add real, plan-derived content).
`ui::draw` is a pure `ratatui` render function, tested against `ratatui::backend::TestBackend`
rather than a real terminal.

**AC3 - tier-1 terminals with graceful capability fallback**: `docs/PLATFORMS.md`'s "tier-1" is
an OS-platform concept, not a terminal-emulator one, so this story defines the axis it actually
needs. `capability::detect` reads `NO_COLOR`/`TERM`/`COLORTERM` for a three-tier `ColorSupport`
(`None`/`Basic`/`Extended`) and `LANG`/`LC_ALL`/`LC_CTYPE` (plus an explicit
`CANCELLAI_TUI_ASCII` escape hatch) for Unicode-vs-ASCII box-drawing, always degrading to the
weaker capability on a missing or unrecognized signal rather than assuming the best case.
`ui::draw` also refuses to lay out a frame narrower than 24 columns or shorter than 6 rows,
rendering a plain "terminal too small" message instead of panicking.

**Manual accessibility checklist** (verification calls for "snapshot/render tests plus manual
accessibility checklist" - the latter cannot be automated): every screen is reachable with the
keyboard alone (`Tab`/`Shift+Tab`/`1`-`4`); `NO_COLOR=1` emits no color escape codes;
`CANCELLAI_TUI_ASCII=1` emits no Unicode box-drawing glyphs; the terminal's raw mode and
alternate screen are restored on both a clean quit and a panic (`main.rs`'s panic hook runs
`restore_terminal` before the default handler); a very small terminal shows the fallback
message rather than panicking. Executed once this story via a real macOS terminal
(`project/evidence/E09-S01/EVIDENCE.md`); Linux/Windows terminal verification is a documented
residual for a follow-up manual pass, the same per-platform honesty `docs/PLATFORMS.md` already
uses.

### Machine and project atlas (E09-S02)

`cancellai_policy::atlas` is the second occupant of the "Engine / Query API" layer, after
`views` (E08-S04). `atlas::summarize(&[ProviderResolution], top_n)` turns already-classified,
already-completeness-aware `ProviderResolution`s into an `AtlasSummary`: total logical footprint
and a *separately-fielded* estimated-reclaimable subset, per-provider and per-project totals
(the latter carrying `views::ProjectBucketKey`'s own explicit `Unattributed` case), and the
`top_n` individually largest artifacts across every provider.

**AC1 - logical and reclaimable are visually distinct**: `AtlasSummary::total_reclaimable_bytes`
is never merged into `total_logical_bytes` - they are two separate `u64` fields a caller cannot
blend even by accident. "Reclaimable" is not a new predicate invented for this screen: an
artifact counts once its `ClassifiedArtifact::reachable_authority` clears
`cancellai_safety::authority::minimum_authority_for(ActionClass::Delete)` - the identical test
`cancellai-cli plan`/`clean` already apply, so a byte counted here as reclaimable is a byte a
real `plan` would also propose deleting today. `cancellai-tui`'s `ui::draw_atlas_summary` labels
the two totals with different text ("Total footprint" / "Estimated reclaimable") unconditionally
and adds distinct styling only where color is available - distinctness never depends on color
capability alone.

**AC2 - unknown/incomplete scans are prominent, never hidden**: `AtlasSummary::any_incomplete`
is true the moment any provider's scan is not `Complete`, but `summarize` still sums whatever
*was* observed into the totals above - incompleteness flags a possible undercount, it never
zeroes or withholds the numbers. The TUI renders a dedicated highlighted line
("scans are incomplete - totals may be undercounted") whenever the flag is set.

**Dependency boundary**: `cancellai-tui`'s `Cargo.toml` gains exactly one new production
dependency, `cancellai-policy` - still no provider adapter, `cancellai-inventory`,
`cancellai-platform`, or `cancellai-safety` directly (AC1 from E09-S01 continues to hold; see
that section and `Cargo.toml`'s own comment for why depending on the query-API crate is the
boundary working as designed, not a gap in it). `data::EngineData` is the one seam the TUI reads
`cancellai-policy` types through, so a future screen (E09-S03/E09-S04) extends that one struct
rather than importing engine types in every screen-drawing function.

**Verification ("golden view models from fixture inventory")**: `atlas::summarize`'s own tests
live in `cancellai-policy` (crate-internal, via a `#[cfg(test)]`-only `ProviderResolution::
for_test` constructor - its `artifacts`/`observation` fields are private by E21-S04/ADR-0018
design) against synthetic multi-provider, multi-project, Partial/Unknown-completeness fixtures.
`cancellai-tui`'s own render tests construct an `AtlasSummary` fixture directly (all its fields
are `pub`) and assert the rendered text distinguishes the two totals and surfaces incompleteness,
without needing a real scan.

**Residual**: wiring a real, live provider scan into the running `cancellai-tui` binary is
deferred - `main.rs` renders the Atlas screen's explicit "No inventory scan loaded yet." state,
matching the identical, already-accepted deferral E08-S04 recorded for `cancellai-cli`'s own
view wiring. This story's contract is fixture-driven view-model correctness, not live-scan
product wiring.

### Artifact explain view (E09-S03)

`cancellai_policy::explain` is the third occupant of the "Engine / Query API" layer, after
`views` (E08-S04) and `atlas` (E09-S02). `explain(classified, actions) -> ExplainView` turns one
classified artifact plus the plan's own `Action` list into the outcome's six named facets - why
it exists (project attribution and structural relationships), classification, evidence, risk,
reversibility, and allowed authority - plus the concrete policy outcome.

**AC1 - every destructive recommendation has a human-readable explanation path**: this is not a
second explanation mechanism. `retention::build_actions` already produces exactly one `Action`
per artifact, every time, carrying a `reason: String` that SI-007 requires is never silently
omitted. `explain` finds the artifact's own `Action` by id and surfaces its `reason` verbatim -
the same sentence a `plan` document would show. `PolicyOutcome` distinguishes three cases so a
caller can never conflate them: `Recommended { action_class, reason }` for a destructive/mutating
class, `ObservationOnly { reason }` for `Observe` (still a real, non-fabricated reason), and
`NotEvaluated` when the given `actions` slice simply does not cover this artifact (e.g. a
filtered plan) - distinct from "policy looked and chose not to act."

**AC2 - low-confidence data is visibly differentiated**: `is_low_confidence` returns `true` for
every `KnowledgeConfidence` tier except `Verified`, mirroring
`cancellai_safety::authority::confidence_ceiling`'s own distinction (`Verified` is the sole tier
reaching the top authority ceiling). Applied independently to an artifact's own
`knowledge_confidence` and to its `AttributedProject::confidence` - one can be low while the
other is not, and `cancellai-tui`'s `ui::confidence_span` renders a `[low confidence]` marker in
the text itself (not only a color), so the flag survives even under `ColorSupport::None`.

**Dependency boundary**: unchanged from E09-S01/E09-S02 - `cancellai-tui` still depends only on
`cancellai-policy` in production. `data::EngineData` gains one field, `explain: Vec<ExplainView>`,
rendered by a new Explain-screen list-plus-detail layout (`Up`/`Down` select an artifact, reduced
modulo the real list length at render time so `App` itself stays engine-data-agnostic).

**Verification ("explanation golden tests")**: `explain`'s own tests build a real
`ClassifiedArtifact`, wrap it in a `ProviderResolution::for_test`, and run it through the actual
`build_actions` - asserting against `build_actions`' real, hardcoded reason strings (stale+
eligible -> the real delete reason; non-stale -> the real retention-window reason; stale but
authority-blocked -> the real constraint name) rather than a mocked stand-in. `cancellai-tui`'s
own render tests cover the `[low confidence]` marker's presence/absence and that a long policy
reason (now `Paragraph::wrap`-enabled in both the Explain and Atlas detail panels, fixing a
real clipping bug this story's own tests caught) renders in full rather than being cut off.

**Residual**: live-scan wiring is deferred identically to E09-S02 - the Explain screen shows "No
artifacts to explain yet." until a future story assembles real classified artifacts and actions.

### Plan review workflow (E09-S04)

**Scope decision, recorded rather than left implicit**: the outcome text ("select/inspect/plan
handoff while execution remains in the shared safety engine") could be read as either the TUI
itself calling `cancellai_safety::mutation_executor::execute_with_system_capabilities`, or the
TUI only reviewing and requiring confirmation while real execution stays a separate step. The
first would require reintroducing `cancellai-safety`/`cancellai-platform` as direct
`cancellai-tui` dependencies - crossing the exact boundary E09-S01 established and E09-S02/S03
preserved - for an interactive confirm-before-delete flow that cannot be validated on a real
terminal from a development environment whose PTY does not pass data through at all (confirmed
independently in E09-S01, not specific to this crate). The owner chose the second reading:
review-only, no real execution, matching the "live-scan wiring is deferred" pattern already
established and evidenced by E09-S02/E09-S03.

No new `cancellai-policy` module: the Plan screen is a different view over the same
`data.explain` list E09-S03 populates, keyed by the same `app.explain_selected` cursor Explain
already uses - "select" is that existing single cursor, not an invented multi-select mechanism.

**AC1 - TUI cannot mutate without a sealed engine plan**: holds by construction. `cancellai-tui`
still depends on neither `cancellai-safety` nor `cancellai-platform` (`Cargo.toml`'s own
comment, and `scripts/check_mutation_boundary.py` confirms no mutation-capability reference
exists anywhere in the crate) - nothing in it, including this screen, is capable of
constructing a `SealedPlan` or reaching the mutation executor. Confirming a plan here reaches
only a terminal, non-executing review state; the confirmed message explicitly names
`cancellai-cli clean` as the real, separate execution path, never claiming to execute itself.

**AC2 - irreversible actions receive stronger confirmation than quarantine**: a new
`EngineData::plan_context` (`data.rs`) derives `PlanContext { can_confirm,
requires_strong_confirmation }` purely from the selected artifact's
`ExplainView::policy_outcome`/`reversibility` - `can_confirm` only for `Recommended`,
`requires_strong_confirmation` only when `reversibility == Reversibility::Irreversible`.
`App::handle_key` (still a pure, engine-data-free reducer - `PlanContext` carries only `bool`s)
implements the two-tier confirmation: one `c` press confirms a non-irreversible recommendation,
an irreversible one arms on the first press and needs a second to confirm; any other key, or
changing the selected artifact, or leaving the Plan screen, cancels a pending or completed
confirmation rather than letting it linger or silently apply to a different artifact.

**Verification ("TUI-to-engine semantic equivalence tests")**: two layers. `cancellai-policy`'s
own `explain.rs` tests (E09-S03) already prove `explain()` copies `reversibility` verbatim from
real `build_actions` output. New `cancellai-tui` tests (`data.rs`, `app.rs`, `ui.rs`) prove
`plan_context`/the confirmation state machine are pure functions of that same
`ExplainView`-carried data, with no separately re-derived classification - `Recommended`+
`Irreversible` requires strong confirmation, `Recommended`+`Quarantinable` does not,
`ObservationOnly`/`NotEvaluated` can never be confirmed regardless of reversibility, and
selection modulo-wraps onto the real list length exactly like the Explain screen.

**Documentation**: this section and `docs/security/SAFETY_INVARIANTS.md`'s SI-016 entry both
record the "review-only, no execution" scope decision so a future reader does not have to
re-derive it from the code.

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
