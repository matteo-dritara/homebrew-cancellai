Review-Scope: epic
Round: 5
Verifier: Codex

# E06 independent verifier review — round 5

- Scope: **E06-S13 only**, the owner-authorized extra pass. E06-S08 and E06-S10 were not judged.
- Reviewed tree: `7f6c554b2a25e4dcda16b11e5aa912d917499e74` (clean before this evidence edit), 2026-09-23. The committed verifier brief was read directly; executor assertions were treated as claims to test.
- E06-S13 Brief-Checksum: `867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121`.

## Verdict

| Story | Verdict | Basis |
| --- | --- | --- |
| E06-S13 | **PASS_WITH_RESIDUALS** | The round-3 initial multi-link refusal and round-4 late-link repair are confirmed. The Unix final held-parent `fstatat` rejects link count other than one before `unlinkat`; the Windows final child handle rejects it before disposition on that same handle. Focused Unix late-link and E21 hard-linked-root-decoy tests pass. An independent stable-release CLI fixture refused a two-link stale session with exit 3, preserved both names and their bytes, then deleted the same session with exit 0 after the extra name was removed. Exact repair-commit Windows CI passed quality, kill harness, and stable CLI jobs, including the Windows late-link unit test. No local Windows host was available. |

## Round-3 and round-4 repair checks

| Required repair | Independent result |
| --- | --- |
| Carry Windows `nNumberOfLinks` and refuse initial multi-link files on both platforms | Confirmed in `windows_identity::observe_identity_of_handle` and both platform `confirmed_delete_file_inner` open-time checks. The separate stable-release CLI fixture returned exit 3 and left both hard-link names intact. Removing the second name let the ordinary single-link deletion succeed. |
| Recheck at final mutation seam, including a link introduced after the first check | Unix `SealedRoot::unlink_child_matching_unix_identity` reads `st_nlink` in its final held-parent `fstatat` and refuses any count other than one before `unlinkat`. Windows `unlink_child_matching_windows_identity` obtains `WindowsFileFacts` from the *same* `NtCreateFile` child handle passed to `SetFileInformationByHandle` and refuses `number_of_links != 1` before disposition. The deterministic Unix late-link test passed locally; the analogous Windows test ran in the successful Windows quality job at the repair SHA. |
| Preserve both names on refusal; retain original Unix zero-link postcondition and E21 root-decoy regression | The focused Unix late-link test asserted both names and bytes survive. The independent CLI fixture did the same. `confirmed_delete_file_inner` still requires `after.nlink() == 0`; the focused `execute_refuses_a_root_renamed_and_replaced_with_a_hardlinked_decoy` test passed. No alternate `DeleteFile` path bypassing these checks was found. |
| Continue refusing directories, reparse points, and unconfirmed kinds; run native Windows CLI and kill harness | `delete_operation_for` admits only `FileKind::File` for both identity variants. Windows `nt_open_child` uses `FILE_NON_DIRECTORY_FILE` and `FILE_OPEN_REPARSE_POINT`; the final handle checks reparse status. The exact repair-commit Windows `quality`, `cli-stable-channel`, and `kill-harness` jobs all concluded success. |

**Blocking findings and required repairs:** none for E06-S13 in this pass. F-03 and F-05 are repaired at their specified seams. The remaining read-to-mutation window below is disclosed for the owner's CR4 decision; a post-delete error is not represented as a refusal that preserved both names.

## Eleven-axis adversarial pass — E06-S13

