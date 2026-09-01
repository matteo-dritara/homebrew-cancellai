# Evidence Packet - E07-S07

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 1 - `FAIL` (`project/evidence/E07-S07-VERIFIER-REVIEW.md`,
  `project/evidence/E07-S07/SAFETY_VERDICT.md`); round 2 - pending (CR4, see "Why this stops at
  `ready_for_review`" below)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-sealedfs/src/lib.rs` (new crate),
  `rust/crates/cancellai-cli/src/main.rs` (`configure_claude_retention`, `cmd_configure`),
  `docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`

## Outcome

PARTIAL

## Scope

Repairs the exact defect the round-1 independent verifier review found and rejected: `configure`'s
root-swap TOCTOU (see "Reproduction / counterexample" in `project/evidence/
E07-S07-VERIFIER-REVIEW.md`). That review found the story's first closure session's `configure`
re-check (`cmd_configure`'s `roots::is_symlink`, still present) insufficient - it and the raw
path-based read/write/rename that followed it in `configure_claude_retention` were separate
syscalls, so a root swapped to a symlink in the gap between them redirected the write outside the
approved root, violating SI-002/SI-003/SI-013/SI-019.

This session adds `cancellai-sealedfs` (ADR-0017), a new, unsafe-isolated workspace crate
providing `SealedRoot`: a directory opened exactly once with `O_NOFOLLOW` and retained, with every
subsequent child read/write/rename issued via `openat`/`renameat` against that one descriptor
rather than the original path. `configure_claude_retention` is rewritten against this capability
in place of the previous raw `std::fs`/path sequence. The Unix-symlink classification-time/
execution-time repair and the disclosed Windows-junction-fixture residual from the story's first
closure session (recorded in `project/evidence/E06-S01/EVIDENCE.md` and this packet's prior
revision) are unchanged by this session and not re-litigated here.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - A default-named Claude or Codex root that is itself a link/reparse point is inspection-only and cannot be cleaned or configured | Unchanged from the prior revision for the static (already-a-symlink-at-classification) case; unaffected by this session's fix, which addresses the *drift* case (AC2 below). Windows NTFS-junction fixture remains the same disclosed residual as before. | PARTIAL |
| AC2 - Root identity and containment revalidation reject link/reparse drift at plan and execution time on every supported platform | Unix: `cancellai_sealedfs::SealedRoot::establish` binds `configure`'s root via a retained `O_NOFOLLOW` directory descriptor; every child operation is issued against that descriptor, not the original path, closing the round-1 finding by construction rather than by a tighter re-check. Proven by a deterministic adversarial unit test (`cancellai-sealedfs::unix_impl::tests::establish_rejects_a_root_swapped_to_a_symlink_after_final_validation_but_before_the_bind`) that reproduces the verifier's exact scenario via a test-only hook between the presence check and the authoritative open, and proves an outside sentinel is never created. Non-Unix: `SealedRoot::establish` always fails closed (`SealError::Unsupported`) - `configure` now refuses outright there rather than attempting an unprotected path-based write, matching `clean`'s existing `ApprovedRoot`/`IdentityObserver::Unsupported` posture. `clean`'s own root-drift re-check (`establish_verified_root`) is unchanged; it did not have this gap (E03-S05's `ApprovedRoot::establish`/`bind` already canonicalize-and-bind before mutation). | PASS (Unix, drift case); FAIL->PASS (Windows/other non-Unix, now fails closed instead of writing unprotected) |
| AC3 - Unix symlink and Windows junction/reparse adversarial fixtures prove no provider mutation reaches the link target | Unix: the new deterministic root-swap test above, plus regression coverage - `establish_refuses_a_root_that_is_already_a_symlink`, `write_new_child_atomically_refuses_a_pre_planted_symlink_at_the_temp_name` (temp-name race, matching the pre-existing `O_EXCL` protection), `read_child_to_string_follows_a_preexisting_symlink_child_matching_prior_behavior` (proves the already-accepted settings.json-symlink read/write split from E06 round 1 is preserved, not silently tightened) - all in `cancellai-sealedfs`'s own unit test suite; the full `cli_behavior.rs` integration suite (18 tests, including `configure_never_writes_through_a_preexisting_settings_json_symlink_to_an_outside_file` and both `*_when_home_dot_claude_is_itself_a_symlink` tests) still passes end-to-end against the real built binary. Windows: unchanged from the prior revision - no genuine NTFS junction fixture exists yet (same disclosed residual); the *new* Windows behavior this session adds (fail-closed via `Unsupported`) is proven by `cancellai-sealedfs::fallback_impl::tests::establish_always_fails_closed_on_a_platform_with_no_verified_handle_capability`, cross-compile-clippy-verified for `x86_64-pc-windows-gnu` (no Windows runner in this environment; executes for real on this repo's Windows CI matrix on the next push). | PARTIAL |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Root swapped to a symlink after classification, before the write | `SealedRoot::establish`'s `O_NOFOLLOW` open is the positive bound; the adversarial unit test above proves it refuses even when the swap happens after `establish`'s own presence check | PASS |
| SI-003 | Child name crafted to escape the sealed directory (`../`, absolute path, embedded `/`) | `validate_child_name_rejects_escaping_and_malformed_names` (`cancellai-sealedfs`) | PASS |
| SI-013 | Identity drift between the last check and the mutation | Same adversarial unit test as SI-002 - the descriptor, not a re-checked path, is what is bound for the operation's full duration | PASS |
| SI-019 | A vendor-configuration write reaching outside its approved root without an authority-verified capability | `configure_claude_retention` now routes exclusively through `SealedRoot`; `scripts/check_mutation_boundary.py` is unaffected (this is a vendor-settings write, not a provider-artifact deletion, per that invariant's own documented scope) | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

All green in this environment (macOS, native toolchain). `cargo test --workspace` includes the
new `cancellai-sealedfs` crate's 8 unit tests (7 Unix, plus a non-Unix fallback test that does not
execute natively here) and all pre-existing suites unchanged, including the full `cli_behavior.rs`
integration suite. No Windows runner is available in this environment; the Windows-specific
fail-closed behavior is a fresh `#[cfg(not(unix))]` code path this session added and has not been
executed on real Windows CI yet - disclosed as a residual below, matching this story's own
pre-existing disclosure pattern for the Windows-junction gap.

## Compatibility

- `cancellai-sealedfs`'s Unix implementation is exercised natively in this environment (macOS).
  Its non-Unix fallback (`Unsupported`, always fails closed) is new code not yet executed on a
  real non-Unix CI runner - see Residual risks.
- `libc` is scoped to `[target.'cfg(unix)'.dependencies]`; a non-Unix build does not compile or
  link it at all.

## Performance / operability

- `configure` now performs the same syscall shapes as before (one directory open, one child
  read, one child create, one rename) but via `openat`/`renameat` instead of path-based
  equivalents - no additional syscalls, no measurable change.

## Documentation updated

- `docs/adrs/0017-sealed-root-handle-for-configuration-writes.md` (new): the full decision,
  alternatives considered, and consequences for this repair.
- `docs/architecture/TARGET.md`: crate list gains `cancellai-sealedfs`; new prose in the
  `MutationExecutor`/SI-019 discussion cross-referencing this crate and its relationship to that
  module's own disclosed, different, unlink-specific residual.
- `docs/architecture/PLATFORM_MODEL.md`: new "`configure`'s TOCTOU: a re-checked path is not
  enough, only a retained handle is" subsection under "Default-root authority never rests on a
  lexical name alone".
- `docs/CLI_RUST.md`: `configure` section and "Known gaps" both updated for the new capability
  and the Windows fail-closed behavior change.
- `docs/security/SAFETY_INVARIANTS.md`: SI-002, SI-003, and SI-013 each gained an implementation
  cross-reference to `cancellai-sealedfs::SealedRoot`.
- `CHANGELOG.md`: `[Unreleased]` entry recording the user-visible behavior change (Windows
  `configure` now refuses outright) alongside the security repair.

## Residual risks

- **Windows fail-closed path is cross-compile-verified only, not executed**: no Windows runner is
  available in this environment. It will run for the first time on this repo's actual Windows CI
  matrix on the next push - not yet observed by this session, same disclosure shape as the prior
  revision's Windows symlink tests.
- **NTFS junction-specific fixture (carried over from the prior revision, unchanged by this
  session)**: still not empirically proven; see the AC1/AC3 rows above.
- **`SealedRoot::establish`'s create-if-absent window**: a small gap remains between
  `create_dir_all` succeeding and the subsequent `O_NOFOLLOW` open, inherent to `mkdir` having no
  atomic "create and return a handle" primitive on any targeted platform - disclosed in ADR-0017's
  "Negative / cost" section as a materially narrower window than the one this session closes, not
  claimed as fully closed.
- **`cancellai-platform::mutation`'s own disclosed unlink-race residual is unrelated and not
  closed by this session** - a different operation (deletion, not configuration write), a
  different crate; ADR-0017 records this explicitly so it is not mistaken for having been
  addressed here.

## Why this stops at `ready_for_review`

This is a CR4 story that a round-1 independent verifier review already rejected once. `AGENTS.md`'s
constitutional non-negotiables require CR4 work to close only with independent verification and
an owner-visible Safety Verdict, and explicitly prohibit the executor from writing its own CR4
Safety Verdict or marking its own work `verification`/`done`. This session's repair addresses the
round-1 finding directly (see the Safety Evidence table above) but does not substitute for the
independent re-review that finding requires; per `docs/development/AGENT_PROTOCOL.md`'s bounded
review process, this is round 2 of at most two - the story returns to `ready_for_review` for that
round, with `project/evidence/E07-S07-VERIFIER-REVIEW.md` and `project/evidence/E07-S07/
SAFETY_VERDICT.md` left unchanged as the round-1 historical record.

## Verifier verdict

Pending (round 2). Not self-graduated to `verification` or `done`.
