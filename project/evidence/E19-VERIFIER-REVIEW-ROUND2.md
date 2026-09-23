Review-Scope: epic
Round: 2
Verifier: Codex
Brief-Checksum: f3743d691096e0e2e9eff43754d06170d6a45d62fa3ccebb7b8a306c2b04e18c

Review target: 5863bbc (repair commit; prior review record e964b98)
Date: 2026-09-23

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E19-S01 | PASS_WITH_RESIDUALS | Round 1 record `project/evidence/E19-VERIFIER-REVIEW.md` passed S01 with disclosed residuals; story is already `done`. This round's target is its S02 consumer. |
| E19-S02 | FAIL | `cargo test --workspace` passed, including the real-socket dashboard suite and `tests/startup_output.rs`; stdout/stderr and opener arguments do not contain the URL. However, Windows `create_private` sets no ACL and explicitly relies on inherited permissions, including for an arbitrary `--url-file` parent; additionally, a write/sync failure in `Launcher::create` returns before constructing the `Launcher` whose `Drop` overwrites the file. Both paths can leave the only credential readable on disk. |

## Findings and required repairs

### F1 - Token files are not guaranteed owner-only on Windows

`rust/crates/cancellai-desktop/src/launch.rs::create_private` sets mode `0600` on Unix. On Windows it passes no security descriptor and uses the destination directory's inherited ACL. The comment's assumption that a path under a user's profile is private is not enforced; `--url-file` accepts any new path, including a directory readable by other local users. A launcher can also inherit a broad ACL if the process temporary directory is shared or reconfigured. Therefore both the URL file and launcher can disclose the dashboard credential to another local user on Windows.

Required repair: create both files with an explicit current-user-only Windows DACL (and required system principals), or fail closed after validating that the effective file ACL is owner-only. Do not treat arbitrary parent-directory inheritance as proof. Add Windows tests that inspect the resulting ACL for both the launcher and `--url-file`, including a destination under a deliberately broader-ACL directory. Preserve Unix `0600` coverage.

Violates ADR-0038's owner-only launcher/URL-file contract and the repair's stated requirement that the token not be exposed to other local users through files.

### F2 - Launcher construction failures leave the token-bearing file behind

`Launcher::create` successfully creates the file, then uses `write!` and `sync_all` with `?`, and only constructs `Launcher { path }` after both succeed. If either operation fails after writing some or all of the redirect, `create` returns `Err` and drops only the raw `File`; no `Launcher::drop` runs, so the temporary HTML remains token-bearing. `main` then reports a generic preparation error and exits. This is a reachable error-path leak not covered by the success-path overwrite tests. `write_url_file` has the analogous partial-write/sync-error shape, though the user's explicit URL-file option is the intended token delivery channel.

Required repair: install an overwrite-on-drop guard immediately after creating the launcher file, before writing any URL bytes, and make it expire on every unsuccessful create path as well as normal exit/first successful page load. Add fault-injected write and sync failure tests that assert the file no longer contains the token. For `write_url_file`, document and test the intended failure semantics; on a failed write, ensure a partial credential is not silently left behind (use an overwrite guard or another fail-closed cleanup strategy consistent with SI-019).

Violates ADR-0038's launcher lifetime contract: the launcher is to be overwritten without the URL on load or exit. The failure exits before either condition is currently guaranteed.

## Other falsification results

