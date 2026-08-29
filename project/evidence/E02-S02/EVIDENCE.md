# Evidence Packet - E02-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (round 1: FAIL, `project/evidence/E02-VERIFIER-REVIEW.md`)
- Change Risk: CR1
- Spec version/commit: `rust/deny.toml`, `.github/workflows/rust.yml` (`quality` job), `scripts/check_workflows.py` as amended in this change

## Outcome

PASS (after round 1 repair)

## Round 1 repair - Docker cargo-deny action on unsupported runners

Codex's independent review (round 1, `project/evidence/E02-VERIFIER-REVIEW.md`) found: the
`quality` job scheduled `macos-latest`/`ubuntu-latest`/`windows-latest`, then ran
`EmbarkStudios/cargo-deny-action`, whose pinned `action.yml` declares `runs.using: docker`;
GitHub only executes Docker container actions on Linux runners, so the macOS/Windows legs
would fail before `cargo deny` ever ran - contradicting the all-platform quality gate ADR-0015
and this story require.

Repair:

- `.github/workflows/rust.yml`'s `quality` job now installs `cargo-deny` with
  `cargo install cargo-deny@0.20.2 --locked` (pinned to the same version verified locally,
  matching `crates.io`'s current `max_stable_version`) and runs `cargo deny check` as a plain
  `working-directory: rust` step - `cargo install` is a first-party, non-container mechanism
  identical across macOS, Linux, and Windows runners, so no platform loses the gate.
- `scripts/check_workflows.py` gained `docker_only_action_errors()` (called from
  `validate_workflows()`), a regression guard maintaining a repository-owned
  `DOCKER_ONLY_ACTIONS` list; a workflow that schedules a listed action into a job whose `os`
  matrix includes `macos-*`/`windows-*` now fails `python3 scripts/check_workflows.py check`
  before it ever reaches CI. Verified the check actually catches the original defect: replayed
  the original `EmbarkStudios/cargo-deny-action` step against the `[macos-latest,
  ubuntu-latest, windows-latest]` matrix through `docker_only_action_errors()` directly
  (not just the finished file) and confirmed it reports the violation; the current
  `rust.yml` (without that action) passes.

## Acceptance Criteria Evidence

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
yet to match most allow-listed licenses). This satisfies the story's own verification
contract ("Quality workflow fails on injected lint and denied dependency fixtures") - proven
directly above per-AC, not merely asserted.

Round 1 repair re-verification: `cargo-deny` (0.20.2) installed locally via `cargo install
cargo-deny@0.20.2 --locked` (the same command `.github/workflows/rust.yml` now runs), then
`cargo deny check` from `rust/` - clean. `python3 scripts/check_workflows.py check` passes
with the new `docker_only_action_errors()` guard active.

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

Round 1 (Codex, independent): FAIL - see "Round 1 repair" above and
`project/evidence/E02-VERIFIER-REVIEW.md`.

Round 2: not run. Per explicit owner direction, the round 1 finding above was repaired and
the story moved directly to `done` without a second independent verification pass (CR1, no
safety obligations; ADR-0014 permits up to two review rounds but does not mandate a second
one be spent here). This is a self-attested repair, not an independently re-verified one -
recorded here rather than silently presented as re-verified.
