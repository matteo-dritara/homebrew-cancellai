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

---

# E14-S05 Independent Verifier Review — Round 2

Review-Scope: story
Round: 2
Review-Target: `414a9fcaed79fafea624507c4b440b6141b1e059`
Verifier: Codex
Date: 2026-09-22

This is an independent repair review of round 1's F1/F2/F3 findings. I read the round-1
record, executor packet, ADR-0037's round-2 account, the story/ACs, SI-004/SI-013, relevant
architecture and threat documentation, and re-diffed `c7f12bf..414a9fc`; executor claims were
not accepted as evidence by themselves.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S05 | PASS_WITH_RESIDUALS | The external F1 consumer now fails specifically with Rust `E0603` because `execute` is `pub(crate)`; a fresh-system-wrapper synthetic-seal attempt is safely blocked; and a native Restore destination-layout drift is safely blocked while retaining the quarantine source. F2's read-to-syscall window remains, but is visibly narrowed to the last pre-mutation statement and is an acceptable disclosed residual at this stage. |

## Round-1 finding re-verification

### F1 — Closed: the public synthetic-observer path is inaccessible

I created a standalone external crate with path dependencies on `cancellai-safety` and
`cancellai-platform`, importing `cancellai_safety::mutation_executor::execute` and assembling
the same category of inputs as the round-1 consumer: `SyntheticProviderLayoutObserver`, system
identity/mutation capabilities, and a sealed plan. `cargo check --offline` failed with exactly:

```text
error[E0603]: function `execute` is private
 --> src/main.rs:10:42
  |
10 | use cancellai_safety::mutation_executor::execute;
  |                                          ^^^^^^^ private function
note: the function is defined as `pub(crate)` in mutation_executor.rs:115
```

This is a visibility failure at the intended boundary, not a dependency, network, or unrelated
build error. `lib.rs` also no longer re-exports `execute` or `execute_all`.

I then checked the remaining public production path rather than stopping at that compile error.
`execute_with_system_capabilities` accepts only `&SealedPlan` and `&BoundedPath`, and hardcodes
`SystemIdentityObserver`, `SystemMutationExecutor`, `SystemProcessObserver`, and
`SystemProviderLayoutObserver`. `BoundedPath` has no public constructor; `ApprovedRoot::bind`
derives it from real path/identity observations. A public caller can still seal a plan with a
synthetic layout observer, but cannot substitute it at execution: in a second external native
run, I sealed a Delete plan with a synthetic signature intentionally contradicting the real root
then called `execute_with_system_capabilities`. The wrapper returned `SafelyBlocked` and retained
the artifact. Thus that public seal-time test seam does not reproduce the real mutation bypass.

Existing legitimate production use remains intact: `cancellai-cli` calls the public wrapper once
per artifact; the crate's own tests still reach the injectable executor internally through
`pub(crate)` visibility. `execute_all` has no production caller, so restricting it does not
break an extant consumer.

### F2 — Residual accepted: last-precondition placement narrows, but cannot close, the race

The provider-layout revalidation now occurs after every action-specific operation is assembled
and immediately before `executor.mutate`; source inspection confirms no operation-building,
filesystem, or caller-controlled work follows it. There remains a real interval between the
completed observation and the mutation capability's syscall. I confirmed this from the actual
call sequence; the code correctly does not claim otherwise.

This is not a full SI-013 closure. It is, however, the same narrower immediate-revalidation
posture this repository accepted for single-file identity before the separately scoped E21-S07
handle-relative/descriptor-retaining closure. A whole-provider-root layout binding must retain a
directory observation through the platform mutation primitive; that is a material platform and
API design change, rather than a safe small extension to this repair. The residual is explicit
in ADR-0037, the executor evidence, CHANGELOG, and the affected architecture documents. I judge
shipping the deliberately narrowed residual acceptable as `PASS_WITH_RESIDUALS`, not a hidden or
blocking claim of full atomicity.

### F3 — Closed: Restore observes and revalidates the real destination provider root

`MoveDestination::root_path()` is populated from the already-established destination
`ApprovedRoot`. `seal_restore` observes that path, whereas `seal`, `seal_with_process_guard`,
`seal_quarantine`, and `seal_archive` continue to observe their own source `root.path()`.
`ProviderLayoutSnapshot` keeps the selected root path, identity, and normalized signature;
`revalidate_provider_layout` re-observes that stored path rather than deriving a root from the
execution target.

