# Evidence Packet - E19-S02

- Commit/PR: the commit carrying this packet on `main`
- Executor: Claude
- Independent verifier: Codex (rounds 1-2, both FAIL); then forked self-reviews 1-3 (see "Verifier verdict")
- Change Risk: CR1
- Spec version/commit: `project/epics/E19.json` E19-S02; technology choice recorded in
  [ADR-0038](../../../docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md)

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "Desktop is optional and core remains headless-capable." | `cancellai-desktop` is a separate crate whose only `cancellai-*` dependency is `cancellai-desktop-api`; no crate depends on it except `cancellai-cli` as a **dev**-dependency for the parity test; `release.yml` builds and packages `cancellai-cli` only; every existing CLI test passes unchanged (`cargo test --workspace`, 1152 passed) | PASS |
| AC2 "UI shows the same inventory/policy semantics as CLI/TUI." | `cancellai-cli/tests/desktop_api.rs::the_dashboard_view_matches_the_cli_human_summaries`: against the real binary, for two queries (one with both delete candidates and observations, asserted), the dashboard view's provider lines equal `status` output line for line, its plan headline equals `plan`'s first line, it shows one row per CLI action line, and every reason it shows is the CLI's; `tests/dashboard.rs::the_dashboard_shows_what_the_desktop_api_returned` (whole chain in process: API server, `ApiViewSource`, dashboard, rendered page); warnings for incomplete scans and root-authority withholding render when the engine reports them (`render::tests::warnings_render_when_the_engine_reports_them`) | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-007 / SI-019 | A dashboard control or request that reaches `clean` | the page has one `GET` form and no script (`the_page_has_no_script_and_only_a_get_form`); `POST`/`DELETE` refused (405); the crate cannot link the policy, platform or safety crates | PASS |
| C-02 (unknown is shown, not hidden) | An unreadable plan rendered as "nothing to clean" | `an_unreadable_plan_is_an_error_not_an_empty_plan`; engine failure renders a 502 page with no counts (`an_engine_failure_is_shown_escaped_not_as_an_empty_dashboard`, `an_unreachable_api_is_a_502_page`) | PASS |
| Local web attack surface | URL guessing, DNS rebinding, oversized head, markup injection from engine text | 404 on wrong/absent token, 421 on foreign or wrong-port `Host`, 431 on an oversized head, all engine text escaped (`engine_text_cannot_inject_markup`); CSP/X-Frame-Options/no-referrer/no-store headers asserted on a real socket | PASS |

## Verification Commands

```text
cargo fmt --check                                                        -> ok
cargo clippy --workspace --all-targets --all-features -- -D warnings     -> ok
cargo clippy ... --target x86_64-pc-windows-gnu -- -D warnings           -> ok
cargo test --workspace                                                   -> 1152 passed, 0 failed
cargo test -p cancellai-desktop                                          -> 18 passed
cargo test -p cancellai-cli --test desktop_api                           -> 6 passed
cargo deny check                                                         -> ok (no new crate in Cargo.lock)
python3 scripts/check_coverage.py check                                  -> OK; cancellai-desktop 84.93%
python3 scripts/check_rust_workspace.py check                            -> 15 crates match TARGET.md
manual smoke: cancellai-desktop --requests 3 on this machine             -> 200 page with real providers; wrong token 404; foreign Host 421; engine child exited with the dashboard
```

## Compatibility

- New crate `cancellai-desktop` (library + `cancellai-desktop` binary), std plus `serde_json`.
- The desktop API envelope gains `provider_summaries` (API version 1 is unreleased, so no
  deployed client is affected); `cancellai-cli` computes it with the function its own human
  `status` output now uses.
- Browser opening: `open` (macOS), `rundll32 url.dll,FileProtocolHandler` (Windows), `xdg-open`
  (Linux), given the launcher file's path; failure to open suggests `--no-open` and never prints
  the URL.

## Performance / operability

