# Evidence Packet - E09-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E09 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/architecture/TARGET.md` "Atlas TUI shell (E09-S01)"

## Outcome

PASS

## Scope

Replaced `cancellai-tui`'s E02-S01 placeholder skeleton with a real keyboard-first navigation
shell: `rust/crates/cancellai-tui/src/{lib.rs,app.rs,ui.rs,capability.rs,event.rs,main.rs}` and
`tests/navigation.rs`. Built on `ratatui` 0.29 + `crossterm` 0.28, the outer-ring dependencies
ADR-0019 names for this epic. `Cargo.toml` now depends on no `cancellai-*` crate at all - the
skeleton's `cancellai-inventory`/`cancellai-platform`/`cancellai-provider-claude`/
`cancellai-provider-codex`/`cancellai-store`/`cancellai-model`/`cancellai-safety`/
`cancellai-provider-api` dependencies are all removed, since none is used by a pure
shell/navigation story and their presence was the concrete violation of AC1.

Four screens (`Home`, `Atlas`, `Explain`, `Plan`); `Atlas`/`Explain`/`Plan` render a
"Coming in E09-S0X" placeholder pending those stories' real content. Navigation:
`Tab`/`Shift+Tab` (both the `BackTab` code and the `Shift`-modified `Tab` char, since terminals
differ on which they deliver) cycle with wraparound, `1`-`4` jump directly, `?` toggles a help
overlay driven by one `KEY_BINDINGS` table, `q`/`Esc` quits. `capability::detect` derives a
`TerminalCapability` (a three-tier `ColorSupport` from `NO_COLOR`/`TERM`/`COLORTERM`, Unicode-
vs-ASCII borders from `LANG`/`LC_ALL`/`LC_CTYPE` plus a `CANCELLAI_TUI_ASCII` escape hatch) via
an injected `EnvSource`, always degrading to the weaker capability on a missing/unrecognized
signal. `ui::draw` refuses to lay out below 24x6 and renders a plain fallback message instead.
`main.rs` installs a panic hook that restores the terminal (disables raw mode, leaves the
alternate screen) before the default panic handler runs, and does the same on both the clean
and the error exit path.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - No direct filesystem/provider access from the TUI crate | `Cargo.toml` depends on `ratatui`/`crossterm` only - no `cancellai-*` crate at all, so no provider adapter, `cancellai-inventory`, or `cancellai-platform` is even reachable from this crate's dependency graph, let alone called. | PASS |
| AC2 - All displayed actions derive from engine plans | This story displays no actions yet - `Atlas`/`Explain`/`Plan` render a fixed "coming in E09-S0X" placeholder string (`ui::tests::stub_screens_show_a_coming_soon_placeholder_not_fabricated_data`), not fabricated data standing in for a real plan. The AC becomes load-bearing once E09-S04 wires real `SealedPlan`-derived content into the `Plan` screen. | PASS (vacuous, by design - see Residual risks) |
| AC3 - Works on tier-1 terminals with graceful capability fallback | `capability.rs`'s 10 unit tests cover every branch (`NO_COLOR` override, dumb/missing `TERM`, truecolor/256color detection, UTF-8 locale detection and precedence, the ASCII escape hatch, the safest-state constant). `ui.rs`'s render tests prove the ASCII/no-color path never emits a Unicode box-drawing glyph and the too-small-terminal guard renders a message instead of panicking (`an_extremely_small_terminal_does_not_panic` at 5x2, an even smaller area than the guard's own 24x6 threshold). | PASS |

## Safety Evidence

No safety obligations are listed for this story (`safety_obligations: []`) and none apply: this
is a pure, non-mutating navigation shell with no `Action`/`SealedPlan`/mutation path and no
`AuthorityLevel` computation. CR1 (observational) is the correct classification.

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-tui: 25 unit + 3 integration passed (all new); 0 failed anywhere else
$ cargo deny check                # advisories ok (see Compatibility), bans ok, licenses ok, sources ok
$ cd ..
$ python3 scripts/check_rust_workspace.py check
$ python3 scripts/check_docs.py check
$ python3 scripts/project_os.py check
```

New tests, all in `rust/crates/cancellai-tui/`:

- `capability.rs`: 10 unit tests over synthetic `EnvSource` maps.
- `app.rs`: 8 unit tests over the key reducer (wraparound both directions, both Shift+Tab
  encodings, direct jump, help toggle, quit, an unbound key changing nothing).
- `ui.rs`: 6 render tests via `ratatui::backend::TestBackend` (Unicode+color content, ASCII+
  no-color content with an explicit glyph denylist, the too-small message, the extreme 5x2
  no-panic case, the help overlay listing every keybinding after the overlay-width fix below,
  the stub-screen placeholder).
- `tests/navigation.rs`: 3 end-to-end tests driving `App`+`ui::draw` together (a full
  Home->Atlas->Explain->Plan->Home->quit session, help-overlay open/close preserving the active
  screen, an ASCII/no-color session across every screen).

One defect found and fixed by these tests during implementation, not merely observed: the help
overlay's fixed `40`-column width clipped the longest keybinding line
(`"Tab / Shift+Tab"` + `"next / previous screen"`, 40 characters before its own 2-column
border), which `help_overlay_lists_every_keybinding_when_toggled` caught immediately. Fixed by
sizing the overlay from the longest rendered line's actual width instead of a fixed guess.

### Manual accessibility checklist

The story's verification plan calls for "snapshot/render tests plus manual accessibility
checklist" - the checklist half cannot be automated by definition. What was actually possible
from this execution environment:

- **Verified for real**: the no-real-tty failure path. Running the built binary with stdin/
  stdout not a terminal (`echo | ./target/debug/cancellai-tui`) exits 1 with a plain
  `Os { code: 6, ... "Device not configured" }` error and no panic, no hang, and nothing left
  behind - `enable_raw_mode()` itself is the call that fails, before `EnterAlternateScreen` is
  ever sent, so there is nothing to restore. This is the one terminal-interaction case
  reachable without a working PTY.
- **Not executable from this environment**: a real interactive keyboard walkthrough. This
  execution environment's PTY plumbing does not deliver data across a `pty.openpty()` pair at
  all - confirmed independently with a trivial `/bin/echo` child process producing zero
  captured bytes on the master side, not specific to this binary - so no interactive session
  (real keypresses, `NO_COLOR=1`, `CANCELLAI_TUI_ASCII=1`, an actual panic-recovery check) could
  be driven end-to-end here. This checklist item is a residual for the user (or a future session
  with real terminal access) to run directly:
  1. `cargo run -p cancellai-tui` in a real terminal; confirm every screen is reachable using
     only the keyboard (`Tab`/`Shift+Tab`/`1`-`4`/`?`/`q`).
  2. `NO_COLOR=1 cargo run -p cancellai-tui`; confirm no color escape codes appear (e.g. pipe
     through `cat -v` in a second pane, or inspect visually - everything should render in the
     terminal's default foreground/background).
  3. `CANCELLAI_TUI_ASCII=1 cargo run -p cancellai-tui`; confirm every border is drawn with
     `+`/`-`/`|` rather than Unicode box-drawing glyphs.
  4. Resize the terminal below roughly 24x6 while running; confirm the "terminal too small"
     message appears instead of a crash, and normal rendering resumes above that size.
  5. Send `SIGINT`/force-kill mid-session (not just `q`); confirm the shell prompt returns
     usable (not stuck in raw mode) - this one is not fully covered even by the panic hook,
     since a hard kill bypasses Rust's panic machinery entirely; documented as a known,
     inherent gap of any raw-mode TUI, not specific to this implementation.

## Compatibility

- No wire-format change; this is a new binary-only UI surface, not a CLI/JSON contract change.
- New dependencies: `ratatui = "0.29"`, `crossterm = "0.28"` (and their transitive deps -
  `Cargo.lock` updated). Both MIT, already on `rust/deny.toml`'s allow-list; no allow-list
  widening needed.
- `cargo deny check`'s advisories gate required one addition: `rust/deny.toml`'s
  `[advisories].ignore` now lists `RUSTSEC-2024-0436` (`paste`, an "unmaintained" - not a
  vulnerability - advisory), pulled in transitively and non-optionally by ratatui 0.29. Ratatui
  0.30 drops `paste` but raises its own `rust-version` to 1.88.0, above this workspace's MSRV
  (1.85.0, ADR-0015) - upgrading was rejected because it would silently violate the documented
  MSRV rather than go through that ADR's own review. The ignore entry documents this reasoning
  and names the two conditions that would let it be removed (ratatui drops the dependency
  upstream, or the workspace's MSRV moves to >=1.88.0 as its own reviewed decision).
- `cargo deny check` also reports (warn-level, not build-breaking per `multiple-versions =
  "warn"`) two pre-existing-shape duplicate-version situations now also touching `cancellai-tui`
  transitively (`unicode-width` 0.1/0.2 via `ratatui`/`unicode-truncate`; `windows-sys` 0.59/0.61
  via `crossterm`'s `rustix` vs. `clap`'s `anstream` chain) - normal dependency-graph noise for
  a new outer-ring addition, not an error state.

## Performance / operability

- No I/O beyond terminal read/write; no allocation beyond per-frame `Vec<ListItem>`/`Vec<Line>`
  construction already accounted for by `ratatui`'s own rendering model.

## Documentation updated

- `docs/architecture/TARGET.md`: new "Atlas TUI shell (E09-S01)" subsection.
- `CHANGELOG.md` Unreleased/Added.
- `rust/deny.toml`: documented `RUSTSEC-2024-0436` ignore (see Compatibility).

## Residual risks

- `Atlas`/`Explain`/`Plan` are placeholders; AC2 is genuinely exercised only once E09-S02/S03/S04
  land real content. Tracked by those stories, not silently assumed satisfied going forward.
- The manual accessibility checklist's interactive items could not be executed from this
  environment (PTY plumbing does not function here at all, independent of this binary - see
  above); the no-tty graceful-failure path was verified for real, but the keyboard/color/
  ASCII/resize/hard-kill walkthrough remains for the user (or a future session with a real
  terminal) to run using the exact steps listed above.
- Only macOS-hosted, non-interactive verification was possible in this environment; a real
  Linux/Windows terminal pass (e.g. Windows Terminal vs. legacy `cmd`, tmux/screen on Linux) is
  not yet exercised, matching the same per-platform honesty `docs/PLATFORMS.md` already uses for
  OS-level capability claims.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
