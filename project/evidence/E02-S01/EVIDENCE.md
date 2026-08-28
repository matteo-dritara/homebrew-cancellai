# Evidence Packet - E02-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E02)
- Change Risk: CR1
- Spec version/commit: `rust/` workspace + `docs/adrs/0015-rust-workspace-toolchain-and-repository-layout.md` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Workspace builds on macOS, Linux, and Windows tier-1 toolchains | `rust/Cargo.toml` declares `edition = "2024"`, `rust-version = "1.85.0"` (ADR-0015). Verified locally on macOS (this environment): `cargo check --workspace --all-targets`, `cargo build --workspace`, `cargo test --workspace`, `cargo fmt --check`, and `cargo clippy --workspace --all-targets --all-features -- -D warnings` all pass with zero warnings against the installed 1.94.0 toolchain (which satisfies the 1.85.0 MSRV floor). No platform-specific code exists yet, so nothing in the workspace is expected to behave differently on Linux/Windows; `.github/workflows/rust.yml` runs `cargo check --workspace --all-targets` on macOS, Linux, and Windows, each against MSRV 1.85.0 and current stable (6 combinations) - this is the CI matrix verification the story names, since Linux/Windows toolchains are not available in this local environment. | PASS (macOS verified locally; Linux/Windows verified by CI per the story's own verification contract) |
| AC2 - Crate dependency direction follows target architecture and has no cycles | All 12 crates from `docs/architecture/TARGET.md`'s workspace diagram exist under `rust/crates/`, wired with real `path` dependencies (each crate's `src/lib.rs`/`main.rs` has a genuine `use other_crate as _;` for every declared dependency, so the graph is what actually compiles, not just what Cargo.toml claims). `cargo tree --workspace` confirms the graph matches TARGET.md exactly (`cancellai-model` at the base; `cancellai-safety`/`inventory`/`provider-api`/`store`/`platform` depend only on `model`; `provider-claude`/`provider-codex` depend on `model`+`provider-api`+`inventory`; `policy` depends on `model`+`safety`; `cli`/`tui` depend on everything except each other and `guardian`; `guardian` depends on `model`+`safety`+`policy`+`store`). Cargo itself refuses a cyclic workspace graph (a hard compile error), so the successful `cargo check` is itself proof of acyclicity; `scripts/check_rust_workspace.py` additionally does an explicit DFS cycle check with a clearer error message, and `tests/test_rust_workspace.py::test_checker_detects_a_dependency_cycle` proves that check actually fires on synthetic cyclic input, not only that the real graph happens to be acyclic. | PASS |
| AC3 - No provider-specific code is placed in core model/safety crates | `cancellai-model` and `cancellai-safety` have zero real code yet (module-doc-comment-only skeletons) and, structurally, cannot depend on any `cancellai-provider-*`/UI/store crate - their `Cargo.toml` `[dependencies]` tables only ever reference each other. `scripts/check_rust_workspace.py` enforces this mechanically (`ISOLATED_CRATES = {"cancellai-model", "cancellai-safety"}`), and `tests/test_rust_workspace.py::test_checker_detects_model_depending_on_a_provider_crate` proves the check actually fires - this holds going forward as real code lands, not only today when there is nothing to violate it with. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story; no mutation-capable code exists in the workspace yet (every crate is a documented skeleton). The `unsafe_code = "forbid"` workspace lint (ADR-0015) was falsification-tested directly: a temporary `unsafe { core::hint::unreachable_unchecked() }` block added to `cancellai-model` during this work was rejected by `cargo check` with `error: usage of an 'unsafe' block ... requested on the command line with -F unsafe-code`, then removed before committing - the lint is confirmed to actually fire, not merely declared.

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/characterize.py check
python3 scripts/diff_harness.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_process.py check
python3 scripts/release.py check

# Rust workspace (from rust/)
cargo check --workspace --all-targets
cargo build --workspace
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

All passed (179 Python tests, 22 subtests; Rust: 0 warnings from clippy/fmt/check/build/test
across all 12 crates). `scripts/check_rust_workspace.py check` and its new tests
(`tests/test_rust_workspace.py`, 8 cases) are wired into `.pre-commit-config.yaml` and
`.github/workflows/tests.yml`; `.github/workflows/rust.yml` is the new cross-platform `cargo
check` gate the story's verification contract names.

Run inside the same local Python virtualenv as prior stories (system Python 3.13 is
externally managed per PEP 668); Rust toolchain (`rustc`/`cargo`/`rustfmt`/`clippy` 1.94.0,
installed via `rustup`) was already present in this environment.

## Compatibility

- Rust edition 2024, MSRV 1.85.0 (ADR-0015). No platform-specific code exists yet to have
  compatibility implications; the cross-platform CI matrix exists precisely to catch the
  first one that does, at the epic it lands in.

## Performance / operability

- `cargo check --workspace --all-targets` completes in well under a second locally (12 tiny
  skeleton crates, zero external dependencies).

## Documentation updated

- `docs/architecture/TARGET.md` - notes that E02-S01 created the skeleton at `rust/crates/`
  and links to ADR-0015 (the story's declared documentation impact).
- `docs/adrs/0015-rust-workspace-toolchain-and-repository-layout.md` - committed separately,
  ahead of this story, recording the toolchain/edition/MSRV/`unsafe`/CI/license decisions
  this story implements (see that ADR's own commit).
- `AGENTS.md` - new "Current Rust checks" section (replacing the placeholder "Future Rust
  checks" section, since E02-S01 now exists) naming the current minimum gate and what
  E02-S02 will add.
- `.gitignore` - `/rust/target/` (build artifacts).

## Residual risks

- `scripts/check_rust_workspace.py`'s dependency-direction check only mechanically enforces
  the one rule expressible purely from the Cargo.toml graph (model/safety isolation). The
  other three forbidden-direction rules in `docs/architecture/TARGET.md` (provider adapters
  may not bypass the safety executor; UI may not access raw provider roots for mutation;
  network/knowledge may not receive direct mutation authority) describe runtime behavior no
  crate has yet, and cannot be checked statically until there is behavior to check - they
  remain design intent enforced by review until then, not yet by CI.
- `cargo-deny`, RustSec audit, and `clippy`/`fmt` CI enforcement are explicitly E02-S02's
  scope, not this story's; `cargo fmt --check`/`cargo clippy -D warnings` were run locally
  and pass cleanly, but nothing yet fails CI if a future change violates them until E02-S02
  lands.
- Linux and Windows builds are verified by CI (`.github/workflows/rust.yml`), not locally in
  this environment, which only has a macOS toolchain available. The workspace contains no
  platform-specific code today, so this is a low-risk gap, but it is a gap the executor
  could not close itself and a reviewer should confirm the CI run is actually green before
  treating AC1 as fully satisfied.

## Verifier verdict

PENDING - epic E02 review runs once every story in E02 is `ready_for_review` (at most twice per epic, per ADR-0014).
