# Evidence Packet - E15-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (Codex, per-epic review once every E15 story is `ready_for_review`)
- Change Risk: CR1
- Spec version/commit: `project/epics/E15.json`'s E15-S02 story contract

## Outcome

PASS (executor self-assessment; independent verification pending). Implements
`cancellai_guardian::notification`: OS-appropriate notification delivery (`osascript` on macOS,
`notify-send` on Linux, `msg.exe` on Windows) with a terminal fallback on any failure. Not yet
wired to a live orchestrator, matching this crate's own detection-module precedent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "Notifications never include sensitive transcript/source content." | By construction: `NotificationKind` (`rust/crates/cancellai-guardian/src/notification.rs`) has zero `String`/`PathBuf`-typed fields - every variant's payload is `PressureState`, `AnomalySeverity`, or a plain `u32` count. `render` is the only function that produces text and is a fixed match over the closed enum, so no caller-supplied string can ever reach a notification body. Redundant defense-in-depth test `every_kind_renders_privacy_safe_text` asserts every variant's rendered title/body contains no `/`, `\`, or `~` (path-like markers); `rendered_body_never_exceeds_a_short_bounded_length` asserts every body stays under a small fixed bound, which a template that started interpolating unbounded caller content would violate. | PASS |
| AC2 "Notification unavailability does not trigger stronger remediation." | Structural: `NotificationOutcome` (`Delivered`/`Fallback`) carries no error variant and no field a caller could read as "try something stronger"; the module imports no `AuthorityLevel`/`ActionClass` type (grep-verified: no such import anywhere in `notification.rs`), the same "cannot express an authority decision" argument `pressure`/`baseline` already make for SI-027. `a_failed_system_delivery_falls_back_to_the_terminal_not_an_error` proves a failed OS-native delivery becomes `Fallback`, not a panic or an `Err` a caller could branch on; `a_successful_system_delivery_never_touches_the_fallback` proves the fallback is not called redundantly. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Notification payload privacy tests." | `notification::tests::every_kind_renders_privacy_safe_text` (all six `NotificationKind` variants), `notification::tests::rendered_body_never_exceeds_a_short_bounded_length`. Both pass; the first caught a real defect during development (see Method defects). | PASS |

## Safety Evidence

The story names no `safety_obligations`. Adversarial cases considered for this CR1 change
(observational only, cannot influence mutation authority):

