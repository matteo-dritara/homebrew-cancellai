# Independent Verifier Review - E27-S01

- Review target: `d9e4117d57fb32472f90348c262f83d984d8c9fe`, repaired by `1c66095`
- Verifier: Codex (independent verifier)
- Date: 2026-09-14
- Risk: CR4; SI-019 mutation boundary / the sole unsafe crate
- Verdict: `PASS_WITH_RESIDUALS` (owner acceptance remains required)

## Initial diff claim

The required `git show d9e4117 -- rust/crates/cancellai-sealedfs/ | grep '^[+-]' | grep -v '^[+-][+-]'`
check disproved a literal comments-only reading. In addition to comments, it changes
`windows_process::exe_file_name` to a checked range and changes `windows_sealed::rename_child`
to return an error instead of aborting on an impossible range. Both were evaluated below.

## Acceptance criteria

| AC | Independent evidence / reproduction | Result |
| --- | --- | --- |
| Every unsafe block has a written argument | `pytest` ran `test_every_unsafe_block_in_the_workspace_carries_a_safety_comment`; manual review enumerated every one of the eleven new arguments. Two `FILE_STANDARD_INFO` claims were false as written and repaired in `1c66095`. | PASS after repair |
| No production panic through unwrap/indexing/slicing on tier-1 platforms | Native, Windows-GNU, and Linux-GNU clippy all pass with warnings denied. The malformed CLI action and Windows buffer changes were source-traced, not assumed from lint output. | PASS |
| Local lint tables cannot drop workspace lints | Synthetic temporary workspaces: no table, weaker `allow`, rust-only table, renamed Clippy tool table, and a newly added crate all produced `lint_policy_errors`; an inheriting table passed. | PASS |
| Exemption is in source | `cancellai-tui/src/lib.rs` has the only crate-wide indexing exemption; `data.rs` only projects engine facts and has no provider/platform/mutation dependency. | PASS |
| Test relaxation does not reach production | `rust/clippy.toml` applies to unit tests; every integration-test exception is an explicit crate attribute; production source has none. | PASS |

## SAFETY argument audit

| Location | Claim tested | Result and method |
| --- | --- | --- |
| `src/lib.rs:is_symlink_at` | `libc::stat` zero is valid; `fstatat` overwrites before read. | CONFIRMED: target Unix definitions are scalar integer fields/fixed arrays; error path reads only `rc`, never `stat`. `MaybeUninit` would be idiomatic but is a preference here. |
| `src/lib.rs:establish_with_hook` | `mkdirat` sees a live directory FD and validated leaf. | CONFIRMED: `current` owns the FD and is borrowed only for the call; `decompose_absolute_path` constructs leaf C strings from one normal path component, excluding separators, dot and dot-dot. No concurrent close is possible through this borrow. |
| `src/lib.rs:unlink_child_matching_unix_identity` | Second `libc::stat` zero is valid and overwritten before use. | CONFIRMED: success is required before `st_dev`/`st_ino` are read; errors return immediately. `MaybeUninit` is a non-required improvement. |
| `src/macos_filesystem.rs` | `libc::statfs` zero is valid and `f_fstypename` is read only on success. | CONFIRMED from the macOS-only aggregate shape (integer fields, `fsid_t`, fixed `c_char` arrays) and error return. `MaybeUninit` is preference, not a defect. |
| `src/windows_allocation.rs` | `FILE_STANDARD_INFO` zero has no validity invariant. | REPAIRED: `windows-sys 0.61.2` defines two `bool` fields. All-zero is valid (`false`, `false`), but the original no-invariant claim was false. `1c66095` corrects it; the API result is checked before any field read. |
| `src/windows_identity.rs` | `BY_HANDLE_FILE_INFORMATION` zero is valid and call overwrites it. | CONFIRMED: only `u32` and `FILETIME` (`u32` pairs); failure returns before field access. `MaybeUninit` is preference. |
| `src/windows_process.rs` | `PROCESSENTRY32W` zero is valid and `dwSize` ordering is right. | CONFIRMED: fields are integers/`[u16; 260]`; zero is valid. `dwSize = size_of::<PROCESSENTRY32W>() as u32` executes immediately before `Process32FirstW`, exactly as Win32 requires; failure returns before `entry` is read. |
| `src/windows_sealed.rs:nt_open_child` | `IO_STATUS_BLOCK` zero is valid and overwritten before use. | CONFIRMED: its union is `NTSTATUS` or nullable raw pointer plus `usize`; zero selects a valid representation. NT call result is checked before use. |
| `src/windows_sealed.rs:is_delete_pending` | `FILE_STANDARD_INFO` has no validity invariant. | REPAIRED as above in `1c66095`; success is required before `DeletePending` read. |
| `src/windows_sealed.rs:bind_existing` | Second `IO_STATUS_BLOCK` argument has a valid zero representation. | CONFIRMED with the same generated union definition; error returns before field access. |
| `src/windows_sealed.rs:rename_child` | Third `IO_STATUS_BLOCK` argument has a valid zero representation. | CONFIRMED with the same generated union definition; `NtSetInformationFile` is the immediately following call. |

