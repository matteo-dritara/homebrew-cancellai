# Evidence Packet - E02-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (round 1: FAIL, `project/evidence/E02-VERIFIER-REVIEW.md`)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-platform/src/{clock,fs_observer,snapshot}.rs` as added/repaired in this change

## Outcome

PASS (after round 1 repair)

## Round 1 repair - unrepresentable modification time silently became a valid epoch timestamp

Codex's independent review (round 1, `project/evidence/E02-VERIFIER-REVIEW.md`) found:
`SystemFsObserver::observe` treated `meta.modified().ok()` and
`.duration_since(UNIX_EPOCH).ok()` as fully fallible-but-ignorable, `unwrap_or`-ing to
`Timestamp::EPOCH` on any failure. A filesystem/platform that cannot report `mtime`, or a
modification time that predates the Unix epoch (unrepresentable in `Timestamp`'s
seconds-since-epoch encoding), was therefore reported as ordinary metadata carrying a 1970
timestamp - an unknown fact read as a credible, extremely-old one, violating AC2 (the seam
must not abstract away security-critical OS semantics) and SI-008/SI-009/SI-010.

Repair (`rust/crates/cancellai-platform/src/fs_observer.rs`):

- extracted `modification_timestamp(modified: io::Result<SystemTime>) -> Result<Timestamp,
  String>`, taking exactly `meta.modified()`'s own return type so it is directly injectable
  in tests without needing to construct a real `std::fs::Metadata` (which has no public
  constructor);
- `SystemFsObserver::observe` now reports `Observation::Unreadable { reason }` - the same
  typed-unknown variant already used for permission/I/O failures - for either failure mode,
  instead of substituting `Timestamp::EPOCH`;
- three new unit tests inject both failure modes directly (an `ErrorKind::Unsupported`
  `io::Error` standing in for a platform that cannot report `mtime`, and a `SystemTime` one
  second before `UNIX_EPOCH` for the unrepresentable-pre-epoch case) and confirm both produce
  `Unreadable` with a distinguishing reason string, plus one test confirming a normal,
  representable time still converts correctly.
- `docs/development/VERIFICATION_STRATEGY.md` extended to state this behavior explicitly (see
  "Documentation updated" below).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Tests can freeze time and synthesize filesystem facts | `FrozenClock` (`clock.rs`) implements `Clock` with a fixed `Timestamp`. `SyntheticFsObserver` (`fs_observer.rs`) implements `FsObserver` and lets a test `set()` an exact `Observation` (`Metadata`/`Absent`/`Unreadable`) per path; any unset path observes as `Absent`. `tests/determinism.rs` uses both together to build a `Snapshot` from fully synthetic inputs. | PASS |
