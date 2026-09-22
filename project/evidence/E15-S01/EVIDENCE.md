# Evidence Packet - E15-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (Codex, per-epic review once every E15 story is `ready_for_review`)
- Change Risk: CR3
- Spec version/commit: `project/epics/E15.json`'s E15-S01 story contract

## Outcome

PASS (executor self-assessment; independent verification pending). Implements
`cancellai_guardian::service`: one Guardian engine (`GuardianService`) with a consistent
install/uninstall/enable/disable/status lifecycle, backed by three real platform adapters
(`launchd` user agent, `systemd --user` unit with explicit fallback, `schtasks.exe` user-scoped
scheduled task) selected at compile time. `cancellai-guardian`'s binary gains its first real
command surface for this lifecycle; `run` (the detection/decision/authority loop) remains the
E02-S01 skeleton, out of this story's scope (E15-S03/S04).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "Guardian can be enabled/disabled and status inspected consistently." | One `ServiceRuntime` trait (`rust/crates/cancellai-guardian/src/service.rs`) implemented identically by all three adapters; `ServiceStatus` is a closed four-value enum (`NotInstalled`/`Disabled`/`Enabled`/`Unsupported{reason}`) shared across platforms. `service::tests::guardian_service_is_the_single_public_engine_type` is a compile-time assertion that `GuardianService` implements the trait regardless of which platform module backs it. Per-adapter round-trip tests (`service_macos::tests::install_enable_status_uninstall_round_trip`, `service_linux::tests::install_enable_status_uninstall_round_trip`, `service_windows::tests::install_enable_status_uninstall_round_trip`) exercise the identical `NotInstalled -> Disabled -> Enabled -> Disabled -> NotInstalled` sequence against a fake command runner. A real `launchctl` smoke test (`service_macos::tests::real_launchd_install_enable_status_disable_uninstall_smoke_test`, `#[cfg(target_os = "macos")]`) ran for real on this executor's own macOS machine and passed (see Verification Commands). | PASS |
| AC2 "Service failure does not block manual CLI operation." | Structural: `rust/crates/cancellai-cli/Cargo.toml` does not depend on `cancellai-guardian` (confirmed by inspection - no such dependency line exists, and no dependency was added by this change), so no Guardian lifecycle failure can reach the CLI binary at all. Within the Guardian binary itself, every `ServiceRuntime` method returns `Result<_, ServiceError>` rather than panicking; `service.rs`'s own module docs record this reasoning next to `ServiceError`. Confirmed by `dispatch`/`status` in `main.rs`, which convert every error into a printed message and a non-zero exit code, never a panic. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Platform service install/uninstall smoke tests." | Real smoke test per adapter, each `#[cfg(target_os = "...")]`-gated to the one CI platform it is real on: `service_macos::tests::real_launchd_install_enable_status_disable_uninstall_smoke_test` (ran for real here, macOS); `service_linux::tests::real_systemd_user_smoke_test_or_explicit_fallback` (asserts the real, honest outcome on this workspace's own Linux CI runners - no active `systemd --user` session bus - is `ServiceStatus::Unsupported`, not a silent guess; will run under `#[cfg(target_os = "linux")]` on CI); `service_windows::tests::real_schtasks_install_enable_status_disable_uninstall_smoke_test` (will run for real under `#[cfg(target_os = "windows")]` on CI). All three are cleaned up via an RAII `Cleanup` guard so a failed assertion still removes the real OS registration. | PASS (macOS run here; Linux/Windows real runs pending CI, see Residual risks) |

## Safety Evidence

The story names no `safety_obligations` (CR3 here derives from "reversible/conditionally
mutating behavior" - registering/deregistering the Guardian binary's own OS-level service
definition - not from a named Safety Invariant). Adversarial analysis was still run per
`docs/development/AGENT_PROTOCOL.md`'s CR3 requirement (`adversarial-cases` skill, folded into
the design before code):

