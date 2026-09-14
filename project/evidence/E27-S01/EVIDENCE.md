# Evidence Packet - E27-S01

- Commit/PR: `d9e4117` plus verifier repair `1c66095` on PR #19
- Executor: Claude
- Independent verifier: Codex
- Change Risk: **CR4** (`cancellai-sealedfs` is the unsafe/mutation-boundary crate)
- Spec version: `project/epics/E27.json`
- Authorising decision: [ADR-0028](../../../docs/adrs/0028-lint-policy-states-the-safety-thesis-and-differs-by-ring.md)

## Outcome

IMPLEMENTED and independently verified. The verifier found two inaccurate `FILE_STANDARD_INFO`
SAFETY arguments: that generated Win32 struct has two Rust `bool` fields, so it does have a
validity invariant even though its all-zero representation is valid (`false`, `false`). Commit
`1c66095` corrects both arguments without changing executable behavior.

## Acceptance Criteria Evidence

| AC | Independent evidence | Result |
| --- | --- | --- |
| AC1 | Cross-target clippy enforces `undocumented_unsafe_blocks`; the independent Python test confirms 41 `unsafe {` occurrences are all in `cancellai-sealedfs` and each source file has at least as many `SAFETY:` comments. All eleven newly documented blocks were source-inspected individually in the verifier review. | PASS |
| AC2 | Native, Windows-GNU, and Linux-GNU `cargo clippy -- -D warnings` pass. The CLI malformed-action path records a failed result and exits with `MutationFailure`; the Windows rename guard returns before `NtSetInformationFile` and therefore cannot act on a partial buffer. | PASS |
| AC3 | `lint_policy_errors()` refused synthetic crates with no lint table, a weaker level, no Clippy table, a differently named tool table, and a newly added crate; a `[lints] workspace = true` crate passed. | PASS |
| AC4 | `cancellai-tui/src/lib.rs` declares its `indexing_slicing` relaxation in source. `data.rs` projects `PolicyOutcome` and `Reversibility` from the engine but neither classifies artifacts nor invokes a mutation capability. | PASS |
| AC5 | `rust/clippy.toml` limits relaxations to `#[cfg(test)]`; integration tests explicitly opt in. Production code carries no test relaxation. | PASS |

## Safety Evidence

| Invariant | Independent result |
| --- | --- |
| SI-019 | The sealedfs diff was not comments-only: `exe_file_name` now refuses an impossible UTF-16 range and `rename_child` has a new unreachable-at-present error return. The latter returns before the rename syscall; the former is observation-only. The only executed verifier repair changes two SAFETY comments. |
| SI-021 | The old segment matcher was source-proven boundary-safe: all offsets came from `starts_with`, `ends_with`, or `str::find`, hence were UTF-8 boundaries. The new oracle is independent character-recursive `*` matching and exhaustively includes empty, `*`, trailing `*`, `**`, longer-pattern, and multi-byte cases through length four. |

## Verification Commands

```text
python3 -m pytest tests -v                                            -> 521 passed
pre-commit run --all-files                                            -> passed
cargo fmt --check                                                     -> passed
cargo clippy --workspace --all-targets --all-features -- -D warnings  -> passed (native macOS)
cargo clippy ... --target x86_64-pc-windows-gnu -- -D warnings        -> passed
cargo clippy ... --target x86_64-unknown-linux-gnu -- -D warnings     -> passed
cargo test --workspace                                                -> passed (two scheduled benchmarks ignored)
cargo deny check                                                      -> passed; pre-existing non-fatal warnings only
python3 scripts/check_coverage.py check                              -> passed
```

## Residual risks

- No nightly toolchain is installed, so Miri was not run; no toolchain was installed by the verifier.
- Windows code was cross-compiled and CI is green, but no native Windows execution occurred locally.
- `expect_used` remains intentionally outside the policy. The verifier counted 15 production
  expects: each names an internal construction/serialization/constant invariant; it is not a
  blanket proof against future lazy expects.
- The TUI-wide indexing relaxation remains safe only while the TUI continues to render/projection
  work and contains no execution path; future authority logic there needs its own review.

## Independent review and Safety Verdict

- Review: [E27-S01-VERIFIER-REVIEW.md](../E27-S01-VERIFIER-REVIEW.md)
- Verdict: [SAFETY_VERDICT.md](SAFETY_VERDICT.md) — `PASS_WITH_RESIDUALS`, pending owner acceptance.
