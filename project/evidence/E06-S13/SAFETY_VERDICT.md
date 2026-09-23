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
