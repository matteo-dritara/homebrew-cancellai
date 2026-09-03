# ADR-0018: Scope completeness is a shared type, not a shared traversal

- Status: Accepted
- Date: 2026-09-03
- Owners: project owner
- Related: ADR-0007, ADR-0016, E04-S02, E04-S03, E05-S03, E05-S04, E06-S01, E21-S03, E21-S04,
  E21-S05, C-02, SI-008, SI-009, SI-010

## Context

E04 built `cancellai-inventory`: `scan_scope`, one recursive walk per scope producing one
`InventorySnapshot`, with scope completeness classified `Complete`/`Partial`/`Unknown` and a
`PlanningView` that cannot be obtained without carrying that classification. E04-S03's
independent review round rejected the story once, specifically because an entry that
`read_dir` listed but that could not then be observed was silently dropped instead of
degrading the scope to `Partial`. The repair landed, was verified, and the epic closed.

E05 and E06 then built the shipping pipeline without using any of it. `cancellai-cli`'s
OBSERVE stage is `discover_claude_sessions` and `discover_codex_sessions` inside the provider
adapters - a second, independent traversal. The adapters' own module docs state the reason
plainly: Claude's `projects/<project>/<session>.jsonl` plus optional companion payload
directory needs a bespoke walk, matching how `cancellai.py`'s own discovery is a bespoke
function rather than a generic tree walk. That reason is sound. Its consequence was not
examined.

The 2026-09-03 target-engine review (`docs/audits/2026-09-03-CODE_REVIEW.md`, CR-TE-01 and
CR-TE-02) measured the consequence. The exact defect E04-S03's reviewer had rejected exists
again, in the adapters: a directory that cannot be listed is skipped by a bare
`else { continue }`, the scope still reports `complete: true`, and the engine deletes an
eligible artifact where the frozen Python reference withholds the whole tool and exits 4. Two
further consequences follow from the same split: the E04-S04 performance budget and the
scheduled benchmark workflow measure `scan_scope`, which the shipped binary never calls; and
the "one traversal per scope" property E04-S02 proved does not hold for the pipeline that
runs.

So the question is not whether `cancellai-inventory` was good work. It is which of its
guarantees the product actually needs to inherit, and by what mechanism.

## Decision

We will treat **scope completeness as a shared type obligation, not a shared traversal**.

- The provider adapters keep their layout-specific discovery. Claude's flat
  project/session shape and Codex's rollout graph with `parent_thread_id` lineage are
  genuinely different problems, and both were written to match the frozen reference's own
  behaviour, which the differential gate pins.
- Discovery must return `cancellai-inventory`'s `ScopeCompleteness`, with named reasons, for
  every scope it observes. Silently dropping an unobservable path stops being expressible.
- `cancellai-policy` must obtain planning candidates only through a value that cannot be
  constructed without completeness attached - the same construction-level guarantee E04-S03
  committed for `planning_view`, including its `compile_fail` regression.
- `cancellai-inventory` must be reachable from the shipped `cancellai-cli` dependency graph,
  and a check must fail if it becomes unreachable again.
- The performance gates move onto the discovery path the CLI invokes.

The load-bearing part of E04 was never the generic walker. It was the impossibility of
holding planning candidates without holding the evidence about how completely they were
observed. That property is portable to a bespoke traversal; the traversal is not portable to
two unlike layouts.

## Alternatives considered

### Rebase the adapters onto `scan_scope`

One traversal, one model, the benchmark automatically measuring the shipped path, and
CR-TE-01 structurally impossible rather than repaired. Rejected for cost and risk, not for
correctness: the adapters' current discovery reproduces `cancellai.py`'s `os.walk` semantics
closely enough to pass the differential gate in both root-origin scenarios, and rewriting
them puts that verified parity back in play during the same epic that is repairing a safety
defect. It remains the better end state if a later epic - E10's storage accounting is the
natural home - needs a unified traversal for its own reasons. This ADR does not foreclose it.

### Declare `cancellai-inventory` non-production and replicate completeness by hand

Cheapest. Rejected because it is what the repository already does by accident, and it is what
allowed the same defect to appear twice. Two hand-maintained models of the same invariant
regress independently; a shared type does not.

### Leave it as is and rely on review

Rejected on evidence. Review did catch this defect - in E04-S03, adversarially and correctly -
and the fix still did not reach the shipped path, because the guarantee lived in a component
rather than in the contract between components.

## Consequences

### Positive

- The invariant that unknown observation reduces authority (C-02, SI-008, SI-009) becomes a
  type-level obligation on every provider adapter, present and future, instead of a rule each
  adapter is trusted to remember.
- A new provider adapter cannot ship a silently incomplete scan; it will not compile without
  answering the completeness question.
- The performance budget starts describing the binary users run.

### Negative / cost

- `cancellai-inventory` keeps a walker that production does not call. It is exercised by its
  own tests and remains the reference implementation of the completeness semantics, but it is
  not, on this decision, the shipped traversal. That is a deliberate residual, and it is
  recorded here rather than left to be rediscovered.
- The adapters take a dependency on `cancellai-inventory`, which the crate-graph checker must
  accept as a legitimate edge in `docs/architecture/TARGET.md`'s layering.

### Neutral / follow-up

- E21-S04 implements this. E21-S05 moves the performance gates.
- If E10 later unifies traversal, this ADR is superseded rather than contradicted: the shared
  type stays correct either way.

## Safety and compatibility impact

- Change Risk implication: CR2 for the refactor itself, which must not change a single
  observable classification; the behavioural repair it supports is CR4 (E21-S03).
- Safety Invariants affected: SI-008 (partial scan is non-destructive), SI-009 (unknown scan
  state is non-destructive), SI-010 (scan errors are visible).
- Migration/rollback: no persisted state and no user-visible contract change. The differential
  gate is the rollback signal: if it diverges during the refactor, the refactor is wrong.

## Supersession

If replaced later, keep this ADR and mark it superseded by ADR-XXXX.