I reproduced the round-1 case externally using only real system capabilities: I created a
quarantine root holding an artifact, a distinct provider destination root with an initial marker,
sealed a Restore plan, added a new marker to the real destination root, then called
`execute_with_system_capabilities`. It returned `SafelyBlocked`; the quarantined artifact
remained and no destination artifact was written. The plan's exposed recorded layout-root path
equalled the established provider destination path (including the platform's canonical path),
not the quarantine root. This independently confirms the execution behavior, not merely the new
unit assertion.

## Fresh AC and safety-obligation assessment

| Requirement | Result | Independent evidence |
| --- | --- | --- |
| AC1 | PASS_WITH_RESIDUALS | Real Delete drift before execution and my real Restore-destination drift are refused by fresh system observation. The unbound observation-to-syscall window remains explicitly residual (F2). |
| AC2 | PASS | External direct `execute` call fails with E0603; the only public execution wrapper hardcodes system capabilities. An externally synthetic-sealed but system-executed contradictory plan was blocked without mutation. |
| AC3 | PASS | A missing seal-time snapshot and a fresh observation error each return `StalePlan`; no unobservable-layout branch reaches `mutate`. The same check now applies to Restore's real destination root. |
| SI-004 | PASS_WITH_RESIDUALS | Drifted/deleted/replaced roots and layout signatures are refused through the public mutation path. The known-signature trust-source residual from ADR-0036 is unchanged, and F2 is the narrow temporal residual. |
| SI-013 | PASS_WITH_RESIDUALS | Target identity plus the action-appropriate provider-root identity/layout are freshly revalidated immediately before mutation. The layout observation is not retained through the syscall (F2). |

## Additional review observations

- `ProviderLayoutSnapshot` is crate-private with crate-private fields; external callers cannot
  construct or alter a stored path, root identity, or signature. The plan remains immutable.
- The changed action-class mapping is coherent: Delete/Quarantine/Archive mutate against their
  source provider root, while Restore mutates into its destination provider root. The existing
  same-device destination guard remains before the layout check and mutation.
- A documentation precision note, not a runtime bypass: because public seal constructors accept
  `&dyn ProviderLayoutObserver`, an external caller can use the public synthetic observer at
  seal time. Statements that describe the stored seal-time snapshot alone as invariably from a
  real/non-forgeable observer should be read with the essential execution qualification: only
  the public mutation path's *fresh* observer is hardcoded system-backed. The external
  contradictory-plan run above confirms the actual safety property holds.
- I found no new malformed-input, link/path, retry, process, boundary, or platform-specific
  mutation path introduced by the repair. Windows/non-Unix runtime was not available locally;
  source/cross-target lint confirms the intended fail-closed refusal remains.

## Gate results

| Command | Result |
| --- | --- |
| External F1 compile probe: `cargo check --offline` | PASS — expected `E0603` privacy error for `execute`, after dependencies compiled successfully. |
| External F1 wrapper probe: `cargo run --offline --quiet` | PASS — fabricated seal-time layout was safely blocked by the public system wrapper; artifact retained. |
| External F3 Restore-drift probe: `cargo run --offline --quiet` | PASS — destination layout drift safely blocked; source retained and destination absent. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS — full workspace and doctests. |
| `cd rust && cargo deny check` | PASS — advisories, bans, licenses, and sources passed; existing unmatched-license/duplicate warnings only. |
| Cross-target clippy, Windows and Linux (`cancellai-platform`, `cancellai-safety`) | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS — 96 Rust sources scanned. |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_evidence.py check` | PASS — 15 recorded pre-convention warnings only. |
| `gh run list --branch main --limit 5` | UNKNOWN — this environment could not connect to `api.github.com`; CI is not treated as green. |

## CR4 Safety Verdict — E14-S05

- Change: live provider-layout freshness check at the sole mutation boundary, including
  per-action provider-root binding for Restore.
- Risk: CR4
- Commit/PR: `414a9fcaed79fafea624507c4b440b6141b1e059`
- Independent verifier: Codex
- Date: 2026-09-22

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

The public mutation entry point now requires a fresh, system-backed provider-layout observation
for the root the action actually mutates against. Injectable internal execution seams are no
longer callable by an external Rust consumer.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 | A fabricated, unknown, or drifted provider layout cannot preserve destructive capability for a real mutation. | Expected E0603 external direct-executor failure; system-wrapper fabricated-seal block; native Restore-destination drift block; fail-closed absent/error paths. | PASS_WITH_RESIDUALS |
| SI-013 | Relevant target and provider-root identity/preconditions are freshly checked immediately before mutation. | Actual last-precondition placement; root/marker comparisons; native Restore drift reproduction. Layout observation is not retained through the syscall. | PASS_WITH_RESIDUALS |

## Adversarial cases

- External fabricated-observer consumer cannot call `execute` because it is crate-private.
- External synthetic seal-time contradiction cannot evade the wrapper's fresh system observation.
- Restore into a provider root drifted after sealing is refused without moving the quarantine
  source.
- Source-root action constructors retain their own source-root observation mapping.

## Differential / compatibility evidence

No Python differential applies. Windows/non-Unix provider-layout observation remains
unsupported and therefore refuses destructive execution fail-closed; Windows runtime testing was
not available here, while the requested Windows cross-target lint passed.

## Known residual risks

- **F2:** a concurrent provider-layout change between the final layout observation and the OS
  mutation syscall is not caught. Full closure needs a handle-retained directory-layout
  capability through the platform mutation operation.
- **Windows/non-Unix:** destructive execution remains refused until a verified handle-bound
  provider-layout observation is implemented.
- **ADR-0036 residual (1):** there is still no trusted known-layout-signature source; this story
  verifies live-vs-seal freshness, not known-good classification.
- CI status is unknown from this environment because GitHub API connectivity failed.

## Rollback / recovery

Reverting `414a9fc` restores the rejected round-1 implementation and is not a safety-preserving
rollback. The safe recovery for the remaining F2 residual is to withhold the action when a
provider root cannot be reliably observed, which the current implementation already does; a
future handle-retained design should receive its own CR4 story and review.

## Owner decision

`ACCEPT_WITH_RECORDED_RESIDUALS` is the verifier recommendation; owner acceptance remains
required. This verifier does not change the story status or make that owner decision.