| Case | Expected under the contract | Test |
| --- | --- | --- |
| `enable` before `install` | `ServiceError`, never a silent implicit install | `enable_before_install_is_an_error_not_a_silent_install` (all three adapters) |
| `uninstall`/`disable` when never installed | `Ok(())` - the postcondition already holds, not an error | `uninstall_when_never_installed_is_not_an_error`, `disable_is_idempotent_when_already_disabled` (all three adapters) |
| Reinstalling over an existing definition | Replaces cleanly, no duplicate/orphaned file | `reinstalling_over_an_existing_definition_replaces_it_cleanly` (macOS, Linux; Windows `/Create /F` is inherently idempotent by the mechanism itself) |
| `status` never mutates | Exactly one read-only command issued, verified via a call-recording fake | `status_issues_exactly_one_read_only_list_call` / `..._is_active_call` / `..._query_call` |
| An unrecognized/ambiguous mechanism state (e.g. Windows `/XML` output with no `<Enabled>` element) | `Unsupported`, never a guessed `Enabled`/`NotInstalled`/`Disabled` | `unrecognized_query_output_is_unsupported_not_a_guess`; `status_reports_unsupported_when_the_session_bus_is_unreachable` |
| A missing platform binary (`launchctl`/`systemctl`/`schtasks` not on `PATH`) | `ServiceError::CommandUnavailable`, never a false `NotInstalled` | `command_unavailable_is_reported_for_a_missing_binary`; `UnsupportedRuntime`'s own `status` for a wholly unrecognized target |
| Program/argument strings containing XML/quoting-special characters (`<`, `>`, `&`, `"`, embedded quotes, whitespace) | Correctly escaped/quoted, never breaking the generated document/command line | `plist_contents_escape_xml_special_characters`, `exec_start_quotes_arguments_with_whitespace`, `exec_start_escapes_embedded_quotes_and_backslashes`, `quote_if_needed_*`, `command_line_joins_quoted_program_and_args` |
| WSL2 guest installing a Windows host service | Structurally impossible - never a runtime check | By construction: `service_windows` is compiled only when `target_os = "windows"` (or under `test`); a Linux/WSL2 binary contains no code path that can invoke `schtasks.exe` at all. Documented in `docs/architecture/GUARDIAN_MODEL.md`'s Runtime section and this module's own doc comment. |
| Second mutation-authority path | None introduced | `scripts/check_mutation_boundary.py check` (unchanged: still only `cancellai-platform::mutation` and `cancellai-safety::mutation_executor` reference the deletion capability) |
| Shell-injection via program/argument content | Not applicable - `std::process::Command::args` never invokes a shell on any platform; every adapter passes arguments as an argv vector, never a shell string | By construction, documented in each adapter's own module doc comment |

## Verification Commands

```text
$ cd rust && cargo fmt --check
(clean)

$ cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
(no warnings)

$ cd rust && cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s

$ cd rust && cargo test --workspace
... every crate: test result: ok. <N> passed; 0 failed ...
cancellai-guardian: test result: ok. 114 passed; 0 failed; 0 ignored
  (includes service::tests::*, service_macos::tests::* [10, incl. 1 real launchctl smoke test],
   service_linux::tests::* [13], service_windows::tests::* [16])

$ cd rust && cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 100 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs
deletes anything, only rust/crates/cancellai-platform/src/mutation.rs,
rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does

$ python3 scripts/project_os.py check
governance OK: 25 decisions, 33 epics, 168 stories

$ python3 scripts/check_docs.py check
docs OK: 475 Markdown files; local links and safety IDs are consistent

$ python3 scripts/gen_docs.py --check
docs/CLI.md is up to date. (+ EARS OK: 527 acceptance criteria classified, 72 describe unwanted
behaviour - no new warning for E15-S01)

$ python3 scripts/check_rust_workspace.py check
rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated

$ python3 scripts/check_platforms.py check
platforms OK: 4 platforms, generated matrix current, all CI/evidence claims verified
(unchanged by this story - see Documentation updated)
```

### Not run locally (unavailable tooling)

- `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings`
  and the same for `--target x86_64-pc-windows-gnu`: both fail during dependency compilation with
  `error occurred in cc-rs: failed to find tool "x86_64-linux-gnu-gcc"` /
  `"x86_64-w64-mingw32-gcc"` - `cancellai-store`'s bundled `rusqlite`/`libsqlite3-sys` needs a
  matching C cross-compiler this machine does not have installed, and `cancellai-guardian`
  depends on `cancellai-store` transitively. Pre-existing local environment gap, not introduced
  by this change (the same gap would block cross-target clippy for any crate in this workspace).
  CI's per-platform native runners (`.github/workflows/rust.yml`) compile natively on each real
  OS and do not hit this; they must run this before merge.
