# E07-S09 Independent Verifier Review - Round 1

- Review target: `a4cb802..c519f86`
- Verifier: Codex (`/root`)
- Date: 2026-09-02
- Scope: E07-S09 only (standalone CR4 carry-forward review); E07-S07 is not re-reviewed.

## Verdict: FAIL

`SealedRoot::establish` now correctly refuses the former intermediate-link path for
`configure`, but the same provider-root path remains accepted by `clean`. A synthetic `$HOME`
symlink whose target contains a real `.claude` leaf caused `clean --yes` to purge a stale
outside session. AC1 expressly covers configuration *or cleanup* mutation, so the story cannot
pass while that independently reproducible escape remains.

## Native reproduction

On macOS, against the built review target, I created only synthetic directories:

```text
/private/tmp/.../home-link -> /private/tmp/.../outside
/private/tmp/.../outside/.claude/settings.json = {"cleanupPeriodDays":7}
/private/tmp/.../outside/.claude/projects/proj-a/<stale UUID>.jsonl
HOME=/private/tmp/.../home-link, with CLAUDE_CONFIG_DIR and CODEX_HOME unset
```

Results:

| Command | Exit | Outside result |
| --- | --- | --- |
| `cancellai-cli configure --claude-retention 30` | 4 (`SAFETY_BLOCK`) | `settings.json` remains `{"cleanupPeriodDays":7}` |
| `cancellai-cli clean --tool claude --days 7 --keep-latest 0 --allow-running --yes --json` | 0 | deletes the outside stale session and reports `succeeded: 1`, `reclaimed_bytes: 3` |

The leaf (`home-link/.claude`) is a real directory, so `roots::is_symlink` returns false.
`configure` then reaches the repaired `SealedRoot::establish` and refuses the intermediate
`home-link`; `clean` instead reaches `ApprovedRoot::establish` through
`establish_verified_root`, which still resolves/canonicalizes the complete lexical path and has
no equivalent whole-component sealed walk. Its leaf-only `roots::is_symlink` pre-check therefore
does not disagree diagnostically, but it is insufficient as an authority boundary.

## Acceptance criteria and counterexamples

| Criterion | Independent evidence | Result |
| --- | --- | --- |
| AC1: intermediate Unix link is refused before configuration or cleanup reaches it | `configure` refuses and preserves its outside sentinel; the directly analogous `clean --yes` run deletes the outside sentinel/session. | FAIL |
| AC2: all Unix components are handle-relative/no-follow; absent leaf creation is below held parent | `SealedRoot` itself opens `/`, iterates with `openat(parent_fd, name, O_NOFOLLOW|O_DIRECTORY)`, and does `mkdirat` only under held `current`; its EEXIST retry reopens no-follow. But that primitive is not used for the cleanup root. | FAIL at the CLI mutation surface |
| AC3: Windows has equivalent verified semantics or fails closed | `fallback_impl::SealedRoot::establish` remains `Unsupported` on non-Unix. No new Windows capability is claimed. | PASS (fail closed) |

Further inspection found no bypass within `SealedRoot`'s walk: component strings use raw Unix
bytes and reject embedded NUL through `CString`; relative paths and `.`/`..` refuse; a long
component chain repeats the same held-parent `openat`; and an `mkdirat` EEXIST race is re-opened
with `O_NOFOLLOW`. The canonicalized test temp roots only eliminate macOS's OS-provided
`/var -> /private/var` prefix and do not canonicalize the attacker-created link fixture. Those
facts establish that the configure repair is real, but cannot compensate for the clean bypass.

## CR4 gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS before review recording |
| `python3 scripts/project_os.py review` | PASS; E07-S09 queued |
| `python3 scripts/project_os.py brief E07-S09 --role verifier` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS (including all 11 `cancellai-sealedfs` tests) |
| `cargo deny check` | PASS; advisories, bans, licenses, and sources pass. It reports only existing unmatched BSD-2-Clause, BSD-3-Clause, and ISC allow-list warnings. |

Targeted direct `SealedRoot::establish` tests passed for the intermediate symlink, relative
path, and `..` path. Passing tests are not a proof of AC1 because the new fixture exercises
only `SealedRoot`, while cleanup uses `ApprovedRoot`.

## Required repair for round 2

Route cleanup's provider-root establishment through an equivalent whole-path, retained-handle
capability before planning or mutation, or extend the root-capability/mutation path so every
component from a trusted anchor is opened no-follow relative to its already-held parent. A
leaf-only `roots::is_symlink` check or a post-resolution canonical-path containment comparison
is not sufficient. Add deterministic CLI tests for both `configure` and `clean --yes` using an
intermediate `$HOME` symlink and assert the outside settings/session sentinels remain unchanged.
Retain the existing `mkdirat` EEXIST/symlink refusal behavior and Windows `Unsupported`
fail-closed posture.
