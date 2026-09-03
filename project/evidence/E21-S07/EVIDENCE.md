# Evidence Packet - E21-S07

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR4
- Spec version/commit: ADR-0017 (E21-S07 extension); `docs/audits/2026-09-03-CODE_REVIEW.md`,
  findings `CR-TE-05`, `CR-TE-11`

## Outcome

PASS

## Scope

Replaces detection with prevention on the delete path, using the no-follow handle capability
this workspace already reviewed and shipped for configuration writes.

`cancellai-platform::mutation` justified its unclosed unlink race by stating that the required
capability did not exist here - no reviewed FFI dependency, `unsafe` forbidden. ADR-0017
superseded that premise and nobody revisited the text. The result was an inverted risk ordering:
writing one JSON key into a vendor settings file was protected by a retained no-follow handle,
while irreversibly deleting a user's file was not. E21-S01 corrected the note; this story closes
the gap it described.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - `cancellai-sealedfs` gains an `unlinkat`-based child removal, same SAFETY discipline, fail-closed off-Unix | `bind_existing` (walk handle-relatively, never create the leaf - creating a directory as a side effect of deleting a file is not an operation anything should perform by accident) and `unlink_child_matching_unix_identity` (`fstatat` with `AT_SYMLINK_NOFOLLOW`, then `unlinkat(…, 0)`, both against one held descriptor). Each `unsafe` block carries its own `// SAFETY:`. The non-Unix stubs return `SealError::Unsupported`, matching `configure`'s existing posture. | PASS |
| AC2 - the confirmed deletion issues its unlink relative to a held directory descriptor | `confirmed_delete_file_inner` binds the target's parent through `SealedRoot::bind_existing` and removes the child through the sealed handle. The pre-existing open-file-descriptor check, the fresh path re-check and the post-unlink link-count check all remain: this is a third, handle-relative confirmation, not a replacement for the other three. | PASS |
| AC3 - `check_mutation_boundary.py` still finds exactly one permitted file; `unsafe` still lives in exactly one crate | `python3 scripts/check_mutation_boundary.py check` passes. `git grep unsafe rust/crates` returns nothing outside `cancellai-sealedfs` except prose. `cancellai-platform` gains a dependency on `cancellai-sealedfs`; the graph stays acyclic. | PASS |
| AC4 - the `MutationOperation` variants no caller requests are identity-confirmed or removed | Removed. `Quarantine` (a bare `fs::rename`) and `DeleteDirectoryTree` (a bare `fs::remove_dir_all`) were unconfirmed, unreachable, and sitting in the one file the workspace permits to delete. `MutationOperation` now has a single variant. `mutation_executor` already refused both action classes upstream, so nothing user-visible changed; re-adding either is E12's job, with the confirmation technique it needs. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | The entry at the confirmed name is swapped between validation and removal | `the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode`: refused, and the replacement's content is asserted intact | PASS |
| SI-003 / SI-013 | The target is reached through a symlinked intermediate component | `a_symlinked_intermediate_component_refuses_the_delete`: refused with "without following a link", and the target survives | PASS |
| SI-013 | The target is swapped before the descriptor is opened | `confirmed_delete_rejects_a_target_already_swapped_before_open` (pre-existing, still passing) | PASS |
| SI-013 | The target is swapped mid-flight, after the open-time check | `confirmed_delete_detects_a_target_swapped_between_open_and_unlink` (pre-existing, still passing) | PASS |
| SI-016 / SI-019 | Mutation still routes through the one boundary | `check_mutation_boundary.py` passes; `cancellai-cli` still cannot name `SystemMutationExecutor` | PASS |
| SI-020 | Deletion still works when lawful | Native `clean --yes --allow-running` deletes through the handle-relative path, exit 0 | PASS |

## Verification Commands

```text
$ cargo test --workspace                 318 passed, 0 failed
$ cargo clippy --workspace --all-targets --all-features -- -D warnings    clean
$ cargo deny check                       advisories ok, bans ok, licenses ok, sources ok
$ python3 scripts/check_mutation_boundary.py check    OK
$ python3 scripts/check_rust_workspace.py check       OK (13 crates, acyclic)
$ python3 scripts/rust_python_parity.py check         12 NORMATIVE fixtures, both scenarios, OK
```

The CLI's own end-to-end integration tests (`cli_behavior.rs`) perform real deletions and pass
unchanged, which is the strongest available evidence that the production path works: they already
canonicalize their temp home, for the same reason this story's own test helper now does.

## Compatibility

- **A real, intended behaviour change**: a provider root reached through a symlinked path
  component can no longer be cleaned. That is the rule E07-S09 already established for root
  *establishment*, now holding at the moment of mutation. `cancellai-cli` proves the default root
  link-free before establishing it, so the production path already met this bar.
- Surfaced by a genuine test failure rather than by inspection: on macOS `std::env::temp_dir()`
  is `/var/folders/…` and `/var` is a symlink, so `mutation.rs`'s own `TempDir` helper had to
  meet the same bar. It now canonicalizes, and the refusal it exposed is pinned by its own test
  so canonicalizing hides nothing.
- Non-Unix is unchanged: `confirmed_delete_file` already returned an error there.

## Performance / operability

- One extra directory-walk per deleted file, proportional to path depth. Deletion is dominated by
  the syscall itself and by the identity checks that already ran.

## Documentation updated

- `docs/adrs/0017-…md` - extended with what this closes and, explicitly, what it does not.
- `docs/architecture/PLATFORM_MODEL.md` - handle-relative mutation, and the user-visible
  consequence stated plainly.
- `CHANGELOG.md`.

## Residual risks

- **Not fully closed, and the ADR says so.** POSIX has no "unlink only if this name still points
  at this inode" primitive, so `fstatat` and `unlinkat` remain two syscalls. What is closed is the
  *directory* being swapped; what remains is an attacker with write access to that specific
  directory replacing the entry between them. Strictly smaller than the path-based surface, and
  the held file descriptor's post-unlink link-count check still detects such a swap.
- Non-Unix platforms cannot delete at all, unchanged, and now with one more reason.
- This packet is executor self-assessment. CR4 requires an independent adversarial pass and an
  owner-visible Safety Verdict.


## Round-1 independent review: PASS_WITH_RESIDUALS

`project/evidence/E21-S07/SAFETY_VERDICT.md` passed all four invariants and confirmed the
documented `fstatat`/`unlinkat` residual is described accurately rather than overclaimed. It
also independently checked the consequences of the intermediate-link refusal: a symlinked
`$HOME` was already non-destructive under E07-S09, a relocated non-symlink home works normally,
and a Unix bind mount is not a reparse traversal and is not newly refused.

The story was blocked only by its failed dependency E21-S01, now repaired. No code change was
required in this round.

## Verifier verdict

`PASS_WITH_RESIDUALS` (round 1) - see project/evidence/E21-VERIFIER-REVIEW.md
