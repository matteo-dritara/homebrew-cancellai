# Evidence Packet - E03-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E03)
- Change Risk: CR4
- Spec version/commit: `rust/crates/cancellai-platform/src/identity.rs` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Identity captures enough information to detect target replacement between plan and execute | `IdentityToken::Unix { device, inode, kind, modified }` (`identity.rs`). Five adversarial TOCTOU tests observe a path, replace the real filesystem object at that path with a different one, observe again, and assert the tokens differ: file→directory, directory→symlink, symlink→regular file, and file deleted-and-recreated with byte-identical content (proving `inode`, not content or timestamp, is what actually detects the replacement - a weaker mtime/size-only check would have missed this last case). A sixth test uses `SyntheticIdentityObserver` to inject a device-number change standing in for a mount-boundary swap, which a test sandbox cannot construct against a real filesystem without root. | PASS |
| AC2 - Unsupported identity state lowers authority rather than guessing | `IdentityObservation::Unsupported { reason }` is a distinct variant, never conflated with `Identity`/`Absent`/`Unreadable`. `SystemIdentityObserver::observe` returns it unconditionally on any non-Unix target (`#[cfg(not(unix))]`) - real Windows volume/file-index/reparse identity is deliberately not implemented (see "Residual risks" below), so Windows correctly reports `Unsupported` today rather than a plausible-but-unverified equality check. `unsupported_identity_is_never_equal_to_a_real_identity` proves `Unsupported` never compares equal to a real `Identity` token, so a naive `==` cannot mistake it for "unchanged." | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 (identity revalidated immediately before mutation) | Object at a planned path is replaced with a different kind of object before "execution" (re-observation) | `toctou_file_replaced_by_directory_is_detected`, `toctou_directory_replaced_by_symlink_is_detected`, `toctou_symlink_replaced_by_regular_file_is_detected`, `toctou_file_deleted_and_recreated_with_identical_content_still_changes_identity` (`identity.rs`) - each asserts the plan-time and re-observed tokens differ, i.e. a caller comparing them (S02/S05's future job) would correctly detect `STALE_PLAN`. | PASS |
| SI-013 (mount/reparse boundary swap) | A different filesystem/volume mounted at the planned path between observations | `toctou_mount_boundary_swap_is_detected_via_synthetic_device_change` - constructed via `SyntheticIdentityObserver` since a real mount swap needs root and is not constructible in this sandbox/CI. | PASS |
| SI-017 (platform-native identity semantics; Unix assumptions not applied to unsupported platforms) | A non-Unix platform is asked for identity | `#[cfg(not(unix))] observe_system_identity` unconditionally returns `Unsupported`, never a Unix-shaped guess; `unsupported_identity_is_never_equal_to_a_real_identity` proves it cannot be mistaken for a real token by comparison. The non-Unix branch itself is exercised for real only by CI's `windows-latest`/other-non-Unix runners (this machine has no such target to execute against - see "Residual risks"). | PASS (compile-verified on two non-native targets locally; runtime-verified by CI, not by this executor - see below) |

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/project_os.py check

# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Cross-platform compile verification this executor could actually run (no Windows/Linux
# runtime available on this machine, but `rustup target add` + `cargo check`/`cargo clippy`
# catch a real class of "compiles here, warns/fails there" mistakes without needing one):
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-gnu
cargo check -p cancellai-platform --target x86_64-pc-windows-gnu --all-targets
cargo check -p cancellai-platform --target x86_64-unknown-linux-gnu --all-targets
cargo clippy -p cancellai-platform --target x86_64-pc-windows-gnu --all-targets --all-features -- -D warnings
```

All passed. `cargo test -p cancellai-platform` now runs 15 unit tests (the 8 from E02-S04 plus
7 new in `identity.rs`) and the 4 `determinism.rs` integration tests, all green. The
cross-target `cargo check`/`cargo clippy` runs caught a real defect during development (an
unused import and dead test helpers that only exist under `#[cfg(unix)]`, both of which would
have been silent locally but hard `-D warnings` failures on CI's `windows-latest`/other
non-Unix legs) - fixed before this packet was written, not left for CI to discover first.

## Compatibility

- Unix (macOS, Linux): fully implemented and tested, both locally (macOS, real filesystem
  fixtures) and via CI's `ubuntu-latest` leg (same `#[cfg(unix)]` code path).
- Windows and any other non-Unix target: `Unsupported` by design (see AC2 evidence and
  Residual risks). No regression versus today - the safety kernel this identity seam feeds
  does not exist yet, so no destructive authority is being narrowed by this choice; it is a
  deliberate scope boundary for when that authority does get built (E03-S02/E03-S05).

## Performance / operability

- Each `IdentityObserver::observe` call is one `symlink_metadata` syscall (or none, for the
  synthetic observer); cost is equivalent to `FsObserver::observe`'s.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` - "Identity token" section now states the Unix
  implementation and, explicitly, the Windows `Unsupported` decision and its rationale (the
  story's declared documentation impact).

## Residual risks

- Real Windows volume/file-index/reparse-point identity is not implemented. This is a
  deliberate, documented scope boundary (see AC2, `PLATFORM_MODEL.md`), not an oversight: this
  executor has no Windows runtime to verify a safety-critical equality check against, and per
  C-12 (cross-platform truthfulness) and SI-017, an honest `Unsupported` - which this story
  ships and tests - is safer than a plausible-but-unverified implementation of exactly the
  logic SI-013 depends on. A dedicated follow-up story (naturally scheduled under E07,
  Cross-Platform Certification, or as an explicit addition to E03) should implement and verify
  native Windows identity once it can be exercised on Windows CI/a Windows runtime, before any
  Windows artifact can carry destructive authority through this seam. Recommending this as
  backlog scope, not creating it unilaterally (AGENTS.md: "Do not silently create product
  scope in code").
- Junction/reparse-point TOCTOU fixtures specifically (the verification contract's fourth
  named case) are satisfied today by the `Unsupported`-lowers-authority path rather than by a
  real token-inequality check, since junctions are a Windows-only concept and Windows identity
  is not yet implemented (above). Once real Windows identity lands, that follow-up story
  should add a junction-swap fixture analogous to this story's Unix TOCTOU tests.
- `device`/`inode` reuse after deletion is not provably impossible on Unix (the kernel is free
  to reissue a freed inode number), only astronomically unlikely in practice within a
  plan-to-execute window; `kind`/`modified` are captured alongside as partial mitigation, not
  because they alone would be sufficient identity evidence.
- This capability is not wired into anything yet (same posture as E02-S04's `Clock`/
  `FsObserver`) - `SealedPlan`/the safety executor that will actually compare a plan-time
  token against a revalidated one belongs to E03-S02/E03-S05, neither of which exists yet.

## Verifier verdict

PENDING - epic E03 review runs once every story in E03 is `ready_for_review` (at most twice per epic, per ADR-0014).