- One API session per page load; the dashboard serves one request at a time with a 10 s read
  timeout and an 8 KiB request-head cap.

## Documentation updated

- ADR-0038 (new), `docs/PRODUCT.md` ("Interfaces"), `docs/architecture/TARGET.md` (crate list;
  "Desktop dashboard (E19-S02)"), `CHANGELOG.md`.

## Method defects

- none

## Residual risks

- No tray or menu-bar icon (ADR-0038): the measured toolkits need licence exceptions and add
  400+ crates. A tray is an owner decision to take on that cost.
- TUI parity is by construction rather than by test: the TUI renders `cancellai-policy` views and
  is not yet wired to a live scan, so the parity test holds the dashboard to the CLI, whose
  output comes from the same engine.
- The tokenised URL is kept in the local browser history. The private directory and its (emptied)
  launcher are not removed; the temporary directory's own cleanup removes them. An exit by signal
  skips the emptying, leaving a dead server's URL in the user's own private directory.
- A same-user process can swap the private directory between check and use; it could read the
  token anyway (ADR-0038). A shared base is refused rather than used.
- Not packaged: users build `cancellai-desktop` from source until a release story adds it.

## Round-1 repair (independent review FAIL, `project/evidence/E19-VERIFIER-REVIEW.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| The tokenised URL was printed on stdout, so redirecting output to a log captured the dashboard's only credential | The URL is never printed or passed as a process argument. Default: an owner-only (0600 on Unix) launcher page with a random name in the temporary directory, handed to the browser opener by path and overwritten without the URL after the first successful page load or on exit (emptied rather than deleted: `check_mutation_boundary.py` reserves deletion for the SI-019 seam). `--no-open --url-file <path>`: the URL goes to a new owner-only file, refusing an existing one. Stdout names only the port. `--no-open` without `--url-file` is a usage error | `tests/startup_output.rs` runs the real binary end to end with a stand-in engine: the token reaches the URL file (mode 0600) and serves a 200 page, and appears in neither stdout nor stderr; a pre-existing URL file is refused untouched with no URL printed. Reintroducing `println!("{url}")` fails two of its three tests. `launch::tests` cover the message, file privacy and launcher expiry (explicit and on drop) on every platform |

Manual smoke with the real `cancellai-cli`: `--no-open --url-file` served a 200 page; the token
appeared 0 times in stdout and stderr; the URL file was `-rw-------`.

## Round-2 repairs (independent review FAIL, `project/evidence/E19-VERIFIER-REVIEW-ROUND2.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| F1: on Windows the launcher and URL file inherited their directory's ACL, so a broad directory exposed the token | `create_private` replaces the new (still empty) file's DACL with one protected rule granting the current user's SID full control, reads it back, and refuses unless exactly that SID remains - before any secret is written. Unix keeps `0600` | `windows_private_files_grant_only_the_current_user_even_under_a_broad_directory` (Windows only: the directory is first granted to Everyone, then both files' ACLs are read back as SIDs); clippy clean for `x86_64-pc-windows-gnu`. **Not executable on the executor's macOS machine; Windows CI runs it** |
| F2: a write or sync failure after creating the launcher (or URL file) left a token-bearing file | `write_private_with` arms a guard right after creation that empties the file on any failure or panic, and is disarmed only after write and sync succeed; both the launcher and the URL file go through it | `a_failed_launcher_write_or_sync_leaves_no_token_on_disk` (failure before any byte, mid-URL, after the URL, at sync), `a_failed_url_file_write_or_sync_leaves_no_url_on_disk`; disarming the guard fails both |

## Self-review repairs (`project/evidence/E19-SELF-REVIEW.md`)

The round-2 repair of F1 restricted each file after creating it. The self-review reproduced the
same exposure on macOS (a `0600` file keeps an inherited `everyone allow read` ACL entry) and
reasoned a race on Windows (a handle opened before `Set-Acl` survives it). The file-level approach
was replaced rather than patched:

