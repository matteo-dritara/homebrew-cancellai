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

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E21-S03 | FAIL | A Claude `projects` root that is a regular file is treated by Rust as a clean, structurally empty scan and `clean --yes` exits `0`. The frozen Python reference records the resulting `ENOTDIR`/“Not a directory”, withholds the tool, and exits `4`. This is an unobserved scope-root/type failure silently converted to authority. |
| E21-S07 | FAIL | A root swapped after revalidation to a replacement directory containing a hard link to the planned inode redirects the current delete path. A temporary public-API probe observed `ActionResult::Succeeded`; it unlinked the replacement-root entry while the original planned entry survived under the renamed-away root. This is outside the disclosed `fstatat`/`unlinkat` same-directory entry-swap residual. |

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

### E21-S07 — root replacement can redirect the deletion before the descriptor is bound

I created and removed a temporary integration test using only public APIs
(`ApprovedRoot`, `BoundedPath`, `SealedPlan`, `execute`, `IdentityObserver`, and
`SystemMutationExecutor`). Its observer performed this deterministic interleaving at the
revalidation observation:

1. seal a Delete plan for `<base>/provider/inner/artifact.txt` under the real `provider` root;
2. rename `provider` to `planned-root-moved-away`;
3. create a replacement `<base>/provider/inner` and hard-link the original planned inode there;
4. return the matching identity for the replacement-path hard link, allowing execution to
   continue.

The probe's assertion expecting a failure failed with:

```text
result=Succeeded
```

After the reported success, `<base>/provider/inner/artifact.txt` (the replacement-root link)
was removed and `<base>/planned-root-moved-away/inner/artifact.txt` (the original planned
directory entry) still existed. The operation therefore followed a replacement root after
validation and reported successful deletion of the planned artifact even though that directory
entry was not removed.

The current `confirmed_delete_file_inner` first revalidates `target` by pathname and only later
calls `SealedRoot::bind_existing(target.parent())`. A hard link preserves the expected file
identity across the root swap, so the new parent is successfully bound and the held descriptor
is for the replacement root, not the approved root. The post-unlink link-count corroboration
also reports success here because the file descriptor was opened from that replacement link.

Classification: **implementation bug**.

Required repair: retain and revalidate a handle/capability for the approved root through the
mutation boundary, derive the parent directory relative to that retained root, and reject a
current root whose native identity differs from the plan's sealed root before any child is
opened or removed. A regression must reproduce the root-rename plus hard-link interleaving and
assert a safety block/failure with both directory entries intact. This is required by E21-S07
AC2 and its verification contract, and violates SI-003, SI-013, SI-016, SI-019 and TM-03.

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

The round-1 directory-link/path-swap repair is real but narrower than claimed. Targeted tests
`confirmed_delete_detects_a_target_swapped_between_open_and_unlink`,
`the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode`, and
`a_symlinked_intermediate_component_refuses_the_delete` passed. They meaningfully show that a
same-name replacement with a different inode is refused and that a path already traversing an
intermediate symlink is refused.

They do not retain the *approved root* through the handoff from safety revalidation to
`SystemMutationExecutor`, and none tests a replacement real directory containing a hard link to
the planned inode. The temporary probe above tests that omitted interaction and falsifies the
broader AC2 assertion.

## Residual-risk accuracy (E21-S07)

ADR-0017 and `SealedRoot::unlink_child_matching_unix_identity` accurately disclose the narrow
POSIX window where a writer of an already-bound directory swaps its child between `fstatat` and
`unlinkat`. That residual remains open by construction.

It is not the whole remaining surface. The reproduced root replacement occurs earlier, between
the safety layer's pathname revalidation and `bind_existing(target.parent())`; the replacement
parent is then deliberately and successfully bound. It is therefore not an entry swap in the
held directory and contradicts the documentation's stronger statement that a path-level swap
after validation cannot redirect removal. The residual disclosure must be broadened or, as the
required repair specifies, this path must be closed before a PASS can be issued.

## Adversarial coverage

| Axis | Case exercised | Result |
| --- | --- | --- |
| Path / identity | Non-directory scope root; root rename plus same-inode hard link | Both counterexamples fail current behavior. |
| Partial reads / permissions | Real mode-`000` Claude `projects` root and unreadable descendant/companion regressions | Original repair holds; exit `4` and no deletion. |
| Links / mounts | Existing intermediate-symlink deletion regression; hard-link replacement probe | Symlink refusal holds; hard-link root replacement is unsafe. |
| Provider layout drift | Missing/symlink roots are known-empty; regular-file root exposed as unhandled malformed layout | FAIL for present non-directory root. |
| Concurrency | Deterministic identity-observer interleaving replaces root between revalidation and system mutation | E21-S07 FAIL. |
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

## CR4 Safety Verdict — E21-S07

- Change: Handle-relative unlink for confirmed deletion
- Risk: CR4
- Commit/PR: `667ce50c8944603b2990b38c754423691eed8f61`
- Independent verifier: Codex
- Date: 2026-09-21

### Verdict

`FAIL`

### Safety surface changed

The mutation seam changed irreversible deletion from path-based removal to handle-relative
`unlinkat`. The parent handle is acquired after outer revalidation, leaving a root-object swap
that can redirect the descriptor acquisition.

### Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | Revalidation binds the identity actually mutated | Root/hard-link probe returned `Succeeded` after removing a replacement-root entry | FAIL |
| SI-016 | Mutation executes the sealed root/target plan | Original planned directory entry survived while a replacement-root entry was removed | FAIL |
| SI-019 | The sole mutation path remains evidence-gated | Boundary is singular, but its current capability handoff permits a redirected mutation | FAIL |
| SI-020 | Irreversible action is explicit and stronger-gated | Delete was explicit but executed against the replacement root | FAIL |

### Adversarial cases

The same-name swap and intermediate-symlink regressions pass; the root rename plus hard-link
case falsifies the broader post-validation no-redirection claim.

### Differential / compatibility evidence

Not applicable to this OS-level mutation race. Mutation-boundary structural check passes but
cannot prove the retained capability is the approved root object.

### Known residual risks

The disclosed `fstatat`/`unlinkat` child-entry race remains. The reproduced root-binding race is
additional and is not acceptable as an undisclosed residual.

### Rollback / recovery

The reproduction left the original artifact intact but removed an external hard-link entry; that
entry cannot in general be restored automatically. Stop destructive use on this path until the
approved-root handle is retained/revalidated through mutation.

### Owner decision

`PENDING — verifier recommends REJECT until the required repair and an independent rerun pass.`

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
