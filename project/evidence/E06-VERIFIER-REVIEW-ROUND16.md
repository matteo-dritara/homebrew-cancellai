Review-Scope: epic
Round: 16
Verifier: Codex
Date: 2026-09-25
Review target: `75ce008..badd59e29da05e35d05bc228c8ed870201e14e5d`

# E06 independent verifier review, round 16

E06-S04 was the only `ready_for_review` story at entry; every other E06 story was already `done`. This round judges its committed eight-item cutover checklist, including the C3 repair after round 15. No cutover or formula adoption occurred.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S04 | PASS_WITH_RESIDUALS | C1-C8 hold on the evidence below. The exact reviewed commit's `tests` run `36104490949` passed, including Homebrew job `107973881655`; an isolated staging reproduction yielded 2.0.0 source, Rust metadata, and an engine/legacy formula. The `rust` run `36104490991` passed native macOS/Linux parity and Windows stable-channel tests. The authenticated `verify-release` rerun is unavailable locally, although the real v1.21.1 release run and prior independent asset checks support C2. Owner authorization remains pending and is required before `done` or adoption. Brief-Checksum: 41346037461d07abb0cb2d459581a90b1a2015696b662e239eacd4529e6f0dbb |

## Checklist and independent counterexamples

| Check | Evidence and result |
| --- | --- |
| C1 dependencies | `project_os.py check` passed. E06-S03/S06-S15, E21, and E22-S01 are `done`; E20-S01 and E17 are also `done`. E06-S14's final independent verdict was `PASS_WITH_RESIDUALS` in round 13; E06-S15's was `PASS` in round 11. PASS. |
| C2 real rehearsal | Public release run `36069385676` for v1.21.1 completed all 11 jobs, including four artifact builds, attestation verification, manifest generation, and publication. The local tag resolves to `db5870568eaf37dd0024eb3f636b9c45d948d580` and contains `cancellai.py`; the public release lists the formula, manifest, four archives, sidecars and SBOMs. `project/evidence/RELEASE-v1.21.1.md` records a successful real `verify-release`, and round 15 independently downloaded and hashed all four archives and compared the formula to its rendering. A direct rerun here stopped at `gh api` because `gh` has no authentication. PASS_WITH_RESIDUAL. |
| C3 2.0.0 installation | `.github/workflows/tests.yml` now runs `stage-candidate`, commits the staged version, builds a stable-channel Rust binary and `git archive` source, renders the local cutover formula, installs it, checks both installed version strings and runs `brew test`. Public run `36104490949` is on exact HEAD `badd59e`; its Homebrew cutover step succeeded. In an independent temporary clone, `stage-candidate` yielded 2.0.0, `cancellai.py version` yielded 2.0.0, Cargo metadata contained version 2.0.0, the archived Python source contained `VERSION = "2.0.0"`, and the rendered formula named version 2.0.0 and installed both commands in the specified roles. This directly addresses round 15's 1.21.1 mismatch. PASS. |
| C4 native partial scans | On exact HEAD, `rust` run `36104490991` passed `parity (macos-latest)`, `parity (ubuntu-latest)`, and `cli-stable-channel (windows-latest)`. The first two run `rust_python_parity.py check`; the Windows job runs the whole CLI suite, including `windows_partial_scan.rs`'s four locked fixtures in default and custom root scenarios. Local parity self-test and check passed for 14 normative fixtures in both root-origin scenarios. PASS. |
| C5 transition window | Independent `render_formula` probes at 2.0.0 and 2.1.0 both installed `cancellai-cli` as `cancellai` and `cancellai.py` as `cancellai-legacy`; the v1.21.1 tag archives `cancellai.py`. PASS. |
| C6 intentional changes | Compared `CHANGELOG.md` Unreleased with `docs/CLI_RUST.md` and `project/cli_inventory.json`: command and flag additions/removals, JSON shape, history behavior, hard links, channel/containment authority, Windows support and configure limitation are disclosed. Focused release/inventory tests: 79 passed and 135 subtests. PASS. |
| C7 rollback | `docs/RELEASING.md`'s “Rolling back the cutover” documents the signed containment notice/refresh, invoking `cancellai-legacy`, and reverting the formula commit. It explicitly says the rollback is documented rather than rehearsed. PASS. |
| C8 owner-bound adoption | E06-S15 passed independent rounds 10-11. A direct call to `cutover_authorization_problems('2.0.0')` on HEAD returns `no owner authorization`; `CUTOVER_AUTHORIZATION.md` is absent. `finalize` checks this before adopting the engine formula, and a status cannot substitute for it. PASS for the required refusal; owner acceptance remains pending. |

