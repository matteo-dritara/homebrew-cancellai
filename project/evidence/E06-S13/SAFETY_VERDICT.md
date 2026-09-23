# Safety Verdict - E06-S13

Verifier: Codex
Brief-Checksum: 867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121

- Reviewed commit: `85fed2f7c66baf9da3ed79f1bb870f4277eec434`; risk CR4.
- Surface: Windows `FileKind::File` admission, handle-bound root listing, and `NtCreateFile`/`SetFileInformationByHandle` deletion.

## Verdict

`FAIL`

## Invariants

| Invariant | Evidence | Result |
| --- | --- | --- |
| SI-013 | The path is observed at execution, rechecked before deletion, and final Windows child open is relative to a held, no-follow parent. Yet two hard-link names of the same object have identical compared volume/index/mtime; no link count is checked. The wrong same-object name can be removed. | FAIL |
| SI-017 | Reparse points, directories and unknown kinds remain refused. Windows identity uses volume serial/file index, but `nNumberOfLinks` is discarded, despite the AC requiring hard-linked files to be refused. | FAIL |
| SI-019 | Mutation continues through the one safety executor; `check_mutation_boundary.py` passes. Windows plain-file admission is too broad within that boundary. | FAIL |

## Adversarial cases and unsafe review

- `windows_sealed.rs` lines 61–110 validate a single child component and refuse UNC/device paths; lines 149–245 keep the parent handle, wide name, `OBJECT_ATTRIBUTES` and `IO_STATUS_BLOCK` alive across `NtCreateFile`, translate NTSTATUS and transfer exactly one successful HANDLE to `File`. Directory walk lines 324–375 rejects reparse components and gives listing rights only to the bound final root.
- Final deletion lines 389–424 opens one child relative to the held parent with `FILE_OPEN_REPARSE_POINT`, checks reparse and volume/index, then marks the open handle pending for deletion. `is_delete_pending` lines 432–457 queries an initialized `FILE_STANDARD_INFO` on the retained handle. These pointer lifetimes and buffer sizes are consistent with the Windows bindings; no memory-safety defect was found in the reviewed unsafe blocks.
- Listing lines 589–670 uses an aligned 64 KiB buffer, copies it before bounds-checked field/name parsing, and returns errors rather than an empty listing. The 1,500-entry Windows CI test passed. A kernel-malformed offset can make this path error or panic; ordinary untrusted filenames cannot write the kernel's structure fields.
- Hard-link counterexample: `WindowsFileFacts` omits `BY_HANDLE_FILE_INFORMATION.nNumberOfLinks`; the safety operation admits all Windows `FileKind::File`; the final check compares only `(volume_serial_number,file_index)`. An initially hard-linked stale session, or a same-object hard-link swap, passes these checks instead of being refused. Existing Windows identity tests prove hard links share the file index. This exact new fixture was NOT RUN natively on the macOS host.
- `gh run view 35891329445` confirms SHA `85fed2f7…`: Windows `quality`, `kill-harness` and `cli-stable-channel` jobs succeeded. They do not include the missing hard-link delete regression. Local Windows-target Clippy passed.

## Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy/tests, Windows-target Clippy | PASS |
| Stable-channel CLI suite with kill-points, local macOS | PASS on full rerun |
| Exact-main Windows quality, kill-harness, cli-stable-channel | PASS |
| `cargo deny --offline check` with writable advisory DB copy | PASS |
| Native Windows hard-link deletion regression | NOT RUN; no Windows host available to this verifier |

## Required repair

Carry link count from `GetFileInformationByHandle` into the Windows identity/confirmation path and refuse link count above one both when planning and at the final open handle. Add native Windows tests for an initially hard-linked eligible session and a second link created or swapped in after planning. Recheck these alongside the directory, reparse and ordinary-file cases before cutover.

### Owner decision

PENDING

Do not accept this story or infer hard-link safety from the green CI jobs until the explicit AC is met.

FAIL

## Round 4

Verifier: Codex
Brief-Checksum: 867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121

- Reviewed commit: `a6d9d316209fa6ad7afe95fcb8661b4957058ea6`.
- Scope: E06-S13 only, answering the round-3 F-03 hard-link repair and its effects.

### Invariants

| Invariant | Independent evidence | Result |
| --- | --- | --- |
| SI-013 | The first open handle rejects an initial multi-link file, but a link added after that check reaches Unix `unlinkat`; the post-unlink check returns an error only after one name is removed. On Windows, the final handle's link count is observed but not checked. | FAIL |
| SI-017 | Windows volume/file-index and reparse classification remain native and kind-bounded. `WindowsFileFacts.number_of_links` is now carried, but `windows_sealed.rs::unlink_child_matching_windows_identity` omits it at disposition time. | FAIL |
| SI-019 | The production call remains through `cancellai-safety::mutation_executor` and `cancellai-platform::mutation`; no alternate CLI mutation path was found. The sole boundary still admits the late multi-link deletion. | FAIL |

### Adversarial cases

