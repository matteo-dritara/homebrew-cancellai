# Evidence Packet - E06-S13

- Commit/PR: the E06-S13 commit on `main`
- Executor: Claude
- Independent verifier: pending - reviewed with the rest of E06's cutover stories
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS (native Windows evidence from CI, see AC1)

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a confirmed Windows plain file is deleted by clean | Two gates stood in the way, and the first Windows CI run of this story found the second. (1) `delete_operation_for` maps `IdentityToken::Windows { kind: File }` to `DeleteFile`, which `cancellai-platform` executes through E20-S05's `confirmed_delete_file` (open-time identity on a retained handle, a fresh re-check, handle-relative `NtCreateFile` delete, delete-pending corroboration). `a_windows_plain_file_is_deletable`. End to end: the CLI deletion tests (`clean_yes_deletes_...`, `keep_latest_protects_...`, the two E06-S10 tests) and the kill harness's deletion cases are no longer Unix-gated and run on `windows-latest` in `rust.yml`. | PASS on host; Windows: first CI run of this commit |
| AC2 - directory, reparse point, unconfirmed kind refused | `a_windows_directory_reparse_point_or_unknown_kind_is_refused`; the Windows observer classifies any `FILE_ATTRIBUTE_REPARSE_POINT` object as `Symlink` (`identity.rs`), which stays refused. | PASS |
| AC3 - a swap between planning and deletion does not reach the substitute | Unchanged primitive: E20-S05's Windows `confirmed_delete_file_inner` refuses on any volume/file-index/last-write mismatch at open time and immediately before the handle-relative delete, with native swap/reparse fixtures verified in E20-VERIFIER-REVIEW-ROUND2. The executor still revalidates the plan (`revalidate`) before selecting the operation. | PASS (inherited, independently verified in E20) |
| AC4 - kill harness and CLI deletion tests run on Windows | `#[cfg(unix)]` removed from those tests and from the harness's deletion case; tests that need `chmod` or Unix symlinks stay Unix-only. | PASS pending CI |

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