- `mypy`/`ruff`/the Python reference test suite: not run - this story touches only `rust/`,
  `docs/`, `CHANGELOG.md` and `project/`, none of which those tools cover, and `cancellai.py` is
  untouched (Python reference freeze, `AGENTS.md`).

## Compatibility

- Platforms exercised: macOS (real `launchctl` install/enable/status/disable/uninstall cycle, run
  on this executor's own machine); Linux and Windows adapters are unit-tested against a fake
  command runner here (every host runs all three adapters' non-real tests, per the
  `#[cfg(any(test, target_os = "..."))]` module-gating this story adds - see
  `rust/crates/cancellai-guardian/src/lib.rs`), with their own real smoke tests gated to run for
  real only on matching CI.
- No provider/schema surface touched.

## Performance / operability

- Each lifecycle operation issues at most a small, fixed number of external process invocations
  (no loop bounded by artifact/session count); `status` issues exactly one command, enforced by
  `status_issues_exactly_one_read_only_*_call` on all three adapters.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md`'s "Runtime" section: added the E15-S01 paragraph
  describing the one-engine/three-adapter design, the `CommandRunner` seam, AC1/AC2's grounding,
  the Linux fallback and Windows `/XML`-parsing rationale, and the disclosed Windows-quoting
  residual - following the same per-story documentation pattern E14-S01..S04 already established
  in this file.
- `docs/PLATFORMS.md`: reviewed, not edited. It is fully generated
  (`scripts/check_platforms.py generate` from `project/platforms.json`) and its tracked
  capabilities are hardcoded to exactly `identity`/`mutation` (the deletion-capability tier
  matrix) - "user-service runtime" is not one of them, and extending that generator's schema to
  track a third capability is a separate, larger decision this story does not make. The file's
  existing "Cross-platform rule" bullet already names "user-service runtime" as a required
  capability-aware abstraction, and this story's design (compile-time platform selection, no
  Unix-only assumption leaking into the domain layer, WSL2 handled structurally) is consistent
  with it without requiring a wording change. `scripts/check_platforms.py check` still passes
  unchanged.
- `CHANGELOG.md`: added an `### Added` entry under `## [Unreleased]` describing the new
  `cancellai_guardian::service` surface, matching the style of the existing E14-S01..S04 entries.

## Method defects

- none

## Residual risks

- The Linux and Windows real smoke tests have not yet run on this executor's own machine (no
  Linux/Windows environment available here) - only their fake-runner-backed logic tests ran
  locally. They will run for real for the first time on CI (`.github/workflows/rust.yml`'s
  `ubuntu-latest`/`windows-latest` quality jobs). If either fails there, it is a CI finding
  against this story, not a silently-assumed pass.
- `service_windows::quote_if_needed` (Windows command-line quoting for `/TR`) is unit-tested
  against the cases this story anticipated (embedded spaces, embedded double quotes) but is not a
  general Windows argv/command-line encoder verified against every edge case (e.g. a trailing
  backslash immediately before a closing quote, which `CommandLineToArgvW`'s own escaping rules
  treat specially). Disclosed in `docs/architecture/GUARDIAN_MODEL.md`'s Runtime section.
- `schtasks`' plain-text output is locale-dependent; this adapter avoids that by parsing `/XML`
  output instead, but the XML parsing here is a small substring match
  (`parse_enabled_from_xml`), not a real XML parser - a task definition whose `<Enabled>` element
  is nested inside unexpected surrounding markup (not produced by `schtasks` itself in any known
  version) would read as `Unsupported` rather than misparsing, which is the fail-closed direction,
  but is noted as a simplification.
- `run` (what an installed service definition actually invokes once enabled) remains the E02-S01
  stub - installing and enabling the Guardian service today runs a binary that prints one line
  and exits; the pressure/anomaly detection loop is E15-S03/S04's scope, not wired here. This
  matches this document's own established "primitive delivered, no orchestrator yet" precedent
  for E14's detection modules.
- No CLI/TUI surface exposes Guardian install/enable/status yet - only the `cancellai-guardian`
  binary's own subcommands do. Per `docs/architecture/TARGET.md`/`docs/PRODUCT.md`, Guardian is
  its own client of the shared engine, so this is the expected shape at this stage, not a gap
  against this story's AC.

## Verifier verdict

(pending - independent review runs at epic scope once every E15 story is `ready_for_review`)