| Axis | Evidence and limit |
| --- | --- |
| Path and identity changes | Final Unix lookup uses a held no-follow parent and checks device/inode; Windows opens the named child relative to a held parent and checks volume/file index on the disposition handle. The E21 renamed-root hard-linked-decoy regression passes. |
| Partial reads and permissions | Open, metadata, identity, and final link-count observation errors return refusal; no missing fact is read as one link. This pass did not construct new permission-failure fixtures. |
| Links, mounts, and reparse points | Initial and late hard links refuse; both names survive in the local reproductions. Unix no-follow traversal and Windows reparse check remain. Directory and other kinds have no delete operation. |
| Provider version and layout drift | The safety executor's confirmed file path remains downstream of root/layout revalidation; this change does not add a provider-specific delete path. The stable CLI fixture used only a synthetic default Claude root. |
| Concurrency | A second link inserted at the platform callback after the first check is rejected by the final seam. There is still a non-atomic interval between Unix `fstatat` and `unlinkat`, and between Windows handle facts and `SetFileInformationByHandle`: a link introduced in that interval can escape the precheck. Unix's retained-handle postcondition would report failure after unlink. The previously owner-accepted Unix leaf-name race covers the same final two-syscall interval; Windows has the analogous link-count observation interval despite handle-bound name resolution. This is a residual, not a claim of atomic multi-link refusal. |
| Crash, failure, and retry | Stable-channel kill-points CLI suite passed locally; exact repair-commit Windows kill harness passed. A refused two-link file was unchanged, so a later run could delete it after the extra link was removed. |
| Boundary values | The implementation accepts exactly one link and rejects counts other than one at the final seam; the independent fixture exercised one and two. Windows count is carried as `u32`, Unix count as platform `st_nlink` widened to `u64`. |
| Policy and trust conflicts | The independent deletion used an explicitly stable build, `clean --yes`, and an eligible default root. The operation selector admits only confirmed plain files; no policy/authority widening was found in the repair diff. |
| Platform differences | macOS executed the independent CLI and focused Unix tests. Windows-target Clippy passed locally; native Windows quality, stable CLI, and kill harness passed remotely at `7f6c554`. Native Windows execution by this verifier is **NOT RUN**: no Windows host. |
| Malformed and untrusted input | The final Windows child name is validated as one component; reparse status and identity are read from the opened object. The final Unix `fstatat` is no-follow. No malformed-name bypass was found by source trace. |
| Performance and large data | The repair adds one link-count comparison to existing final metadata reads. The 2,000-session cutover benchmark passed; the Windows listing/CLI jobs passed. No separate high-concurrency link-race stress run was performed. |

## Gates and provenance

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Windows-target Clippy, `x86_64-pc-windows-gnu` | PASS; target installed |
| `cargo test --workspace` | PASS, including doctests |
| Focused Unix late-link and E21 hard-linked-decoy tests | PASS; one matching test ran in each focused command |
| `CANCELLAI_CHANNEL=stable CARGO_TARGET_DIR=target/stable-channel cargo test -p cancellai-cli --features kill-points` | PASS; includes three kill harness tests |
| Independent stable-release two-link/single-link CLI fixture | PASS; exit 3 with both names and bytes retained, then exit 0 and the only remaining name deleted |
| `python3 -m pytest tests -q` after stable release build | PASS: 703 tests and 669 subtests |
| `python3 scripts/cutover_benchmark.py check` after stable release build | PASS: 2,000-session corpus, three runs; Rust 0.305–0.338 s and 9.6–27.8 MiB across the four measured commands |
| `python3 scripts/check_mutation_boundary.py check` | PASS; single production deletion boundary |
| Exact repair-commit `rust.yml`, run `35898569202`, SHA `7f6c554…` | PASS: Windows quality, stable CLI, and kill harness jobs; quality runs workspace tests including the Windows late-link test |
| Latest-main `rust.yml` at review time, SHA `184acaa5…` | UNKNOWN: queued when checked; the newer commit does not change the reviewed repair in this tree |
| Local native Windows late-link reproduction | NOT RUN: no Windows host; source trace and exact repair-commit native CI are separate evidence |
| `python3 scripts/project_os.py check`, `python3 scripts/verifier_handoff.py check`, `python3 scripts/check_process.py check` | PASS before and after evidence generation; process check reports the owner-recorded E06 round-count exception |
| `python3 scripts/process_metrics.py check`, `python3 scripts/check_evidence.py check` after generation | PASS; evidence check reports only recorded pre-convention warnings |

## Residual and owner decision

The final link-count check and OS mutation are separate calls on both platforms. A new hard link created strictly between them can leave one name removed before the operation notices on Unix; Windows disposition can likewise act after its handle facts become stale. This pass confirms refusal for initial links and deterministic links added after the first platform check, which were the required repairs. It does not claim an atomic guarantee for a concurrent final-window link insertion. The Unix portion lies in the owner-accepted E21-S07 final leaf-name race interval; the Windows link-count interval is explicitly presented to the owner here for the CR4 disposition.

### Owner decision

PENDING

PASS_WITH_RESIDUALS