| Case | Expected under the contract | Test |
| --- | --- | --- |
| OS-native mechanism missing/refuses (binary absent, no display session, `notify-send` daemon unreachable, ...) | Silent fallback to terminal, never a panic, never an escalated action | `a_failed_system_delivery_falls_back_to_the_terminal_not_an_error`; `notifier_end_to_end_never_panics_regardless_of_real_os_availability` (best-effort real-environment run) |
| Program/body text containing AppleScript-special characters (`"`, `\`) | Correctly escaped, never breaking the generated script | `applescript_escaping_neutralizes_quotes_and_backslashes` |
| A future template edit accidentally interpolating caller content | Caught by the redundant privacy/length checks even though the structural guarantee (no String field) should make this impossible today | `every_kind_renders_privacy_safe_text`, `rendered_body_never_exceeds_a_short_bounded_length` |
| Shell injection via title/body content | Not applicable - Linux/Windows sinks pass title/body as separate `std::process::Command` argv elements, never a shell string; macOS embeds them in an AppleScript literal (escaped, not shell-interpreted) | By construction, documented in the module's own doc comments |
| Second mutation-authority path | None introduced - module has no dependency on `cancellai-safety`/`cancellai-platform` at all | `scripts/check_mutation_boundary.py check` (unchanged) |

## Verification Commands

```text
$ cd rust && cargo fmt --check
(clean)

$ cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
(no warnings)

$ cd rust && cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.89s

$ cd rust && cargo test --workspace
... every crate: test result: ok. <N> passed; 0 failed ...
cancellai-guardian: test result: ok. 121 passed; 0 failed; 0 ignored
  (includes notification::tests::* [7 new])

$ cd rust && cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 101 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs
deletes anything, only rust/crates/cancellai-platform/src/mutation.rs,
rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does

$ python3 scripts/check_docs.py check
docs OK: 476 Markdown files; local links and safety IDs are consistent

$ python3 scripts/gen_docs.py --check
docs/CLI.md is up to date.
```

### Not run locally (unavailable tooling)

- Cross-target clippy (`--target x86_64-unknown-linux-gnu` / `--target x86_64-pc-windows-gnu`):
  same pre-existing local gap as E15-S01's own evidence packet (missing C cross-compiler for
  `cancellai-store`'s bundled `rusqlite`, a transitive dependency of `cancellai-guardian`). CI's
  per-platform native runners must run this before merge.
- `mypy`/`ruff`/the Python reference test suite: not run - this story touches only `rust/`,
  `docs/`, `CHANGELOG.md` and `project/`; `cancellai.py` is untouched.

## Compatibility

- No provider/schema surface touched.
- Delivery mechanism is platform-selected at compile time (`#[cfg(target_os = "...")]` per
  `impl NotificationSink for SystemNotificationSink` block); every host runs the full test suite
  (the pure/testable parts of all three platforms' code are `#[cfg(any(test, target_os = "..."))]`
  where platform-specific, matching `service`'s own convention), while the platform-selected
  production implementation is exercised end-to-end only on its own OS
  (`notifier_end_to_end_never_panics_regardless_of_real_os_availability`, real on this executor's
  own macOS machine as part of this run).

## Performance / operability

- One notification triggers at most one external process invocation (system attempt) plus, only
  on failure, one `eprintln!` - no loop, no retry, no blocking wait beyond the one subprocess call.

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md`: added a new "Notifications" section following the
  established per-story documentation pattern (E14-S01..S04, E15-S01) - the story's own declared
  documentation impact names only `docs/PRODUCT.md`; this additional update follows `AGENTS.md`'s
  "add more if implementation changes more contracts."
- `docs/PRODUCT.md`: added "privacy-safe notifications" to the PREVENT step of the product value
  ladder (the only existing place Guardian's own capability set is enumerated in prose).
- `CHANGELOG.md`: added an `### Added` entry under `## [Unreleased]`.

## Method defects

- none. (During development, the initial `PressureChanged` template read "Disk/budget pressure is
  now {state:?}." - `every_kind_renders_privacy_safe_text`'s path-marker check correctly flagged
  the literal `/` in "Disk/budget" as a false positive against the *word*, not a real leak; fixed
  by rewording to "Disk or budget pressure..." rather than loosening the check. This is normal
  test-driven iteration on a fresh test, not a defect in the delivered code or method - recorded
  here for transparency since the failure was real and reproducible during this session.)

## Residual risks

- Windows delivery uses `msg.exe`, a blocking modal dialog to the target session, not a
  dismissible toast notification - disclosed in `docs/architecture/GUARDIAN_MODEL.md`'s
  Notifications section and in this packet. A real toast needs either a new outer-ring dependency
  (e.g. `notify-rust`/`winrt`) or hand-written COM/WinRT bindings, either of which is a larger,
  separately-justified change (ADR-0019: a new dependency needs a story naming what it replaces).
- No caller wires this module to the live pressure/forecast/baseline/structural detection outputs
  yet - only the delivery mechanism itself is implemented and tested. Wiring it into an actual
  Guardian detection loop is E15-S03/S04's scope (the bounded remediation planner and its audit
  trail), matching this crate's own "primitive delivered, no orchestrator yet" precedent for every
  other detection module in this epic.
- The macOS/Linux/Windows real-delivery smoke coverage
  (`notifier_end_to_end_never_panics_regardless_of_real_os_availability`) only ran for real on
  this executor's own macOS machine; it is environment-tolerant by design (asserts no panic and a
  defined outcome, not which outcome), so it will also run - for the first time for real - on
  Linux/Windows CI without requiring a code change.

## Verifier verdict

(pending - independent review runs at epic scope once every E15 story is `ready_for_review`)