## Behavior and oracle attacks

- **Windows rename guard:** `buffer` is allocated as `header_len + new_wide.len() * 2` and is not resized before `get_mut(header_len..)`, so the new error is unreachable under present arithmetic. If reached after future change it returns before `NtSetInformationFile`; the target remains open but unrenamed and is closed on return. Refusal is safer than an abort in a mutation path, and the caller receives a normal error/result rather than an acted-on partial rename buffer. Native Windows execution remains unavailable locally.
- **Malformed CLI action:** the new branch appends a `failed` `ActionResultDoc` with `INTERNAL_FAULT`, zero reclaimed bytes, and `unchanged`, sets `any_failed`, then continues. It skips all lookup/revalidation/mutation work and no counter is silently incremented. JSON therefore includes the failed action; text reports successes only and the exit code is `MutationFailure`, so it is not optimistic.
- **Glob:** `reference_matches` is character-recursive and independently expresses `*` as every possible character-prefix consumption. `segment_matches` is called per slash-separated segment, so the oracle's no-slash corpus is the correct scope. The exhaustive corpus includes empty pattern/input, `*`, trailing `*`, `**`, and patterns longer than inputs. The old byte-slicing version was source-proven safe: prefix/suffix starts and `find` results are UTF-8 boundaries; no panic counterexample was found.
- **Policy judgments:** 15 production `expect`s were counted. They are named construction, serialization, owned-pipe, enum-closed-set, or compile-time-constant invariants; no lazy user/environment failure was found. `expect_used` remains a residual policy choice, not a mechanically enforced guarantee. No denied lint is redundant: `unsafe_op_in_unsafe_fn` exposes operations to the documentation lint, while the remaining Clippy lints cover distinct panic, debug, leak, or indexing mechanisms.
- **TUI:** `EngineData::plan_context` maps engine-owned `PolicyOutcome` and `Reversibility` into rendering/confirmation state; it does not derive authority, classify an artifact, or execute a plan. No direct filesystem/provider/mutation dependency exists in the crate.

## Counterexamples attempted with no finding

- Invalid UTF-8 boundaries (`é`, `𝄞`) and all small glob combinations.
- Empty, star-only, double-star, trailing-star, and longer-than-input glob patterns.
- A false claim that the old matcher panics; none exists under valid `&str` inputs.
- Manifest linter bypasses listed in the AC table.
- Error paths after every zeroed out-parameter and the `PROCESSENTRY32W::dwSize` contract.
- Descriptor escape/close between leaf validation and `mkdirat`.
- Half-prepared Windows rename buffer reaching the rename syscall.

## Gates run

| Command | Target/result |
| --- | --- |
| `python3 -m pytest tests -v` | macOS host: 521 passed |
| `pre-commit run --all-files` | passed |
| `cargo fmt --check` | workspace: passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | native macOS: passed |
| same Clippy command with `--target x86_64-pc-windows-gnu` | Windows-GNU cross-target: passed |
| same Clippy command with `--target x86_64-unknown-linux-gnu` | Linux-GNU cross-target: passed |
| `cargo test --workspace` | macOS host: passed; two scheduled benchmarks ignored |
| `cargo deny check` | passed; existing non-fatal unmatched-license/duplicate warnings |
| `python3 scripts/check_coverage.py check` | passed |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`; `project/epics/E27.json`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`; `docs/adrs/0028-lint-policy-states-the-safety-thesis-and-differs-by-ring.md`.

## Unverified / residual

- Miri was not run: `rustup toolchain list` has no nightly, and the verifier did not install one.
- No local native Windows execution was possible. PR #19's Windows CI is green; local evidence is cross-target compilation plus source inspection.
- `MaybeUninit` would eliminate the need to establish the valid zero representation for all ten out-parameter sites, but every current zero is valid and read only after a successful call; it is not a safety finding in this scoped lint-policy story.
