# Evidence Packet - E09-S05

- Commit/PR: the MSRV resolver fix on `main`
- Executor: Claude
- Independent verifier: the MSRV leg of `.github/workflows/rust.yml`, which had been failing since E09-S01
- Change Risk: CR1
- Spec version/commit: `project/epics/E09.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the workspace resolves MSRV-aware | `rust/Cargo.toml` sets `resolver = "3"`. Cargo's own output is the evidence it applied: `Locking 5 packages to latest Rust 1.85.0 compatible versions`, and for the crate it could not take, `instability v0.3.13 -> v0.3.10 (available: v0.3.13, requires Rust 1.88)`. | PASS |
| AC2 - nothing in the lock is above the MSRV | `cargo metadata` over the whole graph: four packages exceeded 1.85.0 before (`darling`, `darling_core`, `darling_macro` at 0.24.1 requiring 1.88.0, `instability` 0.3.13 requiring 1.88); none after. | PASS |
| AC3 - an impossible constraint fails at update time | This is what resolver 3 does by construction, and what resolver 2 did not: the failure moves from the MSRV job, which runs after a push, to `cargo update`, which runs while the change is being made. Not exercised here - no dependency in this workspace is MSRV-impossible - and recorded as unexercised rather than claimed. | PARTIAL |
| AC4 - the quality set passes unchanged | `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace` (0 failures across 22 test binaries), `cargo deny check` (`advisories ok, bans ok, licenses ok, sources ok`, including the newly added `fnv`). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A dependency reaching a kernel-ring crate through a downgrade | It does not: `instability` and `darling` are build-time macro dependencies of `ratatui`, which only `cancellai-tui` depends on - outer ring, ADR-0019. The kernel crates' dependency sets are unchanged, which `cargo tree` and `check_rust_workspace.py check` both confirm. | PASS |
| n/a | A downgrade that quietly loosens the supply-chain policy | `cargo deny check` re-run in full: advisories, bans, licenses and sources all clean. `fnv` (MIT OR Apache-2.0) is new to the graph and is on the ADR-0015 allow-list. | PASS |

## Verification Commands

```text
cargo update -p instability -p darling   -> 5 packages locked to Rust 1.85.0 compatible versions
cargo metadata                           -> no package above rust-version 1.85.0
cargo fmt --check                        -> clean
cargo clippy --workspace --all-targets --all-features -- -D warnings -> clean
cargo check --workspace --all-targets    -> clean
cargo test --workspace                   -> 0 failed
cargo deny check                         -> advisories ok, bans ok, licenses ok, sources ok
```

## Compatibility

- No product behaviour change. `instability` is a documentation-attribute macro and `darling` its
  derive helper; neither appears in compiled behaviour that this workspace's tests exercise.
- The MSRV itself is unchanged at 1.85.0, so the compatibility promise in ADR-0015 is restored
  rather than renegotiated.

## Performance / operability

- Unchanged. Build time is dominated by the workspace's own crates.

## Residual risks

- **A local toolchain that is newer than the MSRV cannot detect this class of break by compiling.**
  Local `rustc` here is 1.94 and no 1.85 toolchain is installed, so the check that actually ran was
  `cargo metadata`'s declared `rust_version` rather than a compile. A dependency that compiles
  against 1.85 but declares a lower `rust-version` than it needs would still reach CI.
- **Resolver 3 constrains resolution, not the manifest.** A direct dependency whose own minimum is
  above the MSRV still has to be caught by someone reading the `cargo update` output.
- **The real failure was nobody reading a red workflow.** Two releases were attempted over a main
  whose MSRV job was failing. Resolver 3 prevents this specific recurrence; it does nothing about
  the general one, which is tracked separately.

## Verifier verdict

pending
