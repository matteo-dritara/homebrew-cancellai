Review-Scope: epic
Round: 9
Verifier: Codex

# Independent verifier review — 2026-09-24

- Review target: `351295d..7e35a2a` (`7e35a2a` was the committed HEAD at entry).
- Scope: the two committed, `ready_for_review` briefs E06-S04 and E33-S01. Previously closed E06 stories were not re-judged. E33-S02 remains future work and is blocked by E33-S01's failure.
- Verifier identity: Codex. No production code was changed, no commit was made, and no release was tagged, installed or published.
- Brief-Checksum E06-S04: `76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642`.
- Brief-Checksum E33-S01: `bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9`.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S04 | FAIL | `published_digest_problems` accepted a generated 2.0.0 formula whose engine digests matched synthetic `.sha256` responses while the actual archive bytes had a different SHA-256; the probe recorded zero engine archive downloads. The release manifest is not checked by this path. The owner migration Safety Verdict is still pending. Brief-Checksum: `76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642`. |
| E33-S01 | FAIL | Adding a raw-line-count assertion to the existing 16-process refresh integration test failed: two physical install lines for one signed notice, although replay accepted one and `list` passed. A deterministic store test using two appends against the same head also failed a byte-identity assertion. Brief-Checksum: `bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9`. |

## E06-S04: release identity and cutover decision

**Reproduction.** In a read-only Python probe, `render_formula("2.0.0", source_sha, engines)` supplied three engine resources with SHA-256 `aa…aa`. A mocked HTTPS response returned the real tag-source bytes and the same `aa…aa` in each engine sidecar. The actual simulated engine archive bytes hashed to `0d12c9f388630a692893dc73f48aa392bed36bef50b822c3c3c067198d337a8b`. Both `live_formula_problems` and `published_digest_problems` returned `[]`; no `/releases/download/...tar.gz` archive was requested. `engine_sha256s` reads sidecars only. The code also has no release-manifest cross-check. This is a verification false pass for the bytes the formula would install, despite the new whole-formula shape check. It repeats the still-unmet digest portion of R8-01.

**Required repair.** Fetch and hash each released engine archive independently, check version and target identity against the release manifest, and compare the archive digest, sidecar, manifest and generated formula before accepting or writing the release formula. Make disagreement and unavailable evidence fail closed, with adversarial cases for altered archive, sidecar and manifest. This violates E06-S04's full release evidence and G3 package compatibility obligations, and C-16/SI-019's evidence-gated canonical mutation boundary. The current source-archive hash check and whole-formula equality check passed this probe and should remain.

The live formula remains Python-only at v1.21.0 and the Rust candidate is generated for 2.0.0. Python remains in the tags and the generated cutover formula retains `cancellai-legacy` through 2.1.0. Unreleased and `docs/CLI_RUST.md` disclose the principal intentional differences, including the repaired `--days 0` boundary. However, `project/evidence/E06-S04/EVIDENCE.md` still marks AC1 pending, and the latest Safety Verdict remains FAIL with owner decision PENDING. No 2.0.0 tag/artifact or owner acceptance exists in this checkout. Native release installation and E21-S02 partial-scan reproductions on every cutover platform were not performed in this macOS worktree; local `rust_python_parity.py check` passed. The owner acceptance and platform evidence must be present before AC1 and the verification contract can close.

## E33-S01: concurrent refresh changes a refused/current ledger

**Reproduction.** I temporarily extended `concurrent_refreshes_of_one_notice_leave_one_accepted_install_and_a_replayable_history` with `assert_eq!(read_to_string(tree.log()).lines().count(), 1)`. Running its stable-channel binary with `test-curl,kill-points` spawned sixteen processes; the original assertions passed but the new assertion failed `left: 2, right: 1`. A separate temporary store test appended the same event twice using the empty-history head, read bytes after the first append, then asserted the second stale append did not change them; it failed. Both temporary test edits were removed and tracked source was restored.

`append_event` physically writes before learning that its `prev` is stale. `load_history` skips that line, so replay and authority still use the winner's notice, but a refresh that reports `already current` has changed raw ledger bytes. Stale loser lines also consume the 16 MiB history cap; repeated valid concurrent refreshes can eventually turn a replayable ledger into `Unreadable`. The existing regression asserts one *accepted* incident and successful replay, so it misses the raw-byte and self-budget obligations.

**Required repair.** Serialize the replay/current-sequence decision and durable append across processes, or use an equivalent atomic compare-and-append protocol that never writes a losing line. Under simultaneous identical notices, exactly one process may append; the others must report current without changing bytes. Prove crash/retry leaves the ledger usable or explicitly recoverable, and add a regression comparing physical bytes and line count as well as replay. This violates E33-S01 AC2's byte-identical refusal/current behavior, SI-029's fail-closed usable local state, and the brief's concurrent/crash verification obligations. Signature verification, HTTPS-only fixed system curl and the byte cap remained intact in the inspected path; the normal stable-channel containment suite passed.

