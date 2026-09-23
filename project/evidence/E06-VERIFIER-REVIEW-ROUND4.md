Review-Scope: epic
Round: 4
Verifier: Codex

# E06 independent verifier review, round 4

- Reviewed commit: `a6d9d316209fa6ad7afe95fcb8661b4957058ea6` (2026-09-23).
- Scope: exactly E06-S08, E06-S10, E06-S13, the three stories round 3 failed. No other story is judged here.
- Method: read the committed briefs and round-3 record, traced the final diff and mutation paths, used synthetic temporary homes and a stable-channel release binary, constructed new counterexamples, and checked the current main CI. The temporary Rust test used for F-05 was removed immediately after its failing run; no product source change remains.

## Verdicts

| Story | Verdict | Evidence | Committed brief |
| --- | --- | --- | --- |
| E06-S08 | **FAIL** | Nonzero exit and blank stdout are now refused, and the real 2,000-session benchmark passes. A successful, nonblank run with no valid command result still passes the gate (F-04). | Brief-Checksum: cf517c1d13964dbcdef87178e6188718da266910088bb8de8a965bf193971b53 |
| E06-S10 | **PASS** | Independent stable-release JSON clean deleted a synthetic stale Claude session, preserved history bytes with and without `--keep-claude-history`, emitted one parseable result on stdout, and placed the unchanged-history note only on stderr without the flag. Existing human note, verbose action parity, and unsupported-flag tests passed in the stable CLI suite. | Brief-Checksum: d8e38c5fdaf9a6cf5fec0ab16438c50fbe24eecea7add2d53c03a4cac92afa8b |
| E06-S13 | **FAIL** | Initial multi-link files are refused on the local stable CLI; current Windows CI ran its CLI and kill tests. A link added after the initial check causes Unix deletion to remove the planned name before reporting failure, and the Windows final delete handle never checks its own link count (F-05). | Brief-Checksum: 867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121 |

## Findings and exact required repairs

### F-04 — E06-S08: nonempty output from an empty successful command is timed

`scripts/cutover_benchmark.py::failed_run` accepts any exit 0 with nonblank stdout. `measure` does not parse or validate the four timed results; its preflight validates only one `plan` invocation. I wrapped the real stable release binary so `plan` delegates to it but `status`, `inspect`, and `clean --dry-run` print only `x` and exit 0. `python3 scripts/cutover_benchmark.py check --rust-bin <wrapper> --sessions 100 --runs 1` exited 0, printed 0.005-second fake command timings, and said `cutover performance budget OK`. A second independent fake with the correct preflight action count and `x` for the other commands also returned four measurements with no errors or budget violations. The round-3 failed-exit and blank-output cases are repaired; the required command-appropriate work proof is not.

**Required repair:** Validate every timed invocation against its command's actual result contract and the generated corpus: status/inspect must resolve the expected nonzero artifact set; plan/dry clean must propose the expected actions. Reject malformed, unrelated, or zero-work output even when exit is 0. Pin a successful nonblank fake wrapper as a regression alongside the existing failed-exit and blank-output wrappers. Check the reference invocations the same way.

### F-05 — E06-S13: a second link added after the first check is deleted on Unix and unguarded on Windows

The new checks read `before.nlink()` in `cancellai-platform::mutation::confirmed_delete_file_inner` on Unix and `before.number_of_links` on Windows. On Unix, `just_before` and `SealedRoot::unlink_child_matching_unix_identity` compare identity but do not test link count; the latter calls `unlinkat`. The post-unlink `after.nlink() != 0` detects the problem only after the planned name has been removed. I temporarily inserted a deterministic unit test whose callback creates a second hard link after the open-time check. `cargo test -p cancellai-platform verifier_round4_link_added_after_open_must_not_remove_planned_name` failed at `refusal must preserve the planned name`: the operation returned `Err`, yet the planned name was gone. The temporary test was removed and the source diff is clean.

On Windows, `windows_sealed.rs::unlink_child_matching_windows_identity` opens a new, handle-relative child and observes `WindowsFileFacts` on that exact delete handle, but tests only reparse status and volume/file index before `SetFileInformationByHandle(FileDispositionInfo)`. It never checks `facts.number_of_links`. A second link inserted after the platform layer's first-handle check therefore reaches the delete call with the same accepted identity. This final-handle path also directly accepts an initially multi-link file if invoked by its caller without the earlier guard. Native execution of the late-link fixture was **NOT RUN** because this host is macOS; the Windows source path and current Windows CI are distinct evidence, and CI does not exercise this fixture.

**Required repair:** On Unix, re-read and refuse link count before `unlinkat`, including in the held-parent final check; preserve both names when a link is added after the first open. On Windows, refuse `number_of_links > 1` on the exact child handle passed to `SetFileInformationByHandle`. Add deterministic late-link tests at the platform seam for both OSes and a cross-platform CLI regression, asserting both names survive. Retain the original Unix zero-link postcondition and the E21 hard-linked-decoy regression; do not restore eb0638f's weakened postcondition. If the remaining check-to-unlink race cannot meet AC2, put that limit before the owner rather than treating a post-delete error as refusal.

