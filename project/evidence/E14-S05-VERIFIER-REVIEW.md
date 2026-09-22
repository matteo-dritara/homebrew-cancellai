# E14-S05 Independent Verifier Review — Round 1

Review-Scope: story
Round: 1
Review-Target: `c7f12bf53af302e9cf10600444ef69ff27021d32`
Verifier: Codex
Date: 2026-09-22

This review was performed independently from E14-S05's generated verifier brief, its acceptance
criteria, SI-004/SI-013, ADR-0036/ADR-0037, and the committed source. It is a rejection, not a
repair: no source, documentation, control-plane, or executor evidence was changed.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S05 | FAIL | A non-test external Rust consumer used the public `SyntheticProviderLayoutObserver` with public `SealedPlan::seal` and public `execute`, plus `SystemMutationExecutor`, to delete a real file while its actual root layout contradicted the fabricated signature. Two further native reproductions showed a check-to-mutation layout race and a Restore into a changed provider root that was never observed. |

## Findings

### F1 — Public synthetic observation bypasses the live layout gate (SI-004; AC2)

`ProviderLayoutObserver` is public (`rust/crates/cancellai-platform/src/provider_layout.rs:200`),
as is `SyntheticProviderLayoutObserver` and its `set_observed` constructor
(`provider_layout.rs:225-250`). `SealedPlan::seal` accepts that trait object publicly
(`rust/crates/cancellai-safety/src/sealed_plan.rs:145-153`), and the public `execute` accepts it
again (`mutation_executor.rs:102-109`). The same external consumer can name the public
`cancellai_platform::mutation::SystemMutationExecutor`. Thus the system wrapper's correct
hardcoding (`mutation_executor.rs:325-333`) does not make the injected seam test-only or prevent
another real mutation call path.

I compiled and ran an out-of-workspace consumer at
`/private/tmp/e14-s05-verifier.2qrH8v` with:

```text
cd /private/tmp/e14-s05-verifier.2qrH8v && cargo run --offline --quiet
```

Its first scenario created a real provider root containing `artifact.txt`, established a real
`ApprovedRoot`/`BoundedPath`, then configured the public synthetic observer with the genuine
`target.root_identity()` but the contradictory marker `recognized-only-marker`. It passed that
observer to both `SealedPlan::seal` and `execute`, while passing real system identity, process,
and mutation capabilities to `execute`. The command exited 0 only after asserting
`ActionResult::Succeeded` and asserting that the real `artifact.txt` no longer existed.

This is the E14-S04 rounds 3/4 shape at the newly introduced layer: the fact cannot be supplied
as a bare `LayoutSignature`, but the public synthetic implementation supplies the opaque
`BoundLayoutObservation` on demand. `check_mutation_boundary.py` passes because it only scans
the repository's production sources; it cannot make a public capability unavailable to a Rust
consumer. AC2 says “No caller”, so the compiled consumer falsifies it.

### F2 — The observation is released before mutation; layout drift in that window still deletes (SI-004, SI-013; AC1)

`execute` observes and compares the layout at lines 163-168, then constructs an operation and
only later invokes the separate mutation capability. `BoundLayoutObservation::observe` does bind
its directory safely while enumerating, but its held `SealedRoot` has been dropped by the time
`SystemMutationExecutor` binds the target anew. No retained root capability or observation permit
connects the fresh layout to the actual mutation.

The second scenario in the same external consumer sealed a genuine system-observed Delete plan.
Its `ProviderLayoutObserver` implementation first called the real
`BoundLayoutObservation::observe(root)`, then synchronously created a new marker in that root
before returning the otherwise genuine observation. `execute` returned `Succeeded` and deleted
the artifact. This deterministically represents a concurrent provider write in the interval
after the fresh read and before the mutation. The pre-check saw the old matching signature, but
the destructive action occurred after the root had drifted.

The committed real-filesystem test only adds a marker *before* invoking `execute`; it does not
exercise this interval. The safe observation primitive developed for E14-S04 cannot close this
race when its descriptor does not survive through the mutation boundary.

### F3 — Restore validates the quarantine root, not the provider root it mutates (SI-004, SI-013; AC1/AC3)

`seal_restore` explicitly defines its `root` as the quarantine store, “not the original provider
root” (`sealed_plan.rs:227-232`), yet it records `observe_provider_layout(root, ...)`
(`sealed_plan.rs:243-254`). `execute` later always observes `target.root_path()`
(`mutation_executor.rs:163`), which is that same quarantine root. `MoveDestination` records only
the destination path and identity, not its root path, so there is no provider-root layout
baseline or fresh observation available for a Restore operation.

The third external scenario used only system observers/executors. It sealed a Restore from a
quarantine store to a real provider-root destination, added a new provider-root marker after
sealing, then executed. The result was `Succeeded` and the artifact was moved into the changed
provider root, because the unchanged quarantine store was the sole layout checked. This is not a
synthetic-seam artifact. It directly fails the requested provider-root drift behavior for one of
the action classes the new API claims to cover. Quarantine and Archive have the provider root as
their source root; Restore is materially different and needs an explicit destination-provider
binding and live revalidation.

## Acceptance criteria and invariant assessment