SI-019: `check_mutation_boundary.py check` passed over 121 Rust source files, confining the deletion primitive and capability. The reviewed diff changes release CI and version staging, not Rust mutation code. The cutover remains gated by a CR4 Safety Verdict and the separate owner authorization.

No new product defect was reproduced. The known Linux arm64 formula coverage and first real 2.0.0 adoption remain transition risks recorded in E06-S04's evidence; the authenticated rehearsal provenance rerun could not be performed in this worktree. The passing CI and prior independent release-asset checks are evidence, not a claim that a cutover release has already shipped.

## Gates actually run

| Command or check | Result |
| --- | --- |
| `project_os.py check/status/next/review`, `brief E06-S04 --role verifier`, `check_agent_toolchain.py report` | PASS at entry; one reviewable story and no overdue toolchain decision. |
| `gh run list --branch main --limit 5` | UNAVAILABLE: local `gh` has no authentication. Public GitHub Actions API showed `tests`, `rust`, `governance`, and `codeql` successful on exact HEAD; the Homebrew and native parity/Windows job conclusions were inspected individually. |
| `python3 scripts/release.py verify-release --version 1.21.1` | UNAVAILABLE at its remote-tag `gh api` call for the same authentication reason. Prior real successful output and independent asset-byte checks are recorded above. |
| `python3 -m pytest tests -q` | 812 passed, 6 skipped, 954 subtests passed, 1 failure: untracked harness file `.opencode-run/prompt.md` contains a broken relative ADR link. Focused `tests/test_release.py tests/test_cli_inventory.py`: 79 passed, 135 subtests passed. |
| `python3 -m ruff check .`, `python3 -m ruff format --check .`, AGENTS.md `python3 -m mypy ...` | UNAVAILABLE: modules are absent from this local interpreter. Exact HEAD's `tests` lint job passed in CI. |
| AGENTS.md Python script checks, individually | PASS: generated CLI docs, governance, workflows, fixtures, schemas, characterization, differential harness, Rust workspace, mutation boundary, provider compatibility/trust, platforms, Rust/Python parity self-test/check, process, release, release manifest, repository topology, agent skills, process metrics, risk classification, toolchain, skill content, verifier handoff, evidence, safety oracle and EARS. `check_docs.py check` failed only on the untracked harness prompt; `gate_sensitivity.py check` refused the already-failing docs/pytest baseline. |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS locally on macOS. Cross-target Clippy was not required: the reviewed diff changes no Rust source or platform lint surface. |

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/MIGRATION_PYTHON_RUST.md`; `docs/RELEASING.md`; `docs/CLI_RUST.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`; `CHANGELOG.md`; `project/cli_inventory.json`; `project/epics/E06.json`; `project/templates/SAFETY_VERDICT.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND15.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND13.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND11.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND10.md`; `project/evidence/E06-S04/EVIDENCE.md`; `project/evidence/E06-S04/VERIFIER_BRIEF.md`; `project/evidence/E06-S04/SAFETY_VERDICT.md`; `project/evidence/E06-S14/SAFETY_VERDICT.md`; `project/evidence/E06-S15/SAFETY_VERDICT.md`; `project/evidence/RELEASE-v1.21.1.md`; `.github/workflows/tests.yml`; `.github/workflows/rust.yml`.

## Overall verdict and disposition

**PASS_WITH_RESIDUALS — 0 of 1 judged story rejected (0% finding yield).** The measured-yield rule does not require another round. E06-S04 moves to `verification`, and E06 remains `in_progress`, because E06-S04 AC1 requires the owner to record `CUTOVER_AUTHORIZATION.md` bound to this passing Safety Verdict's SHA-256 before the story can be `done` or the 2.0.0 cutover adopted. This review did not create that owner decision, change the live formula, cut a release, or commit.