- A separately constructed stable-release CLI fixture with two initial names returned exit 3 and kept both names byte-identical on macOS. This confirms only the initial check.
- A temporary Unix platform test inserted a hard link via `confirmed_delete_file_inner`'s callback after its first-handle check. The operation returned `Err`, but the test failed at `refusal must preserve the planned name`; the planned path had already been unlinked. The test was removed and `git diff` showed no product-source change.
- The Windows final `NtCreateFile` child handle is the one sent to `SetFileInformationByHandle(FileDispositionInfo)`. Its `WindowsFileFacts` are checked for reparse and volume/index, not `number_of_links`; a late second link has identical compared identity. Native Windows execution of this new fixture is **NOT RUN** on the macOS host.
- Directory, reparse, unknown-kind, and same-object root-decoy guards were traced. The original Unix zero-link postcondition is present after revert `835ec0f`; do not weaken it as `eb0638f` did. The ordinary and initial hard-link CLI tests pass, but neither exercises a link appearing after the first check.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests | PASS |
| Stable-channel CLI suite with kill-points | PASS |
| Latest-main Windows quality, kill-harness and stable CLI at `a6d9d316…` | PASS; `gh run view 35896648462` |
| Previous `b554eab…` Windows quality, kill-harness and stable CLI | PASS |
| Native Windows late-link counterexample | NOT RUN: no local Windows host |
| Temporary deterministic Unix late-link counterexample | FAIL as expected: planned name removed despite reported error; test removed |

### Required repair

Check multi-link state at the final Unix held-parent check and on the exact Windows child handle used for disposition. Add deterministic late-link tests on both platforms and assert that both names survive refusal. Keep the Unix post-unlink zero-link check and E21 root-decoy test. If AC2 cannot be guaranteed across the final check and mutation, escalate the remaining race to the owner explicitly.

### Owner decision

PENDING

FAIL

## Round 5

Verifier: Codex
Brief-Checksum: 867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121

- Reviewed repair commit: `7f6c554b2a25e4dcda16b11e5aa912d917499e74`.
- Scope: E06-S13 only. This is the owner-authorized extra independent pass after rounds 3 and 4 failed.

### Invariants

| Invariant | Independent evidence | Result |
| --- | --- | --- |
| SI-013 | The Unix final held-parent `fstatat` checks device/inode and link count before `unlinkat`; Windows checks volume/file index and link count from the exact child handle used for delete disposition. The deterministic late-link test and E21 renamed-root hard-linked-decoy regression pass on Unix. The final check and OS mutation remain separate calls. | PASS_WITH_RESIDUALS |
| SI-017 | Windows uses native `BY_HANDLE_FILE_INFORMATION.nNumberOfLinks`, volume/file index and reparse classification. Only plain files enter the delete operation; directories, reparse points and unconfirmed kinds remain refused. Windows-target Clippy and exact repair-commit native Windows CI pass. | PASS_WITH_RESIDUALS |
| SI-019 | `check_mutation_boundary.py check` passes. The production CLI still reaches deletion only through the safety executor and platform confirmed-delete path; no alternate multi-link delete route was found. | PASS |

### Adversarial cases

- Independent stable-release CLI fixture: a stale, synthetic Claude session with two hard-link names returned exit 3, reported links, and kept both names byte-identical. Removing the extra name and rerunning returned exit 0 and deleted the eligible single-link session.
- The Unix `a_hard_link_added_after_the_open_time_check_is_refused_before_any_name_is_removed` test passed: its callback adds a second link after the first open; the final held-parent check refuses before either name is removed. The matching Windows test ran in the exact repair-commit Windows quality job. Native Windows execution by this verifier was **NOT RUN** because there is no local Windows host.
- `execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy` passed. The original Unix `after.nlink() == 0` postcondition remains; `eb0638f`'s weakened postcondition was not restored.
- Source trace of `NtCreateFile` and `SetFileInformationByHandle` confirms `number_of_links` is read on the handle whose delete disposition is then set. Unix `fstatat` reads link count on the held parent immediately before `unlinkat`.
- Residual: a link created after the final read but before the deletion syscall can still make the precheck stale. On Unix, the post-unlink check reports failure only after a name might be removed. The Unix final two-syscall leaf-name interval was owner-accepted in E06-S06's Safety Verdict; the analogous Windows link-count interval is stated here for an explicit owner CR4 decision. This pass does not treat post-delete detection as refusal preserving both names.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests | PASS |
| Stable-channel CLI suite with kill-points | PASS |
| Focused Unix late-link and E21 hard-linked-decoy tests | PASS |
| Independent stable-release CLI two-link/single-link fixture | PASS |
| Python tests, cutover benchmark after stable release build | PASS |
| `check_mutation_boundary.py` | PASS |
| Exact repair-commit Windows `quality`, `cli-stable-channel`, `kill-harness` (`gh run view 35898569202`) | PASS |
| Latest-main Windows CI at review time | UNKNOWN: newer main SHA `184acaa5…` queued |
| Native Windows late-link run by this verifier | NOT RUN: no Windows host |
| Post-evidence governance, handoff, process and process-metrics checks | PASS; process check reports the owner-recorded E06 round-count exception |

### Owner decision

PENDING

PASS_WITH_RESIDUALS

## Owner decision - latest round (2026-09-24)

`ACCEPTED`

Owner note (recorded by the executor at the owner's direction): the latest independent verdict
and its stated residuals are accepted; E06-S13 may close.
