# E21-S03 / E21-S07 Independent Adversarial Review — Round 2

- Epic: E21 — Target Engine Trust Remediation
- Review-Scope: epic
- Round: 2
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-21
- Review target: `667ce50c8944603b2990b38c754423691eed8f61` (`HEAD` at review start)
- Scope: the round-1 repairs to CR4 stories E21-S03 and E21-S07 only. This is a late
  independent review requested to close the gap recorded in `project/evidence/E21-CLOSURE.md`;
  it does not retrospectively change the owner-directed E21 closure.

## Correction (2026-09-21, same-day reconciliation)

**The E21-S07 finding below was retracted as a false positive** before any production repair was
made. Executor Claude, before repairing E21-S07, independently reproduced the described
root-rename-plus-hard-link attack through the real, unmodified public-API stack (`ApprovedRoot`,
`bind`, a sealed `Delete` plan, `execute`, a side-effecting `IdentityObserver`, and the real
`SystemMutationExecutor`) and obtained `ActionResult::Failed` - the pre-existing post-unlink
link-count check in `confirmed_delete_file_inner` (`cancellai-platform/src/mutation.rs`) catches
this exact interleaving, because deleting the hard-linked decoy leaves the shared inode's link
count at 1, not 0. This reproduction was committed as a permanent regression test:
`rust/crates/cancellai-safety/src/mutation_executor.rs`,
`execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy`.

