# Evidence Packet - E02-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E02)
- Change Risk: CR1
- Spec version/commit: `rust/deny.toml`, `.github/workflows/rust.yml` (`quality` job) as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Warnings are denied in CI for project code | `.github/workflows/rust.yml`'s new `quality` job runs `cargo clippy --workspace --all-targets --all-features -- -D warnings` on macOS/Linux/Windows. Falsification-tested locally: a temporary `if x == true { true } else { false }` added to `cancellai-model` was rejected (`error: this if-then-else expression returns a bool literal` / `error: equality checks against true are unnecessary`, both promoted to hard errors by `-D warnings`), then removed; `cargo clippy` is clean again afterward. | PASS |
| AC2 - Unknown registries/git sources are denied unless explicitly approved | `rust/deny.toml`'s `[sources]` table sets `unknown-registry = "deny"` and `unknown-git = "deny"`, with only `crates.io` in `allow-registry` and an empty `allow-git`. Falsification-tested: temporarily emptying `allow-registry` (making even crates.io "unknown") against a real dependency (`once_cell`, added temporarily to `cancellai-model`) produced `sources FAILED`; restoring the real config produced `sources ok` again. | PASS |
| AC3 - License allowlist is documented | `rust/deny.toml`'s `[licenses] allow` list matches ADR-0015 exactly (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib) and is cross-referenced from `AGENTS.md`, `docs/security/SUPPLY_CHAIN.md`, and ADR-0015 itself - one definition, reused, not restated inconsistently in multiple places. Falsification-tested: temporarily narrowing the allow-list to just `Apache-2.0` while `once_cell` (`MIT OR Apache-2.0`) was a **real, non-dev** dependency of `cancellai-model` produced `licenses FAILED` with `rejected: license is not explicitly allowed`; restoring the full list produced `licenses ok` again. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story; no mutation-capable code exists in the workspace.

## Verification Commands

```text
# Python governance (repository-wide, unaffected by this Rust-only change)
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
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
warnings`/`check`/`test`/`cargo deny check` all clean (`advisories ok, bans ok, licenses ok,
sources ok`, with expected `license-not-encountered` notes since no real dependency exists
yet to match most allow-listed licenses). `cargo-deny` (0.20.2) was installed via `brew
install cargo-deny` for this local verification; CI installs it via the
`EmbarkStudios/cargo-deny-action` step added to `.github/workflows/rust.yml`. This satisfies
the story's own verification contract ("Quality workflow fails on injected lint and denied
dependency fixtures") - proven directly above per-AC, not merely asserted.

## Compatibility

- No platform-specific behavior. `deny.toml`'s `[graph]` section does not restrict targets,
  so all platforms' dependency graphs are checked uniformly.

## Performance / operability

- `cargo deny check` completes in well under a second against this dependency-free
  workspace; cost grows with real dependencies added in later epics, not with this story.

## Documentation updated

- `docs/security/SUPPLY_CHAIN.md` - "Dependency policy after Rust bootstrap" section now
  states the policy is implemented (`rust/deny.toml`, ADR-0015), not merely planned (the
  story's declared documentation impact).
- `docs/development/RELEASE_GATES.md` - the release evidence packet's "dependency/security
  scan summary" bullet now names `cargo deny check` as the Rust-side mechanism (the story's
  other declared documentation impact).
- `AGENTS.md` - "Current Rust checks" section rewritten: `fmt`/`clippy`/`deny` are now
  enforced, not a future placeholder.

## Residual risks

- `cargo-deny`'s `advisories` check has nothing to check yet (zero external dependencies in
  the real workspace) - it is exercised as "ok" trivially today, and becomes load-bearing the
  first time a real dependency is added. This is expected and stated in `deny.toml`'s own
  comment, not a gap this story could close earlier (there is nothing to audit yet).
- `bans.multiple-versions` is `"warn"`, not `"deny"` (cargo-deny's own template default,
  intentionally kept): multiple versions of the same crate are a normal, often-unavoidable
  consequence of a real dependency tree, and denying them by default tends to produce noisy,
  low-signal CI failures. This is a considered choice, not an oversight, and remains
  revisitable once real dependencies exist to observe the actual pattern.

## Verifier verdict

PENDING - epic E02 review runs once every story in E02 is `ready_for_review` (at most twice per epic, per ADR-0014).
