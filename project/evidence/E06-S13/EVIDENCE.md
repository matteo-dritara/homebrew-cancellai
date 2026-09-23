# Evidence Packet - E06-S13

- Commit/PR: the E06-S13 commit on `main`
- Executor: Claude
- Independent verifier: pending - reviewed with the rest of E06's cutover stories
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS - native Windows evidence: `rust.yml` run 35891329445 on 85fed2f, every leg green, including `quality`, `cli-stable-channel` and `kill-harness` on windows-latest

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a confirmed Windows plain file is deleted by clean | Two gates stood in the way, and the first Windows CI run of this story found the second. (1) `delete_operation_for` maps `IdentityToken::Windows { kind: File }` to `DeleteFile`, which `cancellai-platform` executes through E20-S05's `confirmed_delete_file` (open-time identity on a retained handle, a fresh re-check, handle-relative `NtCreateFile` delete, delete-pending corroboration). `a_windows_plain_file_is_deletable`. End to end: the CLI deletion tests (`clean_yes_deletes_...`, `keep_latest_protects_...`, the two E06-S10 tests) and the kill harness's deletion cases are no longer Unix-gated and run on `windows-latest` in `rust.yml`. | PASS on host; Windows: first CI run of this commit |
| AC2 - directory, reparse point, unconfirmed kind refused | `a_windows_directory_reparse_point_or_unknown_kind_is_refused`; the Windows observer classifies any `FILE_ATTRIBUTE_REPARSE_POINT` object as `Symlink` (`identity.rs`), which stays refused. | PASS |
| AC3 - a swap between planning and deletion does not reach the substitute | Unchanged primitive: E20-S05's Windows `confirmed_delete_file_inner` refuses on any volume/file-index/last-write mismatch at open time and immediately before the handle-relative delete, with native swap/reparse fixtures verified in E20-VERIFIER-REVIEW-ROUND2. The executor still revalidates the plan (`revalidate`) before selecting the operation. | PASS (inherited, independently verified in E20) |
| AC4 - kill harness and CLI deletion tests run on Windows | `#[cfg(unix)]` removed from those tests and from the harness's deletion case; tests that need `chmod` or Unix symlinks stay Unix-only. | PASS (run 35891329445) |

## Round-3 repair (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND3.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A hard-linked Windows file was admitted for deletion, contrary to AC2 | A file with more than one link is refused at open time, on Windows (`nNumberOfLinks`, now carried in `WindowsFileFacts`) and on Unix (`st_nlink`); the Unix post-unlink check stays at a link count of 0. Refusals surface as a failed action with the reason, like the primitive's other refusals. | `a_hard_linked_session_is_refused_and_both_names_survive` (every platform, stable channel); `execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy` stays green |

## Method defects

- **What happened**: the executor first argued the hard-link criterion was wrong, since unlinking a name never removes data another link reaches. The owner amended AC2 on that argument, and the executor weakened the Unix post-unlink check from "0 links" to "one link fewer" (commit eb0638f). CI then failed `execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy`: at deletion time a legitimate second link and a decoy link planted beside a swapped-out root are the same fact, and the 0-link check was what caught the decoy. eb0638f was reverted, the original criterion restored, and the owner chose to refuse multi-link files (option A). **Prevented by**: the executor ran the platform and CLI suites after the change but not `cancellai-safety`'s, where the regression that names this attack lives; `risk-gate` lists the workspace suite for CR4, and running only the touched crates' tests was the shortcut. **Disposition**: accepted 2026-09-23 - reverted and repaired as above; proposed: `risk-gate` should state that a change to a mutation primitive runs every crate that tests through it, not only the crate it lives in.


- **What happened**: three stacked defects kept Windows from deleting, each hidden by the one before it: the executor refused every Windows identity; then the provider-layout observation failed closed on Windows; then E20-S05's own delete primitive compared the raw `FILETIME` against the token's sub-second remainder and refused every target. The third was invisible to E20-S05's tests (and its independent PASS) because the test helper assembled the expected token by hand with the same unit mistake, instead of taking it from `SystemIdentityObserver`. Each layer was only reachable once the one above it was fixed, and each was found by real Windows CI, never locally. **Prevented by**: none exists - no gate requires a mutation-path test to take its expected identity from the production observer rather than a hand-built value, and no end-to-end Windows deletion test existed while the executor refused. **Disposition**: accepted 2026-09-23 - the helper now uses `SystemIdentityObserver`, and the CLI deletion, containment and kill-harness tests run end to end on Windows CI; proposed: an `adversarial-cases` axis asking whether a test's expected value is produced by the code under test's own production path.

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-017 | A non-file Windows identity admitted for deletion | Only `FileKind::File` maps to an operation; unit test enumerates the other three kinds | PASS |
| SI-019 | A second deletion path | No new mutation code; the executor only selects the existing operation. `check_mutation_boundary.py` passes | PASS |
| SI-013 | Stale plan reaching Windows deletion | `revalidate` runs before `delete_operation_for`, unchanged | PASS |

## Verification Commands

```text
cargo test -p cancellai-safety                                                      -> 205 passed
cargo test -p cancellai-cli                                                         -> pass (host)
cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -D warnings -> clean
python3 scripts/check_mutation_boundary.py check                                    -> OK
```

## Residual risks

- **Native Windows behaviour is proven only by CI**, not on a local machine; the new `unsafe` (`GetFileInformationByHandleEx` enumeration) is lint-checked for Windows locally and executed only there.
- **Directory enumeration buffer**: entries are parsed from a copy with bounds-checked reads; a malformed entry is an error, never a truncated listing.
- **Hard links**: a Windows file with several links shares one file index; deleting a planned
  name removes that link only, as on Unix. Not separately fixture-tested.

## Verifier verdict

pending
