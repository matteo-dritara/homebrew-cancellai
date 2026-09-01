# E07-S07 Independent Verifier Review - Round 1

- Review target: `11e7b24..0bb7f6c72afd3dce6b98b83b8dea878954c8e9bb`
- Verifier: Codex (`/root`)
- Date: 2026-09-01
- Scope: E07-S07 only (standalone CR4 carry-forward review)

## Verdict: FAIL

The static-root link regressions are closed, but the configuration authority path still permits
a provider-root symlink swap after the last link check and before the first write operation.
This violates AC2 and SI-002, SI-003, SI-013, and SI-019.

### Reproduction / counterexample

Start `configure --claude-retention 30` with `$HOME/.claude` as a real directory, so
classification records `RootOrigin::Default`.  After `cmd_configure` calls
`roots::is_symlink(&claude_resolved.path)` at `rust/crates/cancellai-cli/src/main.rs:859` and it
returns `false`, atomically rename the directory aside and atomically install
`$HOME/.claude -> <outside>`.  Continue the process.  Line 866 calls
`configure_claude_retention` with the raw lexical path; its `create_dir_all`, `read_to_string`,
temporary-file open, and final `rename` at lines 918-962 follow the replacement symlink.  The
write therefore reaches `<outside>/settings.json`.

The check and use are separate syscalls and the code holds neither a verified directory handle
nor an identity-bound/no-follow configuration capability.  This is an execution-time root-drift
counterexample, not the already-tested case where the root was a link before classification.

### Exact required repair

Implement a root-bound configuration-write capability that validates and retains the provider
root through the write. On Unix, use a no-follow opened directory handle with identity
revalidation and handle-relative settings/temp/rename operations. On Windows, provide
equivalent reparse-safe handle semantics or fail `configure` closed until they are verified.
Add a deterministic root-swap test that pauses after final validation, changes the real root to
a symlink, and proves an outside settings sentinel is unchanged. Also execute a real Windows
junction fixture before claiming AC3 is closed.

The two `#[cfg(windows)]` tests only cover `symlink_dir`, were not executable on this verifier's
macOS environment, and do not create a true NTFS junction. They are residual gaps; the Unix
configuration escape is the reason for FAIL.

## Gate status

| Command actually run | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS before review status change |
| `python3 scripts/project_os.py status` | PASS; E07-S07 was `ready_for_review` |
| `python3 scripts/project_os.py next` | PASS |
| `python3 scripts/project_os.py review` | PASS; E07-S07 was the only queued item |
| `python3 scripts/project_os.py brief E07-S07 --role verifier` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS; native Unix root-link tests execute |
| `cargo deny check` | PASS; three existing unmatched license-allowance warnings |

The completed CR4 Safety Verdict is [project/evidence/E07-S07/SAFETY_VERDICT.md](E07-S07/SAFETY_VERDICT.md).
