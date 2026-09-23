# E19 review record - self-review after independent round 2

Review-Scope: epic
Round: 3

- Epic: E19 - Desktop Experience
- Stories judged: E19-S02 only (E19-S01 is `done` since independent round 1)
- Review target: repair commits `5863bbc` (round-1 finding) and `9757dab` (round-2 findings),
  examined at `HEAD` = `b6dd345`; nothing under `rust/` changed between `9757dab` and `HEAD`
- Reviewer: Claude (the executor's own model), run as a forked `epic-verifier` context
- Date: 2026-09-23

## Reviewer independence - read this before trusting the verdict

**This is a SELF-REVIEW, not an independent review.** `AGENTS.md` names Codex as the independent
reviewer; Codex ran rounds 1 and 2 (`E19-VERIFIER-REVIEW.md`, `E19-VERIFIER-REVIEW-ROUND2.md`),
both of which failed E19-S02, and that was the owner's two-review limit for this epic. This record
was produced by the same model that executed the work. Context isolation removed the executor's
reasoning from the input; it does not remove self-preference bias
(`docs/development/AGENT_PROTOCOL.md`, "Self-review"). This record **cannot** stand in for an
independent round, and it changes no story status: E19-S02 stays `in_progress`.

Second limitation: **this host is macOS.** The Windows-only test added for round-2 finding F1
(`windows_private_files_grant_only_the_current_user_even_under_a_broad_directory`) was not run here.
It has **not run anywhere else either**: `5863bbc` and `9757dab` are not on `origin/main`
(`origin/main` = `263eaaa`, 17 local commits ahead), and the latest CI runs on `main` belong to the
v1.18.0 release commit. The evidence packet's "Windows CI runs it" is a plan, not a result. Every
Windows statement below comes from reading the code and Windows semantics, not from execution.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E19-S02 | FAIL | Round-2 F2 is repaired, and a mutation test confirms it: disarming the guard fails both fault tests. Round-2 F1's Windows repair meets the letter of the required repair (set the ACL, read it back, fail closed) but has not been executed. The class F1 named (a token file readable by another local user through permissions inherited from its directory) is **still open on macOS**: reproduced below against the real binary, where a URL file with mode `-rw-------` carries `group:everyone inherited allow read`. A reasoned Windows race (SR-F2) also leaves the class open there against an attacker who is actively racing. AC1 and AC2 still hold; the failure is against ADR-0038's owner-only contract, which rounds 1 and 2 held this story to. |

## Findings

### SR-F1 - macOS: the 0600 files inherit directory ACEs, so owner-only is still assumed from the directory (reproduced)

**Classification:** implementation bug. The class from round-2 F1 was closed only on the platform
the finding named.

ADR-0038 says: *"'Owner-only' is enforced, not assumed from the directory (E19 round 2): mode
`0600` on Unix"*. On macOS, a file created in a directory that carries an inheritable ACE
(`file_inherit`) receives that ACE, whatever the mode argument to `open`. macOS evaluates ACL
entries before the POSIX mode bits, so an inherited `everyone allow read` grants read to every
local user even though `ls -l` shows `-rw-------`. `create_private` only sets `mode(0o600)` on
Unix and never inspects the ACL.

Reproduction (real `cancellai-desktop` binary, a stand-in engine that prints a descriptor, macOS
25.6):

```sh
mkdir shared && chmod +a "everyone allow read,file_inherit" shared
cancellai-desktop --cli ./fake-cli --no-open --url-file shared/url.txt --requests 1
ls -le shared/url.txt
# -rw-------@ 1 matteo.peo  wheel  88 ... shared/url.txt
#  0: group:everyone inherited allow read
```

The URL file held the 88-byte tokenised URL. Setting `TMPDIR` to a directory with the same
inheritable ACE and running without `--no-open` gives the **launcher** page the same entry
(`0: group:everyone inherited allow read`, mode `-rw-------`). The default per-user macOS `TMPDIR`
here (`/var/folders/.../T/`, `drwx------`, no ACL) does not trigger it, so the default launcher is
not exposed. `--url-file` is: its directory is the user's choice, which is the same scenario round-2
F1 was about.

Test false confidence: `the_url_file_is_private_and_never_overwritten` and
`the_launcher_redirects_is_private_and_forgets_the_url` check only `mode & 0o777 == 0o600`, which
this file passes.

Linux was not exercised. From the POSIX ACL semantics in `acl(5)`, a default ACL on the directory
is masked by the `0600` create mode (the mask and the group class become `---`), so Linux looks
closed. That conclusion is reasoned, not tested.

**Required repair:** on macOS, make sure no ACL entry grants anyone but the owner before any
secret byte is written, or refuse. Add a test that creates both files under a directory carrying
`everyone allow read,file_inherit` and asserts either that no ACE remains or that creation is
refused. Removing the ACE after creation (`chmod -N` plus a read-back) has the same pre-open race
as SR-F2, because the ACE exists from the moment the file does. A race-free design (for example,
creating the file inside a fresh `0700` directory whose own ACL was cleared while it was still
empty) may need an ADR-0038 amendment. Correct the ADR sentence either way.

### SR-F2 - Windows: a handle opened before `Set-Acl` survives it (reasoned, not executed)

**Classification:** implementation bug. Closing it completely may be an architecture decision
(see the repair).

`create_private` creates the file with Rust's default Windows share mode
(`FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`) and only then starts `powershell.exe`
to replace the DACL. Until PowerShell finishes, the new file carries the ACL it inherited from its
directory. That window is a process start, typically hundreds of milliseconds. Windows checks
access when a handle is opened, not on each read. So another local user whose inherited ACL grants
read can open the file during the window, for example after seeing it appear through
`ReadDirectoryChangesW`, which the same broad ACL permits. That user keeps a valid read handle after
the DACL is replaced and reads the token once it is written. The read-back check inspects the DACL
and cannot see handles that are already open. This is exactly the scenario round-2 F1 described:
"a destination under a deliberately broader-ACL directory".

**Required repair:** open the file with `share_mode(0)` (`std::os::windows::fs::OpenOptionsExt`,
a safe API). No other principal can then open a data handle while this process holds the file,
and `Set-Acl`, which needs only `WRITE_DAC`/`READ_CONTROL`, should still succeed; confirm that on
Windows. What remains is an inherited `WRITE_DAC` grant (`FullControl`/`ChangePermissions` on the
directory), which lets a racer rewrite the DACL afterwards. Only a security descriptor supplied
at creation closes that, and that needs FFI, which ADR-0019/ADR-0017 place outside this crate: an
owner/architecture decision. Add a Windows test that holds a second read handle opened before the
ACL change and asserts creation refuses or the handle is rejected.

### SR-F3 - The Windows F1 test has never executed (verification gap)

Covered under "Reviewer independence" above. Until a Windows CI run on the pushed commits shows
`windows_private_files_grant_only_the_current_user_even_under_a_broad_directory` passing, round-2
F1's Windows repair is unverified. Examples of what could fail there: `Set-Acl` behaviour under an
elevated runner whose default owner is `BUILTIN\Administrators`, and `Get-Acl` translation of the
SID.

### SR-F4 - The evidence packet cites a document that does not exist

`project/evidence/E19-S02/EVIDENCE.md` ends: "see `CEILING_DECISION.md` for how the story was
closed". No `project/evidence/E19-S02/CEILING_DECISION.md` exists; the only file with that name is
`project/evidence/E16-S08/CEILING_DECISION.md`. `check_docs.py` does not catch it because the name
is a code span, not a link. This is an evidence defect: the owner's ceiling decision for E19 has to
be recorded before that sentence is true.

## What was falsified and held

| Axis | Attack | Observed |
| --- | --- | --- |
| stdout / stderr | Default launch through a fake `open` on `PATH`; `--no-open --url-file` launch; raw-probe run | Token (64 hex characters) found 0 times in stdout and stderr in every run; stdout names only the port and the URL-file path |
| argv | `ps -axww -o args` while the dashboard ran; the fake opener logged its own argv | The only process line containing the token was the `grep` doing the search. The opener received only the launcher path (`.../cancellai-desktop-<16 hex>.html`) |
| Launcher lifetime | Read the launcher before and after one `200` | Contained the token before the load and did not after it. Mode `-rw-------` in the private TMPDIR |
| F2 scrub guard | Mutated `armed: true` to `false` and ran `launch` tests | `a_failed_url_file_write_or_sync_leaves_no_url_on_disk` and `a_failed_launcher_write_or_sync_leaves_no_token_on_disk` both FAIL; restored, both pass. The 180-byte budget really does write `secret` before failing, so the launcher test is not vacuous |
| HTTP surface (live socket, real `cancellai-cli` with a scratch `HOME`) | GET ok; POST; HEAD; DNS-rebinding `Host`; no `Host`; duplicate `Host` with the foreign one first; `localhost.:port`; upper-cased token; token plus `/`; absolute-form target; `days=-1`; `action=clean` | 200; 405; 405; 421; 421; 421; 421; 404; 404; 404; 400; 400 |
| Referer / outbound | Headers and body of a 200 | CSP `default-src 'none'`…, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`, `Cache-Control: no-store`. Body: one `<form method="get" action="/<token>">` and no `a`, `link`, `img`, `script` or `iframe` elements. The launcher carries `<meta name="referrer" content="no-referrer">` |
| Token comparison | `SessionToken::matches` | Length check, then constant-time XOR fold |
| No mutation path | `cargo tree -p cancellai-desktop -e normal`; the `Request` enum; `check_mutation_boundary.py` | Only `cancellai-desktop-api` among `cancellai-*`. `Request` = `Hello`/`Document`/`Goodbye`. Mutation boundary OK. No `remove_*`/`rename` in non-test desktop code |
| Headless core | `cargo tree -i cancellai-desktop -e all` | Only `cancellai-cli` depends on it, as a dev-dependency |
| CLI parity | `cancellai-cli/tests/desktop_api.rs::the_dashboard_view_matches_the_cli_human_summaries` | ok in the full workspace run |

## Residual observations (not the reason for the FAIL)

- `scrub`/`Launcher::expire` reopen the file **by path** and follow symlinks. If another principal
  can replace directory entries (not possible in a `0700` TMPDIR, sticky `/tmp`, or a per-user
  `%TEMP%`), expiry could overwrite a planted target, and a failed scrub is silent. Truncating
  through the handle already held would remove the dependence on the path.
- A `SIGINT`/`SIGTERM` exit skips `Drop`, so a launcher that never loaded keeps its URL. Observed
  after `kill`. The token authorizes only the process that has just died, so no live credential is
  exposed.
- The server answers one connection at a time, and loopback is shared by every local user, so any
  local account can stall the dashboard by drip-feeding a request head. This is availability only,
  of the same kind as E19-S01's disclosed residual.
- Linux browsers packaged with a private `/tmp` (snap, flatpak) may be unable to open a launcher in
  `/tmp`. Not tested.
- Browser history keeps the tokenised URL. ADR-0038 discloses this.

## Gates run (this host, macOS)

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy ... --target x86_64-pc-windows-gnu -- -D warnings` | PASS (lints the Windows code; does not run it) |
| `cargo test --workspace` | PASS: 1171 passed, 0 failed, 2 ignored; includes `tests/startup_output.rs`, `tests/dashboard.rs`, `cancellai-cli/tests/desktop_api.rs` |
| `cargo deny check` | PASS (advisories, bans, licenses, sources ok) |
| Mutation: disarm `ScrubOnFailure` | Both F2 tests FAIL, as they should; source restored with no diff |
| `python3 scripts/gen_docs.py --check`, `project_os.py check`, `check_docs.py check`, `check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_process.py check`, `release.py check`, `check_agent_skills.py check`, `process_metrics.py check`, `check_risk_classification.py check`, `check_evidence.py check`, `verifier_handoff.py check`, `check_repository_topology.py check`, `check_platforms.py check`, `check_workflows.py check` | all exit 0 (before this record was added) |
| Windows test for F1 | NOT RUN: macOS host, and the commits have not reached CI |
| pytest, ruff, mypy | NOT RUN: no Python changed in `e964b98..HEAD` |

The `adversarial-cases` and `risk-gate` skills were not invoked as tools. Their axes (output
channels, argv, file permissions under hostile parents, failure injection, HTTP method/host/path
confusion, mutation reachability) were applied directly, as shown in the table above.

## Documents opened

`AGENTS.md`; `CLAUDE.md`; `project/evidence/E19-VERIFIER-REVIEW.md`;
`project/evidence/E19-VERIFIER-REVIEW-ROUND2.md`; `project/evidence/E19-S02/EVIDENCE.md`;
`project/epics/E19.json` (E19-S02 entry); `docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md`;
`docs/development/AGENT_PROTOCOL.md` ("Self-review"); `docs/PRODUCT.md` and
`docs/architecture/TARGET.md` and `CHANGELOG.md` (desktop passages only);
`project/evidence/E24-SELF-REVIEW.md` (format precedent); `scripts/process_metrics.py` and
`scripts/check_process.py` (review-record parsing); `rust/crates/cancellai-desktop/Cargo.toml`;
`rust/crates/cancellai-desktop/src/{launch.rs,main.rs,server.rs,source.rs,lib.rs}` and the
relevant lines of `render.rs`; `rust/crates/cancellai-desktop/tests/startup_output.rs`;
`rust/crates/cancellai-desktop-api/src/auth.rs` (`matches`) and `src/protocol.rs` (`Request`,
`Descriptor`). Not opened this session: `docs/INDEX.md`, `docs/CONSTITUTION.md`,
`docs/security/SAFETY_INVARIANTS.md`, `docs/security/THREAT_MODEL.md`.

## Round verdict: FAIL (self-review)

Round-2 F2 is repaired and has a test that would catch its regression. Round-2 F1's Windows repair
is unexecuted, and its class is still open: reproduced on macOS (SR-F1), reasoned on Windows
against an active racer (SR-F2). E19-S02's status is unchanged at `in_progress`. With the
independent-review limit reached, whether SR-F1/SR-F2 are repaired before closure or become backlog
work items carried as residuals is the owner's decision, and SR-F4's missing ceiling record has to
exist either way.