| Requirement | Result | Independent evidence |
| --- | --- | --- |
| AC1: stale recognized plan is refused by a fresh provider-root observation at execution | FAIL | F2 mutates after the fresh observation but after an intervening real layout drift; F3 restores into a drifted provider root that is never observed. |
| AC2: no caller can reach real destructive mutation using contradictory/omitted layout facts | FAIL | F1 is a compiled external consumer that combines the public synthetic observer with real boundary, identity, process, and mutation capabilities. |
| AC3: unobservable roots fail closed | PASS for the implementation's observed source root | `revalidate_provider_layout` returns `StalePlan` for a fresh error and for a missing seal-time baseline. This does not cure F3: the actual Restore destination provider root is not observed at all. |
| SI-004: unknown/drifted provider layout reduces destructive capability | FAIL | F1 supplies false “known” observations; F2 and F3 perform real destructive actions despite an actual drifted provider layout. |
| SI-013: relevant root identity/preconditions are freshly re-observed immediately before mutation | FAIL | F2 demonstrates the root-layout fact is not bound through the actual mutation; F3 re-observes the wrong root for Restore. |

## Test and platform assessment

- The new synthetic drift and unobservable tests establish local branch behavior, but they treat
  the public synthetic observer as if it were test-only and do not compile a hostile consumer.
- The real Delete drift test changes layout before `execute`, not between the observation and the
  actual mutation, and therefore provides no evidence against F2.
- The Restore full-stack test has an unchanged destination provider root; it masks the root-role
  mismatch in F3.
- The claimed Windows/non-Unix consequence is **confirmed** from source and cross compilation:
  `cancellai-sealedfs::windows_sealed::SealedRoot::metadata` and `list_child_names` return
  `SealError::Unsupported` (`rust/crates/cancellai-sealedfs/src/windows_sealed.rs:523-542`).
  That reaches `BoundLayoutObservation::observe` as an error, then the new revalidation refuses.
  It is an accurately disclosed, fail-closed regression of Windows Delete availability. It may be
  an owner-accepted trade-off, but it could not be runtime-tested here; only the Windows clippy
  target was available locally.

## Gate results

| Command | Result |
| --- | --- |
| External reproducer: `cargo run --offline --quiet` | FAIL reproduced: all three asserted unsafe-success scenarios completed; compiler emitted only unused-import warnings in the throwaway consumer. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS |
| `cd rust && cargo deny check` | PASS — after the initial sandboxed invocation could not acquire Cargo's advisory-db lock; the authorized rerun passed advisories, bans, licenses, and sources (existing warnings only). |
| `cargo clippy -p cancellai-platform -p cancellai-safety --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings` | PASS |
| `cargo clippy -p cancellai-platform -p cancellai-safety --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_evidence.py check` | PASS (recorded baseline warnings only) |
| `python3 scripts/check_ears.py check` | PASS (recorded baseline warnings only) |
| `python3 scripts/check_platforms.py check` | PASS |
| `gh run list --branch main --limit 5` | UNKNOWN — DNS resolution of `api.github.com` failed in this environment, so CI was not treated as green. |

## CR4 Safety Verdict — E14-S05

## Verdict

`FAIL`

## Safety surface changed

The sole mutation boundary now takes a provider-layout observation before each destructive
operation. This is a CR4 authority change because a matching observation permits the action and
a missing/drifted one blocks it.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 | A real mutation cannot retain destructive capability from a fabricated, unknown, or drifted provider layout. | F1 fabricated the public synthetic observation; F2/F3 performed real mutations after/into a drifted provider layout. | FAIL |
| SI-013 | Relevant root preconditions are freshly observed and connected to the mutation immediately before it. | F2 has no retained binding from layout read to mutation; F3 observes the quarantine source rather than Restore's provider destination. | FAIL |

## Adversarial cases

- External fabricated synthetic layout with real `ApprovedRoot`, real `BoundedPath`, and real
  `SystemMutationExecutor`: deletion succeeded.
- Real observation followed by a marker write before `execute` reached mutation: deletion
  succeeded.
- Real Restore into a provider root whose layout changed after sealing: restore succeeded.
- Windows/non-Unix unsupported layout observation: source/cross-target-confirmed fail-closed
  behavior; no local Windows runtime was available.

## Differential / compatibility evidence

No Python differential applies. Windows destructive capability is deliberately narrowed: the
claim is truthful in the source, but local Windows runtime verification was unavailable.

## Known residual risks

- The documented Windows/non-Unix refusal is a real compatibility regression, albeit fail-closed
  and explicitly disclosed by ADR-0037.
- CI status remains unknown from this environment because GitHub's API could not be resolved.

## Rollback / recovery

Do not release E14-S05. Revert `c7f12bf` to restore the prior platform behavior if necessary;
that rollback does not provide the intended E14-S05 layout protection. A repair needs a
non-forgeable production-only path (not a public synthetic observer injection), a layout/root
binding that survives through the mutation, and separate source/destination provider-root
semantics for Restore.

## Owner Decision

`REJECT`

Owner action required: return E14-S05 to the executor for a structurally revised design and new
adversarial regressions. The verifier does not set story status or accept residual risk on the
owner's behalf.