## Eleven-axis adversarial pass

| Axis | E06-S08 | E06-S10 | E06-S13 |
| --- | --- | --- | --- |
| Path and identity changes | Synthetic corpus only; preflight plan delegated to real binary | Default-root synthetic Claude session actually removed | Initial links refused; late link deletes one Unix name (F-05); Windows final handle unguarded |
| Partial reads and permissions | Failed exits refused; unrelated nonblank output accepted (F-04) | No history rewrite, note emitted after real success | Open/observation errors propagate; late-link error occurs after mutation |
| Links, mounts, reparse | Temporary corpus paths; no outside writes observed | History bytes preserved | Unix held parent and Windows reparse checks retained; multi-link race F-05 |
| Provider version/layout drift | Fixed synthetic Claude/Codex layout; invalid timed output accepted | Unsupported clean/status flags still usage errors in CLI suite | Plain-file kind only; unknown kinds refused by operation selector |
| Concurrency | Each timed invocation must stand alone; preflight cannot certify later work | Note follows actual Claude delete results | Deterministic link insertion after first check reproduces Unix deletion; Windows final handle omits link count |
| Crash, failure, retry | Nonzero and blank output now fail; fake success still passes | JSON stdout stays parseable with stderr note | Post-unlink Unix error reports failure after removal; current kill harness passes |
| Boundary values | 100-session fake pass; real 2,000-session budget pass | Both keep-flag states; byte-identical history | Initial two-link CLI case refused; one-to-two-link transition fails |
| Policy and trust | Real plan preflight has delete actions; fake command output has none | Stable channel and explicit `--yes`; note cannot authorize mutation | Stable-only deletion, identity and kind gates traced; AC2 still violated |
| Platform differences | Linux CI gate and macOS local pass despite false-pass wrapper | Local macOS reproduction; Windows stable CLI CI green | Unix dynamic reproduction; Windows source and native CI reviewed, native late-link fixture NOT RUN |
| Malformed or untrusted input | `x` accepted as status/inspect/dry-clean result (F-04) | One JSON document on stdout; flags parsed | Invalid child/reparse kinds refused; identical hard-link identity remains insufficient |
| Performance and large data | Real 2,000-session figures below; false pass is near-zero timing | Reporting change is bounded to one note | 1,500-entry listing still covered by Windows CI; link count check missing at final handle |

## Gates and provenance

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Windows-target Clippy, `x86_64-pc-windows-gnu` | PASS; target installed |
| `cargo test --workspace` | PASS, including doctests |
| `CANCELLAI_CHANNEL=stable CARGO_TARGET_DIR=target/stable-channel cargo test -p cancellai-cli --features kill-points` | PASS, including stable CLI deletion and kill harness |
| `python3 -m pytest tests` | PASS on post-build rerun: 701 tests and 667 subtests. The initial pre-build run passed with two release-binary-dependent skips. |
| Stable release build, then `python3 scripts/cutover_benchmark.py check` | PASS on real 2,000-session corpus: Rust status 0.319 s / 9.5 MiB, inspect 0.332 s / 27.2 MiB, plan 0.334 s / 27.8 MiB, dry clean 0.373 s / 13.4 MiB; all below reference timings and 64 MiB ceiling. False-pass reproduction F-04 separately FAILS the gate's validity claim. |
| `python3 scripts/project_os.py check`, `python3 scripts/verifier_handoff.py check`, `python3 scripts/check_process.py check` before evidence edits | PASS |
| Latest-main `rust.yml` at `a6d9d316…` | PASS, including Windows `quality`, `kill-harness`, and `cli-stable-channel`; `gh run view 35896648462`. This CI does not run the new late-link counterexample. |
| Newer main commit `7b566104…` (unrelated E33-S01) | `gh run view 35897192230`: Windows `kill-harness` and `cli-stable-channel` PASS; Windows `quality` IN PROGRESS at 17:43 UTC, therefore UNKNOWN for that newer commit. E33-S01 was not judged in this review. |
| `python3 scripts/project_os.py check`, `python3 scripts/verifier_handoff.py check`, `python3 scripts/check_process.py check`, `python3 scripts/process_metrics.py check` after status/evidence generation | PASS; `check_process.py` reported the owner-recorded E06 round-count exception. |
| Native Windows late-link reproduction | NOT RUN: no Windows host in this session; final-handle source reviewed and exact-main CI inspected. |
| Deliberately failing temporary Unix late-link test | REPRODUCED F-05; test removed after run. This is a counterexample, not a failing committed gate. |

## Disposition

E06-S08 and E06-S13 return to `in_progress`; E06-S10 advances to `verification`. Two of three stories fail this round. The fourth epic review exceeds ADR-0025's three-round cost ceiling; the owner had authorized this exact three-story round in `scripts/check_process.py`. No other story status is judged or changed here. The owner-visible E06-S13 Round 4 Safety Verdict remains pending an owner decision.
