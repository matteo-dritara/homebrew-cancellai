# Evidence Packet - E27-S06

- Commit/PR: ce2e872 reviewed and repaired through b1f0d08 on rework/codebase-health, PR #19
- Executor: Claude
- Independent verifier: Codex
- Change Risk: CR4
- Spec version/commit: project/epics/E27.json at verifier closeout

## Outcome

IMPLEMENTED and independently verified, with the coverage-ratchet failure tracked as E27-S07 rather
than hidden by lowering its baseline.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - delete rather than document | windows-sys 0.61.2 derives Default for FILE_STANDARD_INFO and BY_HANDLE_FILE_INFORMATION; the three local mem::zeroed() blocks remain removed. The former is a 24-byte, align-8 struct with two tail-padding bytes, but both APIs take documented out-buffers and do not inspect incoming padding. The latter is thirteen u32 words, 52 bytes at align 4, with no padding. | PASS |
| AC2 - version-dependent argument says so | af97e09 rewrites every PROCESSENTRY32W and IO_STATUS_BLOCK argument to name windows-sys 0.61.2 and require rechecking after a lockfile update. Both binding definitions were inspected: PROCESSENTRY32W is scalar integers plus [u16; 260]; IO_STATUS_BLOCK is an NTSTATUS/nullable-pointer union plus usize. | PASS after repair |
| AC3 - Miri run and recorded | Exact commands below ran on the owner-provided nightly. Seven classified executable crates run 233 tests: model 19, inventory 42 (native-performance assertion filtered), provider-api 80, provider-claude 26, guardian 0, store 0, TUI 66. | PASS after repair |
| AC4 - each refusal names its cause | Local macOS Miri refusals are recorded individually: sealedfs/platform statfs; policy fsetattrlist; CLI/provider-codex posix_spawnattr_init; safety a sha2 aarch64 Stacked-Borrows report. The workflow inventory makes any added or renamed crate fail until it is classified. | PASS after repair |
| AC5 - cadence chosen from cost | TUI took 353.73 seconds total under Miri (310.02 seconds for unit tests and 43.71 seconds for integration tests), supporting weekly rather than per-push execution. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | Default leaves FILE_STANDARD_INFO padding unspecified | The documented GetFileInformationByHandleEx(FileStandardInfo) parameter is [out]; the value is not an FFI input. Passing a pointer to a valid Rust value with uninitialized padding is not UB on its own. MaybeUninit::zeroed().assume_init() would add unsafe code and provides no needed property; mem::zeroed() was not required. | PASS |
| SI-019 | same question for BY_HANDLE_FILE_INFORMATION | The documented parameter is [out]; binding layout is exactly 52 bytes/no padding. Default initializes every valid field representation. | PASS |
| SI-019 | remaining zero initialization silently rests on stale binding assumptions | Source comments name the locked binding version, list its actual fields, and direct the reader to recheck on dependency updates. PROCESSENTRY32W.dwSize is written immediately before Process32FirstW. | PASS after repair |

## Miri limits and flags

-Zmiri-disable-isolation permits the synthetic temporary filesystem fixtures; it weakens Miri's
I/O isolation and reproducibility, not its invalid-value, aliasing/provenance, or other UB checks.
-Zmiri-ignore-leaks suppresses only leak reporting for deliberate test-only Box::leak lifetime
fixtures in cancellai-tui; it does not suppress UB checks.

Miri does execute the first unsupported statfs unsafe call in cancellai-sealedfs; it cannot model
that FFI call and aborts there. It is therefore false to say it executes none of the 38 remaining
unsafe blocks, and equally false to infer that it tested blocks not reached before the abort.

## Verification Commands

    MIRIFLAGS='-Zmiri-disable-isolation -Zmiri-ignore-leaks' cargo +nightly miri test -p classified-crate
    cargo fmt --check                                                        -> PASS
    cargo clippy --workspace --all-targets --all-features -- -D warnings    -> PASS (macOS)
    cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings -> PASS
    cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings -> PASS
    cargo test --workspace                                                   -> PASS (macOS)
    cargo deny check                                                         -> PASS; pre-existing non-fatal warnings
    python3 -m pytest tests -v                                               -> PASS, 522 passed
    pre-commit run --all-files                                               -> PASS
    python3 scripts/check_coverage.py check                                 -> FAIL, E27-S07

## Residual risks

- Native Windows execution was not available locally; Windows-GNU Clippy compiles the changed code,
  and PR CI remains the native execution evidence.
- The scheduled workflow is new and has not yet run on its Ubuntu runner. Its crate inventory,
  failure behavior, and local macOS Miri results are verified; platform-specific Miri shims can
  still differ.
- E27-S07 owns the reproducible platform coverage-ratchet discrepancy (93.24% measured versus
  95.83% recorded) and must not be resolved by silently lowering the baseline.

## Verifier verdict

PASS_WITH_RESIDUALS
