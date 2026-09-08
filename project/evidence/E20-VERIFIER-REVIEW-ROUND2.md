# E20 Independent Verifier Review - Round 2

- Epic: E20 - Windows and WSL Native Support
- Review target: `2dffe54c7752be80076292ea09096b5bb8b1ef3e..aaad5b3eca42eeee1aa816dc098e7b1fa4b43ff9`
- Verifier: Codex (`/root`)
- Date: 2026-09-08
- Scope: E20-S05 only. E20-S01 through E20-S04 were already closed after the prior review and repair; the final diff does not alter their contracts.

The epic was coherent before review: E20-S05 was `ready_for_review`; E20-S01 through E20-S04 were `done`.

The historical epic-level dependency on E06 is removed as a stale cycle: E06-S04 explicitly
depends on E20 as a platform prerequisite, while E20's work is independently usable and has
its own acceptance and release gates. Retaining both edges makes either epic impossible to
close. This follows the recorded E07 closure precedent for the same cutover-gate dependency
shape; it does not remove E20-S05's concrete dependency on E20-S01.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E20-S05 | PASS | Native Windows `rust.yml` run [33904582262](https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33904582262) passed at `995fa94583b5ffa663d196d521408b8ad5e0c4c5`, the final code commit. A second successful run, [33905293698](https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33905293698), reran the Windows quality job at review head; the intervening `aaad5b3` changes only evidence/documentation. Both Windows quality jobs passed format, clippy, full workspace tests, and cargo-deny. Independent code review and the native fixture set cover reparse/symlink and junction refusal, pre-planted temporary-name reparse, identity mismatch, and before/open/mid-flight target swaps. `GetFileInformationByHandleEx(FileStandardInfo)` supplies allocated size; Toolhelp snapshot enumeration supplies Windows process facts; `NtCreateFile` with a retained `RootDirectory` handle supplies no-follow traversal and deletion. |

## Acceptance and safety verification

- AC1: Windows no longer returns the unconditional unsupported/incomplete process result. `CreateToolhelp32Snapshot`/`Process32FirstW`/`Process32NextW` enumerate native process names; the platform observer strips `.exe` and matches case-insensitively while retaining fail-closed behavior on enumeration error.
- AC2: allocated size is read from `FILE_STANDARD_INFO::AllocationSize` through a no-follow handle, with missing paths mapped to `Absent` and other I/O failures to `Unreadable`; it does not substitute logical length.
- AC3: every Windows root component is opened relative to the retained parent with a single-component name and `FILE_OPEN_REPARSE_POINT`; identity facts reject reparse points. The final operation remains handle-relative, closing the path-relookup swap class.
- AC4: deletion confirms the expected volume serial/file index and last-write ticks, rechecks immediately before deletion, deletes by a retained-parent relative handle, and corroborates `DeletePending` through the original handle. The native tests reject target swaps, reparse points, and mismatched identities.
- SI-013, SI-017, SI-018, and SI-019 pass. The companion Safety Verdict records the CR4 evidence.

## Gate status

| Command / evidence | Result |
| --- | --- |
| `python3 scripts/project_os.py check`, `status`, `next`, `review`, and verifier brief | PASS before status transition; E20-S05 was the sole queued story. |
| `python3 -m pytest tests -v` | PASS — 192 tests. |
| Ruff check/format and mypy, using the repository `.venv` | PASS. |
| Documentation, workflow, fixture, schema, characterization, differential harness/parity, workspace, mutation-boundary, provider-compatibility, platform, process, and release checks | PASS. |
| Rust `fmt`, clippy `-D warnings`, check, test, and deny | PASS. |
| GitHub Actions native Windows evidence | PASS — runs 33904582262 and 33905293698, with all Windows check and quality matrix jobs successful. |

## Overall verdict

**PASS.** E20-S05 moves to `done`, and all E20 stories are now complete, so E20 closes. The required CR4 Safety Verdict is recorded at `project/evidence/E20-S05/SAFETY_VERDICT.md`. The mandatory epic-closure release follows this review record.
