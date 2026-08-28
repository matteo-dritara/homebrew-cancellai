# Evidence Packet - E02-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E02)
- Change Risk: CR2
- Spec version/commit: `rust/crates/cancellai-platform/src/{clock,fs_observer,snapshot}.rs` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Tests can freeze time and synthesize filesystem facts | `FrozenClock` (`clock.rs`) implements `Clock` with a fixed `Timestamp`. `SyntheticFsObserver` (`fs_observer.rs`) implements `FsObserver` and lets a test `set()` an exact `Observation` (`Metadata`/`Absent`/`Unreadable`) per path; any unset path observes as `Absent`. `tests/determinism.rs` uses both together to build a `Snapshot` from fully synthetic inputs. | PASS |
| AC2 - Production paths still use explicit OS-backed implementations | `SystemClock` reads `SystemTime::now()` directly; `SystemFsObserver` calls `std::fs::symlink_metadata` (never following the final symlink, matching the Python reference's `lstat`-based `observe()`). Neither is hidden behind the trait - both are named, public, real implementations a production call site constructs explicitly, not a default a test double could be silently substituted for. `system_clock_reads_a_plausible_recent_timestamp` and `system_observer_distinguishes_absent_from_a_real_file` exercise them against real OS state (a real temp file, actually created and removed), not just against the trait signature. | PASS |

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
`tests/determinism.rs`), all passing. `tests/determinism.rs` is the "determinism test repeats
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

PENDING - epic E02 review runs once every story in E02 is `ready_for_review` (at most twice per epic, per ADR-0014).