| AC2 - Production paths still use explicit OS-backed implementations | `SystemClock` reads `SystemTime::now()` directly; `SystemFsObserver` calls `std::fs::symlink_metadata` (never following the final symlink, matching the Python reference's `lstat`-based `observe()`), and now reports `Observation::Unreadable` rather than a fabricated timestamp when the platform's own `mtime` fact cannot be obtained or represented (round 1 repair, above) - preserving rather than abstracting away that OS-level unknown. Neither implementation is hidden behind the trait - both are named, public, real implementations a production call site constructs explicitly, not a default a test double could be silently substituted for. `system_clock_reads_a_plausible_recent_timestamp` and `system_observer_distinguishes_absent_from_a_real_file` exercise them against real OS state (a real temp file, actually created and removed), not just against the trait signature. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story. `FsObserver`'s `Absent`/`Unreadable` split is a direct, deliberate application of SI-008/SI-009/SI-010 to the new Rust seam (mirroring the Python reference's `Scan`/`observe()`), documented as such, though no mutation-capable code consumes it yet.

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
```

All passed: 179 Python tests, 22 subtests, all governance checks; Rust `fmt`/`clippy -D
warnings`/`check`/`cargo deny check` clean. `cargo test --workspace` includes 9 new
`cancellai-platform` tests (5 unit across `clock.rs`/`fs_observer.rs`, 4 integration in
`tests/determinism.rs`), all passing.

Round 1 repair re-verification: `cargo test -p cancellai-platform` now runs 8 unit tests (the
original 5 plus the 3 `modification_timestamp` cases above) and the 4 `determinism.rs`
integration tests - all passing (`cargo test -p cancellai-platform` output: `8 passed; 0
failed` unit, `4 passed; 0 failed` integration). `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo fmt --check`, and `cargo deny check` re-run clean
against the repaired file. `tests/determinism.rs` is the "determinism test repeats
plan generation byte-for-byte" the story's verification contract names -
`two_independent_runs_with_the_same_frozen_inputs_are_byte_identical` builds a `Snapshot`
twice from the same frozen clock and synthetic facts and asserts the pretty-printed JSON is
byte-for-byte equal; `a_different_frozen_reading_changes_the_output` and
`a_single_changed_fact_changes_the_output` prove that equality check is falsifiable rather
than vacuous (changing either input actually changes the output).

Falsification-tested directly, beyond the permanent regression tests above: `Snapshot`'s
`observations` field was temporarily changed from `BTreeMap` to `HashMap` (still iterating
the same set of keys, just via Rust's randomized-hasher default collection). Run three times,
`two_independent_runs_with_the_same_frozen_inputs_are_byte_identical` failed every time, each
with a different observed key order between the two runs in the *same test process* - proving
the ordering non-determinism is real, not theoretical, and that `BTreeMap` (not merely "avoid
using a HashMap, in principle") is load-bearing for AC1's byte-for-byte claim. Reverted before
committing; `cargo test -p cancellai-platform --test determinism` passes again afterward.

## Compatibility

- `SystemFsObserver` uses `std::fs::symlink_metadata`, available identically on macOS,
  Linux, and Windows; no platform-specific branch exists yet (E07's job). `Timestamp` is a
  plain `u64` (seconds since epoch), avoiding any platform-specific `SystemTime`
  serialization concern.

## Performance / operability

- All 9 new tests are in-memory or touch a single small temp file; `cargo test -p
  cancellai-platform` completes in well under a second.

## Documentation updated

- `docs/development/VERIFICATION_STRATEGY.md` - new "Rust: deterministic clock/filesystem
  seams" subsection under Filesystem integration tests (the story's declared documentation
  impact).

## Residual risks

- `Snapshot`/`build_snapshot` are explicitly a stand-in for real plan generation (documented
  as such in `snapshot.rs`'s own doc comment and in this packet) - `SealedPlan` itself
  belongs to E03 (Formal Safety Kernel) and E04 (Single-Pass Inventory Engine), neither of
  which exists yet. This story proves the seam composition is deterministic; it does not
  claim to be the real plan builder, and a reviewer should not read `Snapshot` as
  prematurely committing to that type's final shape.
- `FsObserver::observe` does not yet model everything the Python reference's `Scan` does
  (for example, `MAX_RECORDED_SCAN_ERRORS`-style budgeting for a whole scan scope, C-11's
  self-budget concern) - this story is a per-path observation seam, not the full scan/scope
  abstraction; that composition is expected to land with the inventory engine (E04).
- `Clock`/`FsObserver` are defined in `cancellai-platform` but nothing outside their own
  tests calls them yet - like E02-S03's `Diagnostic`, they are shared infrastructure other
  crates are expected to adopt as they gain real fallible/time-dependent logic, not
  something this story could wire into production call sites that do not exist yet.

## Verifier verdict

Round 1 (Codex, independent): FAIL - see "Round 1 repair" above and
`project/evidence/E02-VERIFIER-REVIEW.md`.

Round 2: not run. Per explicit owner direction, the round 1 finding above was repaired and
the story moved directly to `done` without a second independent verification pass. This
story carries no `safety_obligations` and is CR2, not CR4, so no independent Safety Verdict
is required to close it. This is a self-attested repair, not an independently re-verified
one - recorded here rather than silently presented as re-verified.
