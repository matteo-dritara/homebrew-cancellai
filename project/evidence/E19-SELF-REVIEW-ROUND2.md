# E19 review record - second self-review of E19-S02 (private-directory redesign)

Review-Scope: epic
Round: 4

- Epic: E19 - Desktop Experience
- Stories judged: E19-S02 only (E19-S01 has been `done` since independent round 1)
- Review target: `da24f93` (the private-directory redesign) and `8b946b5` (the Windows script moved
  to .NET APIs, and a test-helper change in `cancellai-desktop-api`). Examined at `8b946b5`. During
  the review, local `HEAD` moved to `cd6a71e`, which changes only `Formula/cancellai.rb`.
- Reviewer: Claude (the executor's own model), run as a forked `epic-verifier` context. This is a
  retry: the first attempt was cut off by a rate limit and left no record.
- Date: 2026-09-23

## Reviewer independence and limits - read this before trusting the verdict

**This is a SELF-REVIEW, not an independent review.** The same model that executed the work
produced it. Context isolation kept the executor's reasoning out of the input. It does not remove
self-preference bias (`docs/development/AGENT_PROTOCOL.md`, "Self-review"). Independent rounds 1
and 2 (Codex) and self-review 1 (`E19-SELF-REVIEW.md`) all failed E19-S02. This record changes no
story status: E19-S02 stays `in_progress`.

Limits of what was run:

- **This host is macOS.** Nothing Windows-specific ran here. Windows results below come from
  GitHub Actions logs (named run IDs) or from reading the code and Windows semantics. Each
  statement says which.
- **No second local account and no `sudo`.** Every "another user" attack is reasoned from the
  base directory's permissions. What *was* run is the mechanism: as the same user, a watcher
  swaps the directory at the moment another user could, and the binary's response is observed.
  Whether a different user may perform that swap depends only on the permissions of the base
  directory, and those are stated for each case.
- Linux was exercised only by CI (`quality (ubuntu-latest)`) and by clippy for the Linux target.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E19-S02 | FAIL | Self-review 1's findings are closed: SR-F1 is closed on the real binary under an ACL'd `TMPDIR`, and removing `chmod -N` fails the new test; SR-F2's file-level race is gone; SR-F3's Windows test ran and passed in run 35850280481; SR-F4's record exists. The redesign, however, trusts its private directory **by path**. Every step re-resolves the name, follows links, and never checks that the directory is the one it created. Reproduced on this Mac (same-user swap, 5/5 runs): a directory shaped like `~/Documents` (`drwx------`, `everyone deny delete`) lost its ACL entry, was accepted as "private", and received `url.txt`, and the dashboard reported success (SR2-F1). Separately, the Windows CI leg is intermittent on this exact tree: 2 of 3 Windows runs failed (SR2-F2). AC1 and AC2 still hold. |

## Findings

### SR2-F1 - The private directory is trusted by path; a swapped-in link redirects the permission change and the files

**Classification:** implementation bug. A complete repair needs handle-relative directory
operations; if that means depending on `cancellai-sealedfs`, it is an ADR-0019 architecture
decision.

`PrivateDir::create` (`launch.rs:68-82`) makes the directory and then runs four separate operations
on its **path**: `set_permissions` (`chmod(2)`, follows links), `/bin/chmod -N` (follows links
without `-h`), `/bin/ls -led` (does not follow a link given on the command line), and `metadata`
(follows links). On Windows, `DirectoryInfo.SetAccessControl` and `GetAccessControl` resolve the
path the same way, and follow junctions. `write_private_with` then creates the files by path again.
Nothing checks that the path is still the directory created a moment earlier. There is no owner
check, no identity check (device/inode or file ID), no refusal of links or reparse points, and no
handle held open. So the checks confirm the *state* of whatever the path names, not *which*
directory it is.

**Reproduction (macOS 25.6, real binary, same-user swap).** `sr2/swap.c` in the scratchpad polls
the base directory and, as soon as `cancellai-desktop-*` appears, renames it and puts a symlink to
a "victim" directory in its place:

```sh
mkdir base victim && chmod 700 victim && chmod +a "everyone deny delete" victim   # like ~/Documents
chmod +a "everyone allow list,search,add_file,add_subdirectory,delete,delete_child,file_inherit,directory_inherit" base
./swap base victim &
TMPDIR=base cancellai-desktop --no-open --requests 1
```

| Victim | Runs | Result |
| --- | --- | --- |
| `drwx------` + `everyone deny delete` (the real `~/Documents` and `~/Desktop` on this machine have exactly this) | 5/5 | ACL entry **removed**; directory accepted as private; `url.txt` written **into the victim**; stdout `URL written to base/cancellai-desktop-…/url.txt` (the path resolves through the link) |
| `drwxr-xr-x` + `everyone allow read` | 12/12 | ACL entry **removed**, *then* refused with "could not make the dashboard's directory private". The refusal comes after the mutation |

The swap lands in the window created by spawning `/bin/chmod`. The same window is observable
without any swap: `sr2/aclwatch.c` read the new directory's ACL while it was visible, and in 2 of 3
runs it still carried the inherited
`everyone:…:allow,inherited,file_inherit,directory_inherit:read,write,execute,delete,append,delete_child`.

**Who can do this to another user.** Anyone who may rename an entry in the base directory and
create one there. That is either `delete` on the new directory (which it inherits from the base
during the window above) or `delete_child` on the base, plus `add_file` on the base. The defaults
do **not** grant this: the per-user macOS `TMPDIR` (`drwx------`), sticky `/tmp`, `XDG_RUNTIME_DIR`,
and the per-user Windows `%TEMP%`. It is granted by:

- **Windows, `TMP`/`TEMP` = `C:\Temp` (reasoned).** A folder created in `C:\` inherits
  `Authenticated Users:(OI)(CI)(M)` from the drive root, so every account has Modify, including
  `DELETE`, on the new directory until PowerShell replaces its DACL. That window is a PowerShell
  start-up, hundreds of milliseconds. Another account opens it with `DELETE` and renames it away.
  It then leaves a **junction** in its place (no privilege needed), pointing at a directory the
  victim owns. `SetAccessControl` follows the junction and replaces that directory's DACL with a
  protected current-user-only one, dropping `SYSTEM` and `Administrators` and inheritance. The
  read-back passes, and the two files are written there. Alternatively, it leaves a directory it
  **owns** that grants Everyone full control. The script's `SetAccessControl` succeeds and the
  read-back (`Count -eq 1`, own SID) passes, because it checks rules and not the owner. The owner
  keeps an implicit `WRITE_DAC`, so it can add an inheritable read entry after the read-back and
  before `url.txt` is created. That window is the PowerShell process exit, and it is how the token
  would leak.
- **macOS or other Unix, a `TMPDIR`/`XDG_RUNTIME_DIR` another user can rename entries in**: a
  non-sticky world-writable directory, or a base ACL granting `delete_child` or inheritable
  `delete`, as in the reproduction. On macOS a substitute the attacker owns that grants
  `writesecurity` passes `chmod 0700`, `chmod -N` and the mode read-back (`man chmod`:
  writesecurity is "ownership, mode, ACL"). Its owner can then re-add an inheritable read entry
  before the file is created, which is a race. On Linux the leak does not follow: the file is
  created `0600`, and a default ACL on an attacker's directory is masked by that mode. The
  link-following `chmod` still does.

**Why it is a FAIL and not a disclosed residual:**

1. ADR-0038 discloses only "a parent directory that grants another user `WRITE_DAC`". The
   condition above is weaker (`DELETE`/Modify) and is the default for a `C:\Temp`.
2. ADR-0038 claims SI-019 "hold[s] structurally - the shell has no mutating request to send". The
   reproduction shows the shell itself changing the ACL/DACL and mode of a directory it did not
   create, chosen by whoever performs the swap, outside the safety executor. `check_mutation_boundary.py`
   counts deletion only, so it cannot see this.
3. This is the class the repository already treats as a defect: E07-S07/SI-002 found a path
   re-resolved through a link and repaired it with handle-relative operations.

**Required repair**, in the executor's choice of means:

- Establish the directory's identity and keep it. Refuse if the path is a link or reparse point
  (`symlink_metadata`; `chmod -h` on macOS; `$d.Attributes -band ReparsePoint` on Windows). Verify
  the owner is the current user; on Windows, add an owner-SID check to the read-back. Create the
  files only in that same directory: on Unix, compare device/inode before each create, or use a
  held descriptor. On Windows, open a handle right after `CreateDirectory` with
  `FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT` and without `FILE_SHARE_DELETE`
  (safe `OpenOptionsExt`), and hold it until the files exist. A handle someone else opened with
  `DELETE` during the window then makes that open fail, so the dashboard refuses, and nobody can
  rename the directory afterwards. Confirm on Windows that `SetAccessControl` still succeeds while
  that handle is held.
- Tests: a same-user swap test (a symlink on Unix, a junction on Windows) that asserts a refusal
  and that the victim's mode, ACL and contents are unchanged. This review's harness is a template.
- ADR-0038: correct the residual to what remains after the repair, and correct the SI-019
  sentence.

### SR2-F2 - The Windows leg is intermittently red on this tree

**Classification:** test defect in `cancellai-desktop-api/tests/unauthorized_clients.rs` (E19-S01's
suite, changed by `8b946b5` under `Story: E19-S02`). Verification gap for the ceiling decision's
condition (3).

The same Rust tree (`8b946b5`; `cd6a71e` touches only the formula) ran three times on Windows:

| Run | Commit / trigger | Windows result |
| --- | --- | --- |
| 35850280481 `rust` | `8b946b5` push | `quality (windows-latest)` passed, including `launch::tests::windows_private_directories_and_files_grant_only_the_current_user_under_a_broad_parent` |
| 35850978612 `release` | tag `v1.19.0` → `8b946b5` | `verify-rust (windows-latest)` **failed**: `an_unsupported_version_is_refused_with_the_supported_list` (`left: 0, right: 1`) |
| 35851039777 `rust` | `cd6a71e` push | `quality (windows-latest)` **failed**: that test, and `a_token_from_another_server_is_refused` (`index out of bounds: the len is 0`) |

`exchange` sends a second line before reading. The server refuses the first line and closes with
that second line unread, so Windows resets the connection. The reset discards the refusal line the
client has not read yet, and `Err(_) => break` then returns no lines. `8b946b5` fixed the Linux
*write-side* broken pipe (the earlier failure in run 35848802304); the Windows *read-side* loss
remains. The desktop client itself sends `hello` and reads the reply before sending more, so this
is a harness defect, not a product one.

**Required repair:** make the helper read the refusal before sending anything else, or make the
server drain and shut down its write side before closing (E19-S01 behaviour). Then show the
Windows leg green on repeated runs, not once.

Observation for the owner, outside this story's verdict: `v1.19.0` (E18's release) was tagged on
`8b946b5`, and its `release` workflow failed on this flake.

### SR2-F3 - Stale statements (minor, documentation)

- ADR-0038 (the private-directory bullet) and the `windows_acl` module doc in
  `launch.rs:148-153` still say "PowerShell `Set-Acl`/`Get-Acl`". `8b946b5` replaced both with
  `DirectoryInfo` APIs.
- `project/evidence/E19-S02/EVIDENCE.md`, "Compatibility", says "failure to open prints the URL".
  The round-1 repair removed exactly that. The binary now prints "could not open a browser; rerun
  with --no-open". The header line "Independent verifier: Codex (pending, E19 round 1)" is also
  stale.

## Self-review 1 findings - closure check

| Finding | Status | Evidence |
| --- | --- | --- |
| SR-F1 macOS inherited ACL entries on `0600` files | **Closed** for the reported case | Real binary under a `TMPDIR` with `everyone allow …,file_inherit,directory_inherit`: private directory `drwx------`, `launcher.html` / `url.txt` `-rw-------`, **no ACL entries** (`ls -le`), both default and `--no-open` runs. Mutation: turning `strip_and_confirm` into `return Ok(())` makes `macos_inherited_access_entries_do_not_reach_the_private_directory_or_its_files` fail; restored with no diff. The class is not closed; see SR2-F1 |
| SR-F2 Windows create-then-restrict race on the file | **Closed** for the file | Files are created only after the directory's DACL is replaced and read back, so they inherit only the current-user rule (asserted by the Windows test, run 35850280481). A handle opened on the *directory* during the window gives list access only, not the files. The window itself remains and now carries SR2-F1 (`DELETE`) and the disclosed `WRITE_DAC` residual |
| SR-F3 Windows test never executed | **Closed** | Run 35850280481, job `quality (windows-latest)`: all 17 `cancellai-desktop` lib tests pass, including the Windows ACL test. Note that `tests/startup_output.rs` is `cfg(unix)` and runs 0 tests on Windows, so the real binary is never run end to end there. The Windows test also has no control step proving its `icacls` broadening took effect, unlike the macOS test |
| SR-F4 missing ceiling record | **Closed** | `project/evidence/E19-S02/CEILING_DECISION.md` exists and states the closure conditions |

## What was falsified and held

| Axis | Attack | Observed |
| --- | --- | --- |
| stdout / stderr | Default launch (fake `open` on `PATH`) and `--no-open`, both under an ACL'd `TMPDIR`; an 8-request HTTP session | Token (64 hex characters) found 0 times in stdout/stderr in every run. Stdout names the port and, with `--no-open`, the `url.txt` path |
| argv | `ps -axww -o pid,args` captured while running; fake opener logged its argv | Token 0 times in the process list; opener received only `…/cancellai-desktop-<16 hex>/launcher.html` |
| Nonce vs token | `nonce()` source | First 16 hex characters of a *separately generated* `SessionToken`, not the dashboard token |
| Launcher lifetime | Read the launcher before and after one `200` | Contained the token before and not after |
| Base manipulation | `XDG_RUNTIME_DIR` relative, missing, a file, an ACL'd directory; `TMPDIR` missing, read-only, a symlink to a directory, containing a newline; `umask 0777` | Invalid `XDG_RUNTIME_DIR` falls back to `TMPDIR`; an ACL'd one is stripped. Missing or read-only `TMPDIR` refuses (exit 3). A newline in the path makes the `ls -le` parse **fail closed** ("could not remove inherited access entries"). `umask 0777` yields a `----------` URL file: private but unreadable by its owner (availability only) |
| `ls -led` parsing | Can it report "no entries" while entries exist? | It only confirms after `chmod -N` has succeeded. A split first line (newline in path) produces a spurious entry and refuses. No false-pass case found, apart from SR2-F1's link, where `ls -d` lists the link and not its target |
| Windows script (read) | `$ErrorActionPreference='Stop'`; protection flag; read-back | Any .NET exception exits non-zero → refused. Read-back includes inherited rules, so failed protection gives `Count > 1` → refused. A NULL DACL reads back as an Everyone rule → refused. Path passed via environment, not command text. **Not checked: owner, reparse point** (SR2-F1) |
| F2 scrub guard (regression) | `launch::tests` fault injection | Both fault tests pass on macOS and on Windows run 35850280481 |
| HTTP surface (live socket, real `cancellai-cli`, scratch `HOME`) | GET; POST; foreign `Host`; upper-cased token; token plus a character; `action=clean`; `localhost:` Host; wrong-port `Host` | 200, 405, 421, 404, 404, 400, 200, 421. `server.rs`, `render.rs` and the API crate's `src/` are unchanged since self-review 1 |
| No mutation path (request level) | `cargo tree -p cancellai-desktop -e normal`; `check_mutation_boundary.py` | Only `cancellai-desktop-api` and `serde_json`; mutation boundary OK. The shell's *own* permission changes are SR2-F1 |
| Headless core | `cargo tree -i cancellai-desktop -e all` | Only `cancellai-cli` depends on it, as a dev-dependency |
| CLI parity | `cancellai-cli/tests/desktop_api.rs::the_dashboard_view_matches_the_cli_human_summaries` | ok in the workspace run |
| `unauthorized_clients.rs` change | Does `break` on a later write error weaken any assertion? | No: each test still asserts the response count and `engine.calls == 0`, so fewer responses fail rather than pass. It is flaky on Windows instead (SR2-F2) |

## Residual observations (not the reason for the FAIL)

- Exit by signal skips `Drop`, leaving the launcher's URL in the user's own private directory
  (disclosed in ADR-0038). The private directories are never removed.
- On Linux, snap- or flatpak-packaged browsers may not be able to read a launcher under
  `/run/user/<uid>`. That is availability only; `--no-open` is the workaround. Not tested.
- Browser history keeps the tokenised URL (disclosed).

## Gates run

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy … --target x86_64-pc-windows-gnu -- -D warnings` | PASS |
| `cargo clippy … --target x86_64-unknown-linux-gnu` | Workspace: could not build here (`libsqlite3-sys` C cross-compile). `-p cancellai-desktop -p cancellai-desktop-api`: PASS |
| `cargo test --workspace` (macOS) | PASS: 1173 passed, 0 failed, 2 ignored |
| `cargo deny check` | PASS |
| Mutation: disable `chmod -N` | macOS ACL test FAILS as it should; restored, no diff |
| Swap harness (SR2-F1) | 5/5 and 12/12, as above |
| `python3 scripts/` `project_os.py check`, `check_process.py check`, `process_metrics.py check`, `release.py check`, `check_evidence.py check`, `check_docs.py check`, `verifier_handoff.py check`, `check_rust_workspace.py check`, `check_risk_classification.py check`, `check_agent_skills.py check`, `check_repository_topology.py check`, `check_platforms.py check`, `check_workflows.py check`, `check_mutation_boundary.py check`, `gen_docs.py --check` | all exit 0 before this record was added |
| Windows | Not run here. CI: runs 35850280481 (pass), 35850978612 and 35851039777 (fail, SR2-F2) |
| pytest, ruff, mypy | Not run: no Python changed in `68ae556..8b946b5` |

The `adversarial-cases` and `risk-gate` skills were not invoked as tools. Their axes (output
channels, argv, filesystem races and link substitution, hostile environment, fault injection, HTTP
confusion, mutation reachability, platform divergence) were applied directly, as tabulated above.

## Documents opened

`AGENTS.md`; `CLAUDE.md`; `project/evidence/E19-SELF-REVIEW.md`;
`project/evidence/E19-S02/CEILING_DECISION.md`; `project/evidence/E19-S02/EVIDENCE.md`;
`project/epics/E19.json` (via `project_os.py brief E19-S02 --role verifier`);
`docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md` (the diff in
`da24f93` and the mutation/residual lines); `docs/security/SAFETY_INVARIANTS.md` (header, SI-002,
SI-019); `docs/security/THREAT_MODEL.md` (scope, assets, actors); `docs/PRODUCT.md` (desktop
passage); `CHANGELOG.md` and `docs/architecture/TARGET.md` (diff in `da24f93`);
`scripts/process_metrics.py` and `scripts/check_process.py` (record parsing);
`rust/crates/cancellai-desktop/src/launch.rs` (whole); `rust/crates/cancellai-desktop/src/main.rs`
(whole); `rust/crates/cancellai-desktop/tests/startup_output.rs` (whole);
`rust/crates/cancellai-desktop-api/tests/unauthorized_clients.rs` (helper and assertions); the
logs of CI runs 35848802304, 35850280481, 35850978612, 35851039777. Not opened: `docs/INDEX.md`,
`docs/CONSTITUTION.md`, `docs/development/AGENT_PROTOCOL.md` (relied on self-review 1's reading of
"Self-review"), `server.rs`/`render.rs` (unchanged since self-review 1, which read them).

## Round verdict: FAIL (self-review)

The redesign closes what self-review 1 reported, and the Windows ACL test has now run green once
on CI. It does not close the class. The private directory is made private by path and used by path,
so whoever can swap that path picks which directory the dashboard re-permissions and writes into.
That is reproduced mechanically on macOS, and on Windows it is reachable with a `C:\Temp`
temporary directory. The Windows leg the ceiling decision depends on is also not reliably green
(1 of 3 runs). Per `CEILING_DECISION.md`, E19-S02 does not close on this round. Two paths are open:
repair SR2-F1 and SR2-F2 and self-review again, or have the owner accept SR2-F1 as a named residual,
with ADR-0038's residual and SI-019 sentences corrected. That choice is the owner's. Status is
unchanged at `in_progress`.