## Falsification axes and limits

| Axis | Result |
| --- | --- |
| Path/identity and links | Formula resources are now generated within fixed platform blocks. Fixed system curl and state-root code were inspected; no new path escape was reproduced. |
| Partial reads/permissions | Source download failure is a refusal. A sidecar can authorize a mismatched engine archive without any archive read. Corrupt containment history and trust are refused by existing stable-channel tests. |
| Provider layout drift | E21 partial-scan parity gate passed locally; native per-platform release reproductions remain unobserved here. |
| Concurrency and crash/retry | Sixteen-process refresh produced two raw lines for one notice; store stale-head probe independently reproduced the append-before-rejection mechanism. No process-kill probe of the new ledger append was run. |
| Boundary values and large data | `--days 0` is refused in the CLI suite; feed byte-cap tests passed. Stale lines count against the 16 MiB ledger cap without contributing accepted state. |
| Policy/trust conflict and malformed input | Valid signed notices pass the shared install path; malformed/untrusted/rolled-back feed tests passed. The race is between valid signed requests, not forged content. |
| Platform difference | Native host is macOS. Windows-target Clippy passed; Linux-target Clippy could not build bundled SQLite without `x86_64-linux-gnu-gcc`. CI status was unavailable because `gh` is unauthenticated. No Linux/Windows runtime claim is made from cross-compilation. |

## Gates actually run

| Command / gate | Result |
| --- | --- |
| `python3 scripts/project_os.py check/status/next/review` and both verifier briefs | PASS at entry; exactly E06-S04 and E33-S01 queued. |
| `python3 scripts/check_agent_toolchain.py report` | PASS; no review date expired. |
| `gh run list --branch main --limit 5` | UNKNOWN: CLI requested authentication; workflow conclusions unavailable. |
| `cargo fmt --check`, host `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace` | PASS. |
| Windows-target workspace Clippy `x86_64-pc-windows-gnu` | PASS. Linux-target Clippy attempted but stopped before linting because cross C compiler `x86_64-linux-gnu-gcc` is absent. |
| `cargo deny check` | Initial attempt could not lock the read-only default advisory DB. Re-run as `CARGO_HOME=/private/tmp/e06-cargo-home cargo deny --offline check`, using a writable copy of the existing advisory DB and the local registry cache: PASS, all four checks. |
| `CANCELLAI_CHANNEL=stable cargo test -p cancellai-cli --features test-curl,kill-points --test containment` | PASS: 15 tests. Temporary raw-line-count extension FAILED as described; temporary deterministic store-byte test FAILED as described. Both were removed. |
| `python3 -m pytest tests -q` | 762 passed, 3 skipped, 1 failed: untracked harness `.opencode-run/prompt.md` contains two broken relative links, which `tests/test_docs.py` scans. This was present at entry and is not a product-source edit. |
| `python3 -m ruff check .`, `python3 -m ruff format --check .`, `python3 -m mypy ...` | UNAVAILABLE: `ruff` and `mypy` modules are absent; repository instructions prohibit verifier toolchain installation. |
| Individual Python script gates from AGENTS.md | All passed except `scripts/check_docs.py check` (same untracked `.opencode-run/prompt.md` links) and `scripts/gate_sensitivity.py check` (reports docs/pytest fail on an unmutated tree). Includes `rust_python_parity.py self-test/check`, `release.py check`, `check_process.py check`, `verifier_handoff.py check`, `check_evidence.py check` and `process_metrics.py check`. |

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `project/epics/E06.json`; `project/epics/E33.json`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/AGENT_TOOLCHAIN.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/VERIFICATION_STRATEGY.md`; `docs/development/MIGRATION_PYTHON_RUST.md`; `docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/security/THREAT_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/INCIDENT_RESPONSE.md`; `docs/security/SUPPLY_CHAIN.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md`; `docs/RELEASING.md`; `docs/CLI_RUST.md`; `CHANGELOG.md`; `project/templates/SAFETY_VERDICT.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND8.md`; `project/evidence/E06-S04/EVIDENCE.md`; `project/evidence/E33-S01/EVIDENCE.md`; both stories' `SAFETY_VERDICT.md` files; `scripts/release.py`; `scripts/release_manifest.py`; `.github/workflows/release.yml`; `rust/crates/cancellai-cli/src/cli.rs`; `rust/crates/cancellai-cli/src/containment.rs`; `rust/crates/cancellai-cli/tests/containment.rs`; `rust/crates/cancellai-store/src/containment_state.rs`; `rust/crates/cancellai-safety/src/incident.rs`; `tests/test_release.py`.

## Overall verdict and disposition

**FAIL: 2 of 2 stories rejected (100% yield).** E06-S04 and E33-S01 return to `in_progress`; E33-S02 is blocked on E33-S01. ADR-0025's 10% rule requires another round after repair, subject to an explicit owner decision on the already exceeded cost ceiling. No residual risk was accepted and no epic was closed. The owner migration Safety Verdict acceptance remains pending.