- Token generation remains a fresh 256-bit path token. `startup_message` prints only the port and optional file path. The child opener receives the launcher path, not the URL; the `cancellai-cli desktop-api` child receives no dashboard token. No repair-commit code path found prints the token to stdout/stderr or includes it in a process argument.
- The successful launcher page has a `no-referrer` meta policy; dashboard responses include `Referrer-Policy: no-referrer` and `Cache-Control: no-store`. The dashboard binds only to `127.0.0.1`, requires the token path and an exact loopback `Host`, accepts only `GET`, bounds request headers, and escapes rendered engine content. The socket tests exercised allowed/refused host, route, method, query, and header-size cases.
- Browser history retains the tokenised URL. ADR-0038 explicitly discloses that residual; it is accepted for this decision and is not a new finding in this round.
- Launcher overwrite rather than deletion is acceptable and disclosed. `scripts/check_mutation_boundary.py` confirms that deletion is restricted to the SI-019 seam; the desktop shell has no reason to gain a second mutation path. Expiring on first 200 response and on normal Rust drop is coherent, subject to F2's partial-construction failure.
- The dashboard depends only on `cancellai-desktop-api`, whose request vocabulary is read-only; no mutation API is expressible. `cancellai-desktop` is not a dependency of CLI/TUI, so core remains headless. View-model construction copies API summaries and counts delete/observation actions using the CLI's `print_plan_summary` split. CLI/Desktop parity integration tests passed.
- No third independent dashboard mutation path or HTTP mutation surface was found. These findings concern token confidentiality, not an engine mutation bypass.

## Gates run

| Gate | Result |
| --- | --- |
| `python3 scripts/project_os.py check` (session start) | PASS |
| `python3 scripts/project_os.py status`, `next`, `review` | PASS / read-only; queue showed E19-S02 |
| `python3 scripts/check_agent_toolchain.py report` | PASS; 12 managed components, none past review |
| `gh run list --branch main --limit 5` | PASS; five latest workflows shown successful |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS; full workspace suite, including live loopback/child-process desktop tests |
| `cargo deny check` | PASS using `CARGO_HOME=/private/tmp/e19-round2-cargo-home`; advisories, bans, licenses and sources all OK (existing duplicate-version and unmatched-allowance warnings) |
| `python3 -m pytest tests -v` | PASS; 689 tests, 638 subtests |
| Ruff check and format | PASS; pinned tools installed under `/private/tmp/e19-round2-python-tools` |
| mypy required source list | PASS; 27 source files |
| All remaining Python checks from AGENTS.md | PASS: `gen_docs --check`, project/docs/workflows/fixtures/schemas/characterize/diff harness/Rust workspace/mutation boundary/provider compatibility/provider trust/platforms/parity self-test and check/process/release/release manifest/repository topology/agent skills/process metrics/risk classification/toolchain/skill content/verifier handoff/evidence/safety oracle/EARS/gate sensitivity |
| `scripts/check_process.py check` | PASS with existing historical round-ceiling warnings; E19 does not have a ceiling exception |

The initial `cargo deny check` without an isolated Cargo home could not lock the advisory DB in the read-only `/Users/.../.cargo` cache. Retrying with the writable temporary Cargo home completed successfully. Ruff and mypy were initially absent from the environment; installing the repository-pinned development requirements into `/private/tmp` allowed both to pass.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `project/templates/VERIFIER_PROMPT.md`; `project/epics/E19.json`; `project/evidence/E19-S02/VERIFIER_BRIEF.md`; `project/evidence/E19-VERIFIER-REVIEW.md`; `docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md`; `docs/PRODUCT.md`; `docs/architecture/TARGET.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `rust/crates/cancellai-desktop/src/launch.rs`; `rust/crates/cancellai-desktop/src/main.rs`; `rust/crates/cancellai-desktop/src/server.rs`; `rust/crates/cancellai-desktop/src/viewmodel.rs`; `rust/crates/cancellai-desktop/src/render.rs`; `rust/crates/cancellai-desktop/tests/dashboard.rs`; `rust/crates/cancellai-desktop/tests/startup_output.rs`; `scripts/check_mutation_boundary.py`.

## Overall verdict: FAIL

The round-1 stdout leak is repaired, but E19-S02 cannot close while Windows file ACLs are inherited without enforcement and launcher construction failures can leave a token-bearing file. These are the final round's findings; repair them as follow-up work without treating this record as a pass. The story is returned to `in_progress`; epic status remains `in_progress`.