| Finding | Repair | Evidence |
| --- | --- | --- |
| SR-F1 macOS inherited ACL entries; SR-F2 Windows create-then-restrict race | Token files are only ever created inside a `PrivateDir`: a fresh random directory made owner-only while empty (Unix `0700` read back; macOS `chmod -N` and `ls -le` confirmation; Windows protected current-user DACL set and read back). `--url-file <path>` is removed: with `--no-open` the URL goes to `url.txt` in the private directory and only its path is printed | `macos_inherited_access_entries_do_not_reach_the_private_directory_or_its_files` (the self-review's reproduction as a control that must expose a plain file, then zero entries on the private directory, URL file and launcher; removing the `chmod -N` step fails it); `unix_private_directories_and_files_are_owner_only`; `windows_private_directories_and_files_grant_only_the_current_user_under_a_broad_parent` (Windows CI); `tests/startup_output.rs` (real binary: URL file under a `0700` directory, file `0600`, token absent from stdout/stderr); manual smoke on this Mac: `drwx------` directory, `-rw-------` file, no ACL entries, token 0 times in output |
| SR-F3 Windows test never executed | The E19 closing commit is pushed and `rust.yml`'s Windows leg is read before the release tag is pushed | recorded in `CEILING_DECISION.md` |
| SR-F4 missing ceiling record | Written | `CEILING_DECISION.md` |

## Self-review 2 repairs (`project/evidence/E19-SELF-REVIEW-ROUND2.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| SR2-F1: the private directory was restricted by path; a swapped link redirected `chmod -N` / `SetAccessControl` onto another directory and the files into it | No permission is changed after creation any more. The base must already be private (Unix: owned and not group/other-writable, or sticky; macOS: no ACL entry; Windows: no reparse point, only user/SYSTEM/Administrators), the directory is born `0700` there, and both are only read-checked; an unsafe base is refused | `a_base_other_users_can_write_is_refused_unless_sticky`, `macos_a_base_with_access_entries_is_refused_and_left_unchanged` (the base's ACL listing is identical before and after), `windows_private_directories_hold_only_private_rules_and_broad_bases_are_refused`; disabling the base check or the macOS ACL check each fails a test |
| SR2-F2: the Windows leg was flaky - a reset discarded the refusal the server had sent | The API server now ends a connection gracefully: after the last response it shuts down its write side and drains (bounded: 250 ms, 64 KiB) what the client already sent, so no reset discards the response | `tests/unauthorized_clients.rs` pipelines a request behind every refusal; Windows CI run repeatedly before any tag (recorded in `CEILING_DECISION.md`) |
| SR2-F3: stale statements | ADR-0038, the Windows module comment and this packet corrected | this commit |

## Self-review 3 repairs (`project/evidence/E19-SELF-REVIEW-ROUND3.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| SR3-F1: a sticky base was accepted whoever owned it; its owner can rename entries in it | A sticky base must be owned by the user or root (`base_is_safe`) | `base_safety_follows_ownership_and_the_sticky_bit`; dropping the owner condition fails it |
| SR3-F2: the "same user as the process" comment described a different check | Comment rewritten: the owner of the directory `mkdir` just created is the process's effective user, which is what is compared | this commit |
| SR3-F3: Windows never checked the owner, who can always change permissions | The script refuses unless the owner is the user, `SYSTEM` or `Administrators`; conditional ACEs (not listed by .NET) disclosed in ADR-0038 | Windows CI |
| Residuals: drain had no total deadline; `powershell.exe` found through the search path | A total 250 ms deadline on the drain; Windows PowerShell by absolute path under `%SystemRoot%` | this commit |
| SR2-F3 remainder: this packet's verifier header | Corrected | this commit |

## Verifier verdict

Independent round 1: FAIL. Independent round 2: FAIL (the owner's limit for this story).
Self-reviews 1, 2, 3: FAIL, each with fewer and smaller findings (3 found only low-severity
issues, none reachable with default settings). All repaired above; see `CEILING_DECISION.md` for
how the story closes.