The discrepancy was put back to Codex (the original verifier) for reconciliation rather than
resolved unilaterally. Codex ran the committed test, confirmed the `Failed` result, and determined
that a probe matching this document's originally stated sequence (two live hard links, the real
`SystemMutationExecutor`, the original entry surviving) **cannot** produce `ActionResult::Succeeded`
against this implementation - the claim was internally inconsistent with the mechanism it
described. Codex could not identify which specific detail of the original temporary probe (already
deleted, per this document's own method) diverged from the stated sequence, but confirmed the
corrected outcome below is the only one that sequence permits, and directed this correction.

E21-S03's finding and repair are unaffected by this correction and are not revisited here; see
`project/evidence/E21-S03/EVIDENCE-ROUND2-REPAIR.md` for that repair's own record.

The sections below are corrected in place, per the reconciling verifier's own instruction, rather
than left to read as still-open; this note is the audit trail of that correction.

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E21-S03 | FAIL, repaired (see `project/evidence/E21-S03/EVIDENCE-ROUND2-REPAIR.md`) | A Claude `projects` root that is a regular file is treated by Rust as a clean, structurally empty scan and `clean --yes` exits `0`. The frozen Python reference records the resulting `ENOTDIR`/“Not a directory”, withholds the tool, and exits `4`. This is an unobserved scope-root/type failure silently converted to authority. |
| E21-S07 | PASS_WITH_RESIDUALS (corrected; see above) | A root-rename-plus-same-inode-hard-link interleaving, reproduced through the real `execute`/`SystemMutationExecutor` stack, is refused: the post-unlink link-count corroboration in `confirmed_delete_file_inner` observes `nlink == 1` (the renamed-away original survives) and returns `ActionResult::Failed`, not `Succeeded`. The originally reported `Succeeded` result could not be reproduced and is retracted. The disclosed `fstatat`/`unlinkat` same-directory entry-swap residual is unchanged. |

## Requirement reconstruction

E21-S03 must distinguish a known-absent/symlinked provider layout from any observation
failure, record each failure with path and reason, lower scope authority, report the real count,
and return safety-blocked exit `4` consistently for plan/clean/dry-run. This implements SI-008,
SI-009, SI-010, SI-014 and C-02.

E21-S07 must bind deletion to the approved directory object rather than a path that can be
re-resolved after validation. It must not mutate another root after a path/root swap; the only
accepted documented residual is a replacement of the *entry in the already-held directory*
between `fstatat` and `unlinkat`. This implements SI-013, SI-016, SI-019, SI-020, SI-003 and
TM-03.

## Reproductions and required repairs

### E21-S03 — non-directory `projects` root is silently complete

Using an otherwise empty synthetic `$HOME`, I created `$HOME/.claude/projects` as a regular
file, then ran the current built Rust binary and frozen Python reference with identical clean
arguments:

```text
HOME=<scratch> rust/target/debug/cancellai-cli clean --yes --allow-running \
  --days 1 --keep-latest 0 --tool claude
Nothing to clean: no artifact is both stale and unblocked.
exit 0

HOME=<scratch> python3 cancellai.py clean --yes --allow-running \
  --days 1 --keep-latest 0 --tool claude
SCAN INCOMPLETE: 1 unreadable path(s) in claude
  unreadable: <scratch>/.claude/projects: Not a directory
Nothing was cleaned: safety withheld the requested work.
exit 4
```

`discover_claude_sessions` classifies `projects_meta.is_dir() == false` as
`SessionDiscoveryScope::Unavailable`, whose `resolve_claude` branch returns a complete empty
resolution. The reference instead attempts directory listing and records `ENOTDIR`.

Classification: **implementation bug**.

Required repair: reserve `Unavailable` for only absent/symlinked `projects`; treat a present
non-directory root as named unobservable evidence (or perform/record the failed `read_dir`),
preserve it through `resolve_claude`, and add native Rust CLI regressions for `clean --yes`,
`clean --dry-run`, and `plan` asserting safety exit `4`, no deletion, and nonzero
`scan_completeness.error_count`. Add it to the parity corpus if the current fixture language can
express a non-directory provider root. This is required by E21-S03 AC1/AC2/AC4, SI-008,
SI-009, SI-010, SI-014, and C-02.

### E21-S07 — corrected: root replacement is refused, not redirected (see Correction above)

The original round-2 pass reported a temporary public-API probe (`ApprovedRoot`, `BoundedPath`,
`SealedPlan`, `execute`, `IdentityObserver`, `SystemMutationExecutor`) performing this
interleaving at the revalidation observation as returning `ActionResult::Succeeded`:

1. seal a Delete plan for `<base>/provider/inner/artifact.txt` under the real `provider` root;
2. rename `provider` to `planned-root-moved-away`;
3. create a replacement `<base>/provider/inner` and hard-link the original planned inode there;
4. return the matching identity for the replacement-path hard link, allowing execution to
   continue.

That result could not be reproduced. The committed regression test
(`rust/crates/cancellai-safety/src/mutation_executor.rs`,
`execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy`) performs the identical
sequence through the same real stack and returns:

```text
Failed { reason: "deletion removed a different filesystem object than the one confirmed open \
(post-deletion link-count check failed); the intended target may still exist" }
```

with `<base>/planned-root-moved-away/inner/artifact.txt` (the original) surviving and only
`<base>/provider/inner/artifact.txt` (the hard-linked decoy) removed.

Why `Succeeded` cannot arise from the stated sequence: renaming the original directory does not
remove its entry, so the target inode has one link under `planned-root-moved-away` throughout.
The hard link at the replacement path brings the shared inode's link count to 2; the file
descriptor `confirmed_delete_file_inner` opened (from the replacement path, after the swap) sees
that count. `unlink_child_matching_unix_identity` removes only the replacement entry, dropping the
count to 1, not 0 - and `confirmed_delete_file_inner` returns the quoted error whenever that final
count is nonzero, which `execute` maps to `Failed`, never `Succeeded`. A probe claiming both "two
live hard links" and "`ActionResult::Succeeded`" against this unmodified implementation is
internally inconsistent; at least one of those claimed probe properties did not hold as described.

Classification: **false positive in the original probe, not a production defect**. No production
repair was made or is required for this finding. The pre-existing, disclosed `fstatat`/`unlinkat`
same-directory entry-swap residual (below) is the only open surface this story still carries.

## Confirmation of round-1 repairs and regression-test audit

### E21-S03

The original mode-`000` counterexample is repaired on the current binary. A synthetic
`$HOME/.claude/projects/project-a/<uuid>.jsonl` with `projects` mode `000` printed
`Nothing was cleaned: safety withheld the requested work.` and exited `4`; the synthetic tree
was restored and removed after the probe. Existing tests are meaningful for that original case:

- `an_unreadable_claude_projects_root_withholds_and_exits_four` exercises a real child process,
  asserts exit `4`, and asserts the session survives.
- `an_unreadable_claude_projects_root_is_reported_incomplete_with_a_real_count` parses the
  native JSON output and asserts `complete=false`, `error_count=1`.
- `an_unreadable_project_directory_makes_the_scope_partial` and
  `a_degraded_companion_is_reported_on_both_channels` cover descendant listing and companion
  failures; `reason_retention_is_bounded_but_the_count_is_not` verifies the 64-reason cap does
  not truncate the total.

They do not exercise a present non-directory scope root. Consequently they establish the
specific permission repair but cannot establish E21-S03's broader AC1/AC2 class.

The root-unreadable, project-unreadable, companion, and bounded-count targeted tests all passed
when rerun. `rust_python_parity.py check` also passed 13 NORMATIVE fixtures in both root-origin
scenarios; it does not contain this non-directory-root counterexample, so it is not contrary
evidence.

### E21-S07

The round-1 directory-link/path-swap repair holds against the case this correction adds, not only
the narrower cases the original round-2 pass credited it for. Targeted tests
`confirmed_delete_detects_a_target_swapped_between_open_and_unlink`,
`the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode`, and
`a_symlinked_intermediate_component_refuses_the_delete` passed, and now so does
`execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy`
(`cancellai-safety/src/mutation_executor.rs`), which specifically exercises a replacement real
directory containing a hard link to the planned inode through the full `execute`/
`SystemMutationExecutor` stack, and confirms the same post-unlink link-count corroboration that
defends the simpler cases also defends this one.

## Residual-risk accuracy (E21-S07)

ADR-0017 and `SealedRoot::unlink_child_matching_unix_identity` accurately disclose the narrow
POSIX window where a writer of an already-bound directory swaps its child between `fstatat` and
`unlinkat`. That residual remains open by construction and is unchanged by this correction.

The root-rename-plus-hard-link interleaving this document originally reported as outside that
residual, and as an additional unsafe surface, is not: the post-unlink link-count check
(`confirmed_delete_file_inner`, independent of and in addition to the handle-relative unlink
itself) catches it, exactly as its own module documentation says it exists to do ("a bare nlink
check after remove_file cannot distinguish... Refusing here... is what keeps a same-named
replacement from being deleted as collateral damage"). No broadening of the disclosed residual is
required.

## Adversarial coverage

| Axis | Case exercised | Result |
| --- | --- | --- |
| Path / identity | Non-directory scope root; root rename plus same-inode hard link | E21-S03 fails current behavior (repaired); E21-S07's hard-link counterexample is refused, corrected. |
| Partial reads / permissions | Real mode-`000` Claude `projects` root and unreadable descendant/companion regressions | Original repair holds; exit `4` and no deletion. |
| Links / mounts | Existing intermediate-symlink deletion regression; hard-link replacement probe | Symlink refusal holds; hard-link root replacement is also refused (corrected - originally misreported as unsafe). |
| Provider layout drift | Missing/symlink roots are known-empty; regular-file root exposed as unhandled malformed layout | FAIL for present non-directory root (E21-S03, repaired). |
| Concurrency | Deterministic identity-observer interleaving replaces root between revalidation and system mutation | E21-S07 refuses the interleaving (corrected; originally misreported as FAIL). |
| Crash / retry | Not implicated by either reproduction; no mutation is permissible after the S03 safety block | Not independently expanded. |
| Boundary values | Empty/missing provider remains exit `0` in the named counter-test | Existing behavior retained. |
| Policy / trust conflicts | Scope incompleteness is authority-withholding before policy can authorize deletion | S03 regular-file root bypasses this barrier. |
| Platform differences | Native macOS Unix execution; Windows implementation was compile-tested by workspace clippy only | Runtime conclusion is Unix-scoped. |
| Malformed / untrusted input | Regular file in place of expected discovery directory | E21-S03 FAIL. |
| Performance / large datasets | Reason retention test confirms bounded retained reasons and truthful count | No additional performance defect found. |

## Gate results

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS before review record creation: `governance OK: 25 decisions, 33 epics, 168 stories` |
| `python3 scripts/check_mutation_boundary.py check` | PASS: only `rust/crates/cancellai-platform/src/mutation.rs` deletes; only it and safety executor reference the capability |
| `python3 scripts/check_fixtures.py check` | PASS: 13 fixtures cover required categories |
| `python3 scripts/check_schemas.py check` | PASS: 4 golden documents match |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/rust_python_parity.py self-test` | PASS |
| `python3 scripts/rust_python_parity.py check` | PASS: 13 NORMATIVE fixtures, both root origins |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS (scheduled performance tests remained intentionally ignored) |
| `cargo deny check` | PASS after the sandboxed attempt could not lock the read-only advisory DB; completed with approved advisory-DB access. Existing unmatched-license and duplicate-dependency warnings only. |
| `pre-commit run --all-files` | FAIL only at `process-metrics-check`: this new review record makes `project/generated/PROCESS_METRICS.md` stale. The user-directed scope forbids modifying that generated file; every other hook, including evidence, handoff, safety-oracle, parity, mutation-boundary, and Rust-workspace checks, passed. |

The temporary E21-S07 public-API integration probe was intentionally failing (`result=Succeeded`)
and was removed immediately after reproduction. No temporary files remain.

## CR4 Safety Verdict — E21-S03

- Change: Scan completeness propagation in provider adapters
- Risk: CR4
- Commit/PR: `667ce50c8944603b2990b38c754423691eed8f61`
- Independent verifier: Codex
- Date: 2026-09-21

### Verdict

`FAIL`

### Safety surface changed

Provider observation decides whether deletion authority is withheld. A present regular file at
the Claude discovery root is currently promoted to a complete/empty state.

### Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | Partial/unobservable scope is non-destructive | Non-directory root became complete and clean exited `0` | FAIL |
| SI-009 | Missing evidence is not absence of data | Python records `ENOTDIR`; Rust treats same state as absent/empty | FAIL |
| SI-010 | Scan errors are visible | Rust emitted no completeness error for the present non-directory root | FAIL |
| SI-014 | Safety withholding is non-success | Rust exited `0` rather than the required safety exit `4` | FAIL |

### Adversarial cases

Mode-`000` root, descendant, and companion failures were independently rerun and hold. The
non-directory scope-root case above falsifies the complete failure class.

### Differential / compatibility evidence

The standard differential gate passes, but the direct Python/Rust reproduction diverges. The
fixture corpus lacks this malformed-root topology.

### Known residual risks

Reason retention is bounded to 64 named paths while retaining a truthful total; that residual is
acceptable but does not mitigate the failure found here.

### Rollback / recovery

No deletion occurred in the reproduction. Until repair, malformed provider roots must be treated
as unsafe and destructive use withheld.

### Owner decision

`PENDING — verifier recommends REJECT until the required repair and an independent rerun pass.`

## CR4 Safety Verdict — E21-S07 (corrected; see Correction above)

- Change: Handle-relative unlink for confirmed deletion
- Risk: CR4
- Commit/PR: `667ce50c8944603b2990b38c754423691eed8f61`
- Independent verifier: Codex
- Date: 2026-09-21 (original pass); correction confirmed same day via reconciliation

### Verdict

`PASS_WITH_RESIDUALS`

### Safety surface changed

The mutation seam changed irreversible deletion from path-based removal to handle-relative
`unlinkat`. The parent handle is acquired after outer revalidation; a root-object swap in that
window was hypothesized to redirect the descriptor acquisition undetected. Reproduction shows the
pre-existing post-unlink link-count corroboration independently catches it.

### Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Revalidation binds the identity actually mutated | Root/hard-link reproduction returns `Failed`, not `Succeeded`; the real target's identity is never satisfied by the decoy alone | PASS |
| SI-016 | Mutation executes the sealed root/target plan | The planned directory entry survives (now under the renamed-away root); only the decoy is removed, and the operation reports `Failed` | PASS |
| SI-019 | The sole mutation path remains evidence-gated | The link-count corroboration, layered on the handle-relative unlink, refuses this interleaving | PASS |
| SI-020 | Irreversible action is explicit and stronger-gated | No irreversible deletion of the planned artifact occurred | PASS |

### Adversarial cases

The same-name swap, intermediate-symlink, and root-rename-plus-hard-link regressions all pass
(the last added by this correction:
`execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy`).

### Differential / compatibility evidence

Not applicable to this OS-level mutation race. Mutation-boundary structural check passes; the
regression test proves the retained capability's post-unlink corroboration is bound to the
originally opened object, not the replacement root.

### Known residual risks

The disclosed `fstatat`/`unlinkat` same-directory entry-swap race remains, unchanged and
unbroadened by this correction.

### Rollback / recovery

Not applicable - the reproduction found no irreversible mutation of the wrong object.

### Owner decision

`PASS_WITH_RESIDUALS, corrected same day. No production repair required or made for E21-S07. The
original REJECT recommendation is withdrawn.`

## Documents opened

- `.claude/skills/epic-verifier/SKILL.md`
- `.claude/skills/adversarial-cases/SKILL.md`
- `.claude/skills/risk-gate/SKILL.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`
- `docs/architecture/JSON_CONTRACTS.md`
- `docs/architecture/PLATFORM_MODEL.md`
- `docs/CLI_RUST.md`
- `project/epics/E21.json`
- `project/evidence/E21-VERIFIER-REVIEW.md`
- `project/evidence/E21-CLOSURE.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`
- `project/templates/SAFETY_VERDICT.md`
- `docs/audits/2026-09-03-CODE_REVIEW.md`
- `rust/crates/cancellai-provider-claude/src/session.rs`
- `rust/crates/cancellai-provider-codex/src/session.rs`
- `rust/crates/cancellai-inventory/src/completeness.rs`
- `rust/crates/cancellai-policy/src/retention.rs`
- `rust/crates/cancellai-cli/src/main.rs`
- `rust/crates/cancellai-cli/tests/cli_behavior.rs`
- `rust/crates/cancellai-sealedfs/src/lib.rs`
- `rust/crates/cancellai-platform/src/mutation.rs`
- `rust/crates/cancellai-safety/src/mutation_executor.rs`
- `rust/crates/cancellai-safety/src/root_capability.rs`

### Reconciliation pass (same day)

- `rust/crates/cancellai-safety/src/mutation_executor.rs` (the committed
  `execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy` test and its
  `RootRenamePlusHardlinkObserver` helper)
- `rust/crates/cancellai-platform/src/mutation.rs` (`confirmed_delete_file_inner`'s post-unlink
  link-count check)
