# E07-S07 Independent Verifier Review - Round 2

- Review target: `7e3d938..f9db57e`
- Verifier: Codex (`/root`)
- Date: 2026-09-02
- Scope: E07-S07 only (standalone CR4 carry-forward review)

## Verdict: FAIL

The repair closes the round-1 final-provider-root swap: `SealedRoot` retains an
`O_NOFOLLOW`-opened final directory descriptor and uses `openat`/`renameat` for child work.
It does not meet the story's broader requirement to reject a root whose *path resolution*
crosses a link boundary. `O_NOFOLLOW` applies only to the final component; the initial
`symlink_metadata`, `create_dir_all`, and `OpenOptions::open(path)` all follow intermediate
components.

## Native reproduction

On macOS, I built the review target and ran the resulting binary with a synthetic filesystem:

```text
/private/tmp/.../home-link -> /private/tmp/.../outside
/private/tmp/.../outside/.claude/settings.json = {"cleanupPeriodDays":7}
HOME=/private/tmp/.../home-link cancellai-cli configure --claude-retention 30
```

With `CLAUDE_CONFIG_DIR` and `CODEX_HOME` unset, the command exited `0` and printed
`Set Claude Code cleanupPeriodDays to 30.` The file outside the lexical HOME path became:

```json
{
  "cleanupPeriodDays": 30
}
```

This is not a final-leaf link: `home-link/.claude` is a real directory, so
`roots::is_symlink` returns false and `SealedRoot::establish` accepts it. The symlink is an
intermediate component, followed before the retained descriptor exists. It violates AC2 and
SI-002, SI-003, SI-013, and SI-019. It also means AC1's default-root authority guarantee is
not true for all path-resolution forms. AC3 remains incomplete: native Unix leaf fixtures pass,
but there is no intermediate-link fixture; Windows has no true NTFS-junction fixture and was not
run locally.

## Counterexample review

- Final-root swap after `establish`: closed by the held Unix descriptor; no lexical root lookup
  occurs in the child `openat`/`renameat` operations.
- Temp-file/rename: `O_CREAT|O_EXCL` prevents an existing temporary symlink from being opened,
  and `renameat` replaces a final symlink entry rather than following it. No separate escape was
  found there.
- Intermediate components: FAIL as reproduced above.
- Windows: `SealedRoot` fails closed on non-Unix, but the pre-existing Windows tests are
  directory-symlink-only, not true NTFS junction fixtures. This remains disclosed residual
  evidence, not proof of the AC3 junction claim.
- Concurrent changes after the final descriptor is established cannot redirect the sealed child
  operations, but concurrent replacement before establishment can select an intermediate-link
  target for the retained descriptor.
- `cancellai-cli`: `configure_claude_retention` is the only changed configuration writer and
  uses SealedRoot for child operations. The unsafe raw root-resolution path remains inside
  `SealedRoot::establish`, not a second CLI bypass.

## CR4 gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS before review record/status update |
| `python3 scripts/project_os.py review` | PASS; E07-S07 queued |
| `python3 scripts/project_os.py brief E07-S07 --role verifier` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS; existing unmatched BSD-2-Clause, BSD-3-Clause, ISC allow-list warnings |

## Required follow-up (no third review round)

Per the bounded-review rule, this finding is recorded as new CR4 backlog item **E07-S09** rather
than requesting E07-S07 round 3. E07-S09 must bind every Unix path component handle-relatively
with no-follow semantics (and only create below an already-held parent), prove both configure
and clean do not reach an outside sentinel through an intermediate link, and retain Windows
fail-closed behavior until real junction/reparse semantics are verified.
