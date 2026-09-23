# E19 Desktop Experience - verifier review round 1

Review-Scope: epic
Round: 1
Verifier: Codex
Date: 2026-09-23
Review-Target: f1847aa..ff5ba05 (branch `review/e19-round1`)

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E19-S01 | PASS_WITH_RESIDUALS | `Request` admits only hello/document/goodbye, and the desktop API crate has no engine mutation dependencies. I inspected the CLI document builders and integration parity checks; the API calls the same `inventory_doc`, `plan_actions`, and `plan_doc` paths. Raw socket cases exercise wrong/missing/late authentication, version refusal, oversized/split frames, and no mutating request. API request/response schema tests pin v1. `cargo test --workspace` could not complete because spawning the CLI child process is denied by this environment; see gate results. Brief-Checksum: E19-S01 c10ad52d3cec6803f9258a0d0d414c0275502ba53eb46d21c87ef4a34ef06658 |
| E19-S02 | FAIL | `rust/crates/cancellai-desktop/src/main.rs` prints the complete dashboard URL to stdout (`println!("{url}")`); the URL embeds the bearer path token returned by `Dashboard::url()`. Capturing stdout in a launcher log therefore exposes the token to anyone who can read that log and defeats the shell's token secrecy claim. Brief-Checksum: E19-S02 f3743d691096e0e2e9eff43754d06170d6a45d62fa3ccebb7b8a306c2b04e18c |

## Findings and required repairs

### E19-S02 - bearer token printed to stdout

**Reproduction:** `Dashboard::url()` returns `http://127.0.0.1:<port>/<token>`. `main()` writes that full value with `println!` before serving. Redirecting `cancellai-desktop` stdout to a log captures the bearer token verbatim.

**Required repair:** Remove the tokenized URL from stdout/stderr and other logs. Keep it in process memory and pass it directly to the browser opener when `--open` is requested; when the browser is not opened automatically, provide a retrieval/display path that does not persist it in a log. Add a regression test that captures normal startup output and proves the session token is absent. This violates E19-S02's read-only local dashboard boundary and the explicit review requirement to falsify token leakage through stdout/stderr/logs/Referer. Referrer policy is present and correctly suppresses Referer propagation, but does not address this stdout leak.

The no-tray/menu-bar limitation is expressly disclosed by ADR-0038. The dashboard implementation and recorded dependency/license measurements provide an evidence-backed equivalent choice; this limitation alone does not violate either E19-S02 acceptance criterion. The token leak does.

### E19-S01 residual and verification limitation

The API serves connections serially and permits a peer to remain idle for up to 30 seconds; this is disclosed in `docs/architecture/TARGET.md`. The server also has no independent response-size limit, while the client accepts up to 256 MiB. These are residual availability limits for the single-user local API. No bypass into mutation or unauthorized document disclosure was found by source inspection. Workspace integration tests that spawn `cancellai-cli` could not run under the current sandbox, so CI must run them before merge.

## Gate results

### Rust (`rust/`)

- `cargo fmt --check` - PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` - PASS.
- `cargo check --workspace --all-targets` - PASS.
- `cargo test --workspace` - BLOCKED/FAIL in this environment: CLI integration tests failed to spawn `cancellai-cli` with `Operation not permitted (os error 1)`, then failed parsing the empty descriptor stdout. This does not establish a product defect; the relevant end-to-end tests remain unverified here.
- `cargo deny check` - PASS (existing warnings for unmatched allowed licenses and duplicate `hashbrown` versions; command exited successfully).
- `cargo test -p cancellai-desktop-api -p cancellai-desktop` - desktop unit tests passed (12 tests), but dashboard integration tests could not bind loopback sockets (`Operation not permitted (os error 1)`). The package test command stopped there before completing the API package.

### Python / repository gates

- `python3 scripts/project_os.py check`, `status`, `next`, `review` - PASS; both E19 stories were `ready_for_review` before review. The queue also contains unrelated E17-S07.
- `python3 scripts/check_agent_toolchain.py report` - PASS; report found 12 components and no overdue decisions.
- `python3 scripts/gen_docs.py --check`, `check_docs.py check`, `check_workflows.py check`, `check_fixtures.py check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check`, `check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py check`, `check_provider_trust.py check`, `check_platforms.py check`, `rust_python_parity.py self-test`, `rust_python_parity.py check`, `check_process.py check`, `release.py check`, `release_manifest.py check`, `check_repository_topology.py check`, `check_agent_skills.py check`, `process_metrics.py check`, `check_risk_classification.py check`, `check_agent_toolchain.py check`, `check_skill_content.py check`, `verifier_handoff.py check`, `check_evidence.py check`, `safety_oracle.py check`, `check_ears.py check`, `gate_sensitivity.py check` - PASS.
- `python3 -m pytest tests -v` - PASS (689 passed, 637 subtests passed).
- `python3 -m ruff check .`, `python3 -m ruff format --check .` - NOT RUN: `ruff` is not installed (`No module named ruff`).
- Prescribed `python3 -m mypy ...` file list - NOT RUN: `mypy` is not installed (`No module named mypy`).
- `gh run list --branch main --limit 5` - latest four workflows listed were completed/success; this reports main branch state, not CI for this review commit.

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/templates/VERIFIER_PROMPT.md`
- `docs/development/AGENT_PROTOCOL.md`
- `project/epics/E19.json`
- `project/evidence/E19-S01/VERIFIER_BRIEF.md`
- `project/evidence/E19-S02/VERIFIER_BRIEF.md`
- `docs/architecture/TARGET.md`
- `docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/adrs/0006-cli-tui-shared-engine.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `scripts/verifier_handoff.py`
- Rust implementation and tests in `rust/crates/cancellai-desktop-api/`, `rust/crates/cancellai-desktop/`, and `rust/crates/cancellai-cli/src/{desktop.rs,main.rs,cli.rs}`.

## Overall verdict

**FAIL** - E19-S02 is rejected for token disclosure through stdout. E19-S01 passes with the disclosed availability residuals, but its workspace process-spawning integration tests still need to run in CI. With one of two stories rejected (50%), ADR-0025 requires another independent review round after repair.
