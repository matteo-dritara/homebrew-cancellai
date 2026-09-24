Review-Scope: epic
Round: 14
Verifier: Codex
Date: 2026-09-24
Review target: `2ac50ebaeafae6a8e576f1ee43aaa99ada0c1c4c..1f7873fadf809ba6dae19c7058c27f006076e874`

# E06 formal verifier review, round 14

E06-S04 was the only story at `ready_for_review`; all other E06 stories were already `done`. This is the owner-authorized migration-verdict round after E06-S14's round-13 pass. I reconstructed the cutover contract from the committed brief and story, then checked the release authority and partial-scan evidence independently. No release was adopted.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S04 | FAIL | AC1 is unmet: `project/evidence/E06-S04/CUTOVER_AUTHORIZATION.md` does not exist, and `cutover_authorization_problems('2.0.0')` returns `no owner authorization`; the current Safety Verdict also fails. The verification contract is unmet for Windows: `.github/workflows/rust.yml` runs the 14-fixture differential gate only on macOS and Linux, while its Windows stable-channel CLI job does not run the E21-S02 partial-scan fixtures or compare with the frozen reference. The local macOS parity self-test and check passed for all 14 normative fixtures in default and custom root scenarios, but that cannot establish the required native Windows reproduction. Brief-Checksum: b48e620110482c92c727a7f8bad34338f6b6b7fbe228714ce16b6df83afe97e9. |

## Findings and exact required repair

1. **F-14-01, native partial-scan evidence missing on Windows.** The E06-S04 verification contract calls for a native reproduction on each platform in the cutover perimeter using the E21-S02 fixtures, with the engine withholding where the reference withholds. `docs/PLATFORMS.md` names native Windows tier 1; `CHANGELOG.md` says Windows is supported natively; ADR-0039 does not remove Windows from the CLI cutover perimeter. Yet the `parity` job's matrix is `[macos-latest, ubuntu-latest]`. The Windows `cli-stable-channel` job executes different CLI tests; the relevant `cli_behavior.rs` partial-read tests are `#[cfg(unix)]`. Reproduction: inspect the two workflow matrices and search the Windows-eligible tests for `codex-partial-tree`, `claude-partial-project`, and `codex-unreadable-rollout`; none executes those fixtures on Windows. **Repair:** run native Windows partial-scan cases derived from the E21-S02 fixtures in both root-origin scenarios, independently checking complete/partial state, exit status, and zero delete actions against the frozen reference's expected withholding; if Windows is intentionally outside the initial cutover, record that perimeter and change the Windows support/release claims through an owner-approved ADR. This is a verification failure, not proof that Windows currently deletes unsafely.
2. **F-14-02, owner acceptance absent.** E06-S04 AC1 requires the migration Safety Verdict to be accepted. No `CUTOVER_AUTHORIZATION.md` exists. A direct call to `scripts.release.cutover_authorization_problems('2.0.0')` returned the missing-file refusal; `safety_verdict_passes` returned false for the existing file. **Repair:** after an independently passing migration Safety Verdict, the owner must add the authorization for version 2.0.0 bound to that verdict's SHA-256, as E06-S15 and ADR-0040 require. Story status cannot substitute for this decision. The verifier cannot supply the owner's acceptance.

## Other acceptance and safety evidence

