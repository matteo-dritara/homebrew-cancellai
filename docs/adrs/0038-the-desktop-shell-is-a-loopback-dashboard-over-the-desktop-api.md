# ADR-0038: The desktop shell is a read-only loopback dashboard over the desktop API

- Status: Accepted
- Date: 2026-09-23
- Owners: project owner (decision delegated to the executor for this work session, 2026-09-23;
  open to owner reversal)
- Related: E19-S01, E19-S02, ADR-0006, ADR-0019, ADR-0015

## Context

E19-S02 asks for a "tray/menu-bar/dashboard experience using shared core, with Tauri or an
equivalent evidence-backed choice", under two acceptance criteria: the desktop is optional and
the core remains headless-capable, and the UI shows the same inventory/policy semantics as the
CLI and TUI. ADR-0006 already fixes that every client uses the engine's contracts and holds no
mutation logic; E19-S01 built the only channel a desktop client may use - a versioned,
token-authenticated, read-only loopback API hosted by `cancellai-cli desktop-api`.

The choice of UI technology is therefore not about capability to reach the engine - every option
reaches it through that API - but about what the option costs this workspace. The workspace gates
every dependency with `cargo deny` against an explicit licence allow-list (MIT, Apache-2.0,
BSD-2/3-Clause, ISC, Unicode-3.0, Zlib; widening it is its own reviewed change, ADR-0015), bans
wildcards, and runs clippy and tests on macOS, Linux and Windows for every crate in `crates/*`.

Measured on 2026-09-23 against the current `rust/deny.toml`, each candidate resolved in an
isolated scratch project:

| Candidate | Crates in lock file | Licence rejections | Duplicate versions | Tray / menu bar |
| --- | --- | --- | --- | --- |
| Tauri 2 (`tray-icon` feature) | 430 | 6 (MPL-2.0: `cssparser`, `cssparser-macros`, `dtoa-short`, `option-ext`, `selectors`; Apache-2.0 WITH LLVM-exception: `target-lexicon`) | 28 | yes |
| eframe 0.33 (egui) | 403 | 3 (BSL-1.0 x2; a font bundle under OFL-1.1 / Ubuntu-font-1.0) | 29 | no |
| std-only loopback dashboard | 0 new | 0 | 0 | no |

For scale, the whole existing workspace resolves 235 crates. Tauri and eframe would each roughly
triple the supply-chain surface of a tool whose value is a trustworthy safety boundary; both
would need the licence allow-list widened; and on Linux, Tauri additionally needs WebKitGTK and
AppIndicator development packages on every CI runner that builds the workspace.

## Decision

We will ship the desktop experience as a **read-only dashboard served on loopback and opened in
the system browser**, in a new outer-ring crate `cancellai-desktop` whose only cancellai
dependency is `cancellai-desktop-api`.

- `cancellai-desktop` starts `cancellai-cli desktop-api`, reads its descriptor, and serves one
  HTML page on `127.0.0.1` behind a fresh 256-bit path token.
- The token is the dashboard's only credential, so the tokenised URL is never printed and never
  passed as a process argument (where other local users could read it in a process list). By
  default it goes into an owner-only launcher page in the temporary directory - an HTML redirect
  handed to the browser opener by path and overwritten without the URL once the dashboard has
  loaded (emptied, not deleted: deletion belongs to the one mutation seam, SI-019); with
  `--no-open`, `--url-file <path>` writes it to a new owner-only file instead. Standard output
  names only the port (E19 round 1).
- "Owner-only" is enforced, not assumed from the directory (E19 round 2): mode `0600` on Unix;
  on Windows the file's DACL is replaced by a single protected rule for the current user's SID
  (through PowerShell `Set-Acl`, since `unsafe` FFI is not available to this crate) and read back
  before any secret is written, refusing if it names anyone else. A write or sync that fails
  part way leaves the file empty, never holding part of the URL.
- The page contains no script and one `GET` form that changes only the query. It shows the
  engine's per-provider `status` summary, root origin and eligibility, and the plan preview
  counted exactly as the CLI's `plan` summary counts it, plus the CLI's incomplete-scan and
  root-authority warnings. It offers no control that cleans, configures or executes.
- The HTTP surface refuses anything but `GET` on the tokenised path from a `Host` of
  `127.0.0.1:<port>` or `localhost:<port>` (DNS rebinding), and sends a CSP forbidding script and
  framing, `Referrer-Policy: no-referrer` and `Cache-Control: no-store`.
- The crate is not packaged by `release.yml`, which builds `cancellai-cli` only. The core stays
  headless: nothing in the CLI depends on the desktop crate.

A native tray or menu-bar icon is **not** part of this decision. It needs a platform UI toolkit,
and every toolkit measured carries the licence and dependency costs above; this workspace's
`forbid(unsafe_code)` also rules out calling Cocoa or Win32 directly. It is recorded as a
disclosed gap, to be revisited only by a decision that accepts one of those costs explicitly.

## Alternatives considered

### Tauri 2

The option the story names, and the only one measured that provides a tray. Rejected on the
measurements above: six crates outside the licence allow-list, 430 crates, 28 duplicates, and
Linux system packages on every CI runner. None of that is a verdict on Tauri's quality; it is
the cost of adopting it here, which the story's own "evidence-backed" wording asks to weigh.

### eframe / egui

A native window without a web view. Rejected for the same reason at a slightly lower cost
(three licence rejections, 403 crates), and it provides no tray either, so it buys no capability
the dashboard lacks.

### A tray via direct platform calls

Would need `unsafe` FFI into Cocoa, Win32 and a Linux status-notifier protocol. ADR-0017 permits
`unsafe` in exactly one crate for exactly one reason, and a tray icon is not that reason.

## Consequences

### Positive

- No new third-party code: the dashboard adds zero crates to the lock file and needs no licence
  exception, CI package, or platform toolkit.
- The desktop can only read. It reaches the engine solely through the E19-S01 API, whose
  vocabulary has no mutating request, so the UI cannot become a second path to a decision.
- Parity with the CLI is testable end to end (`cancellai-cli/tests/desktop_api.rs`), because
  the view model copies engine-computed values rather than recomputing them.

### Negative / cost

- No tray or menu-bar presence; the user starts `cancellai-desktop` and a browser tab opens.
- A browser tab is a weaker UI surface than a native window: the URL (with its token) sits in
  the browser's history on the local machine.

### Neutral / follow-up

- If a tray becomes a requirement, the decision to take on a toolkit's licence and dependency
  cost is an owner decision recorded as a new ADR that supersedes the tray paragraph here.

## Safety and compatibility impact

- Change Risk implication: CR1 for the shell (observational); the boundary it consumes is E19-S01
  (CR3).
- Safety Invariants affected: none weakened. SI-007 (no ambiguity resolves toward mutation) and
  SI-019 (one mutation boundary) hold structurally - the shell has no mutating request to send.
- Migration/rollback: the crate is optional and unpackaged; removing it removes nothing the CLI
  or TUI uses.

## Supersession

If replaced later, keep this ADR and mark it superseded by ADR-XXXX.
