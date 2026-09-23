# Evidence Packet - E19-S02

- Commit/PR: the commit carrying this packet on `main`
- Executor: Claude
- Independent verifier: Codex (pending, E19 round 1)
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
  (Linux); failure to open prints the URL.

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
- The tokenised URL is kept in the local browser history.
- Not packaged: users build `cancellai-desktop` from source until a release story adds it.

## Verifier verdict

(pending - independent reviewer)