- **AC2:** `git tag --list` includes v1.21.0, and the source archive path in `scripts/release.py` preserves the Python reference. `render_formula` installs `cancellai.py` as `cancellai-legacy` through 2.1.0. No 2.0.0 release has been adopted.
- **AC3:** `CHANGELOG.md` Unreleased lists the engine switch and intentional command, flag, JSON, history, hard-link, Windows configure, and channel changes; `docs/CLI_RUST.md` contains the CLI details. `docs/PLATFORMS.md` and `docs/architecture/PLATFORM_MODEL.md` still say the full Windows safety executor refuses every deletion, conflicting with E06-S13 and the changelog; carry that documentation drift to the story that repairs the Windows evidence.
- **AC4 / SI-019:** E06-S06 through S15 dependencies are `done`; ADR-0039 defines the local-ledger cutover perimeter. `check_mutation_boundary.py check` passed: 121 Rust sources scanned, with the raw deletion primitive and its capability confined to the platform mutation and safety executor files. `release.py check` passed for the still-Python live v1.21.0 formula. These checks do not discharge the missing CR4 cutover authorization or Windows reproduction.
- **Falsification performed:** ran the parity comparator's injected-divergence self-test, then its full local fixture check. Both passed; all 14 normative fixtures matched in two root-origin scenarios on macOS. Inspected the Windows-capable CLI tests for partial-read coverage and the workflow matrices rather than treating those local results as a cross-platform proof.

## Gates actually run

| Command / evidence | Result |
| --- | --- |
| `project_os.py check/status/next/review`; `brief E06-S04 --role verifier`; `check_agent_toolchain.py report` | PASS at entry; one reviewable story, no toolchain decision past its review date. |
| `gh run list --branch main --limit 5` | UNAVAILABLE: local `gh` has no authentication. Public GitHub Actions API showed current main `1f7873f`: tests and governance success, Rust and CodeQL still in progress when checked. Pending is not green. |
| `python3 -m pytest tests -q` | 809 passed, 6 skipped, 948 subtests passed, 1 failed: docs link check reads untracked harness file `.opencode-run/prompt.md`, whose relative ADR link does not resolve. |
| `ruff check .`; `ruff format --check .`; `mypy` on all 29 AGENTS.md-listed Python sources | PASS using the pinned project venv tools on PATH. |
| AGENTS.md Python script checks, including parity self-test/check, release, project governance, mutation boundary, safety oracle, EARS, evidence and process | PASS except `check_docs.py check` and `gate_sensitivity.py check`. Both fail because the untracked harness prompt makes the docs/pytest baseline fail. `check_process.py check` passed with its recorded historical cost-ceiling warnings. |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace` | PASS locally on macOS. |
| `CANCELLAI_CHANNEL=stable cargo test -p cancellai-cli --features test-curl`; same with `--features kill-points --test kill_harness` | PASS locally; the kill harness reached all three tests. |
| `cargo deny check` | Initial run could not acquire the advisory DB lock under a read-only home path. With `CARGO_HOME=/private/tmp/e06-round14-cargo-home`, both offline and online `cargo deny check` passed advisories, bans, licenses and sources. |
| Windows-target Clippy; Linux-target Clippy | Windows-target PASS. Linux-target blocked locally because `x86_64-linux-gnu-gcc` is absent for bundled SQLite's build script; this is a host toolchain gap, not a reported Rust lint. |
| Native Linux/Windows E21-S02 fixture reproduction; 2.0.0 installed release and complete release evidence packet | NOT OBSERVED in this workspace. The macOS/Linux parity workflow exists; Windows has no equivalent fixture run. No 2.0.0 cutover release exists. |

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`; `docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/MIGRATION_PYTHON_RUST.md`; `docs/CLI_RUST.md`; `docs/PLATFORMS.md`; `docs/RELEASING.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md`; `docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`; `CHANGELOG.md`; `project/epics/E06.json`; `project/templates/SAFETY_VERDICT.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND13.md`; `project/evidence/E06-S04/EVIDENCE.md`; `project/evidence/E06-S04/VERIFIER_BRIEF.md`; `project/evidence/E06-S04/SAFETY_VERDICT.md`.

## Overall verdict and disposition

**FAIL, 1 of 1 story rejected (100% yield).** E06-S04 returns to `in_progress`; E06 stays `in_progress`; no formula change or release occurs. The existing Python formula remains the rollback. Further review exceeds the normal three-round cost ceiling and requires a recorded owner decision under ADR-0025; this record does not authorize another round or accept these findings as residual risk.
