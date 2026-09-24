Review-Scope: epic
Round: 15
Verifier: Codex
Date: 2026-09-25
Review target: `89e319b..e54a63994e70d2ad235b3cf7dad829f21859f996`

# E06 independent verifier review, round 15

The judged stories were both `ready_for_review` at entry. The review covers the two open stories as one epic; earlier E06 stories were already independently closed. No cutover or formula adoption occurred.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S04 | FAIL | C3 names a **2.0.0** engine formula on the reviewed commit. The successful `tests.yml` Homebrew job on `e54a639` sets `version="$(python3 cancellai.py version)"`, which is **1.21.1**, and passes it to `render-local-cutover`; its formula says `version "1.21.1"`. Thus its `brew test` did not install or test a 2.0.0 formula. C1, C4-C8 have the evidence below; C2 has the E06-S16 residual. Brief-Checksum: 41346037461d07abb0cb2d459581a90b1a2015696b662e239eacd4529e6f0dbb |
| E06-S16 | PASS_WITH_RESIDUALS | The real v1.21.1 release run `36069385676` at tag commit `db5870568eaf37dd0024eb3f636b9c45d948d580` completed all 11 jobs, including four builds, attestation verification, manifest generation and publication; the public remote tag API names the same commit as the local tag. The release evidence records the successful `verify-release` output. Independently downloaded all four archives: each SHA-256 equals its manifest entry; the manifest exactly equals the generator output for the local tag, run and downloaded digests; the tag archive hash and published `cancellai.rb` exactly match the rendered Python formula. Direct rerun of `verify-release` and `gh attestation verify` was unavailable because this worktree's `gh` has no authentication. This limits independent provenance verification; it revealed no contradictory asset. Brief-Checksum: 9bdf85e0a19b367c0724580601f62c343c5f77c2c60be30e739e501dcc18b482 |

## E06-S04 finding and required repair

**F-15-01 — C3's claimed version was not tested.** Reproduction: `python3 cancellai.py version` prints `1.21.1`; `.github/workflows/tests.yml`'s cutover job assigns that output to `version`, builds the Rust CLI from `rust/Cargo.toml` version `1.21.1`, and calls `release.py render-local-cutover --version "$version"`. `render_formula` puts `version "1.21.1"` in the local formula. A direct render with these inputs reproduced that line and the engine/legacy installation lines. GitHub run `36071425444`'s Homebrew job passed on reviewed commit `e54a639`, but it exercised 1.21.1. The earlier passing run `36070653690` did the same. No observed job stages and installs a 2.0.0 candidate.

**Required repair:** make the reviewed-commit CI cutover job build and install a version-aligned 2.0.0 source and Rust engine formula from the reviewed commit's candidate archives, require its exact installed versions and `brew test`, and record that run; if the owner intended only an engine-shaped formula at the current version, explicitly amend C3 instead of calling the 1.21.1 run a 2.0.0 test. Until then the E06-S04 verification contract's C3 and its cutover eligibility claim are unmet. This is a verification gap, not evidence of an unsafe deletion. E06-S04 returns to `in_progress`; no owner authorization is requested on a failing verdict.

## Other checklist and safety evidence

- **C1:** `project_os.py check` passed; E06-S03, E21, E22-S01, E06-S06..S15 are `done`. E06-S14/S15 have earlier independent review records and Safety Verdicts.
- **C2 / E06-S16 AC1-AC3:** `verify_release` calls `engine_sha256s`, `archive_sha256`, then byte-compares published `cancellai.rb` with `render_formula`; `engine_sha256s` enforces the closed manifest, local/remote tag, four downloaded archive hashes and attested run identity. The focused release tests passed, including a one-byte formula change and failed release-run refusal. The public GitHub API identifies release run `36069385676` as a successful tag push at `db58705`; the release evidence records the real command's successful output. The authenticated provenance step could not be repeated here. The command did not change the repository on the failed local rerun.
- **C4:** on `e54a639`, rust run `36071425394`'s `parity` jobs passed on macOS/Linux and `cli-stable-channel (windows-latest)` passed. The latter runs `windows_partial_scan.rs`, which tests four locked fixture types in default and custom root scenarios, requires incomplete scope, safety-block exit and intact sessions, and proves unlocked work can delete. Local parity self-test and the 14 normative fixtures in both root-origin scenarios passed on macOS.
- **C5:** `_ENGINE_BODY` installs `cancellai.py` as `cancellai-legacy`; `render_formula` retains the engine body through version 2.1.0. The v1.21.1 tag contains `cancellai.py`.
- **C6:** the Unreleased changelog names the Rust switch and command, flag, JSON, history, hard-link, channel and Windows differences. `docs/CLI_RUST.md` and `project/cli_inventory.json` agree in the focused inventory tests (78 tests with the release tests passed).
- **C7:** `docs/RELEASING.md` describes signed containment, `cancellai-legacy`, and formula revert. It labels rollback documented, not rehearsed.
- **C8 / AC1:** `cutover_authorization_problems('2.0.0')` returns `no owner authorization`; `finalize --adopt-cutover` refuses without a passing, hash-bound owner authorization. The owner acceptance is explicitly after an independent migration pass and is absent, as expected while this verdict fails.
- **SI-019:** `check_mutation_boundary.py check` passed on 121 Rust sources. This review found no new mutation caller or bypass in the S16 release-verification diff. The migration Safety Verdict remains FAIL because C3 is unproved.

## Gates actually run

| Gate | Result |
| --- | --- |
| `project_os.py check/status/next/review`, verifier briefs for both stories, `check_agent_toolchain.py report` | PASS; both open stories reviewable, no toolchain decision overdue. |
| `gh run list --branch main --limit 5` | UNAVAILABLE: local GitHub CLI has no authentication. Read-only public GitHub Actions API used for per-job conclusions. |
| `python3 scripts/release.py verify-release --version 1.21.1` | UNAVAILABLE at the remote-tag `gh api` step for the same authentication reason; no contrary asset found by independent download/hash/render checks. |
| `python3 -m pytest tests -q` | 811 passed, 6 skipped, 953 subtests passed, 1 failure: untracked harness file `.opencode-run/prompt.md` has a broken relative ADR link. Focused `tests/test_release.py` and `tests/test_cli_inventory.py`: 78 passed, 135 subtests passed. |
| Python `ruff` and `mypy` commands | UNAVAILABLE: neither module is installed in this worktree's Python; no tooling installed during review. |
| AGENTS.md Python script gates, run individually | PASS: gen_docs, project_os, workflows, fixtures, schemas, characterization, differential harness, Rust workspace, mutation boundary, provider compatibility/trust, platforms, Rust/Python parity self-test/check, process, release, release manifest, topology, agent skills, process metrics, risk classification, toolchain, skill content, verifier handoff, evidence, safety oracle and EARS. `check_docs.py check` FAILS only on `.opencode-run/prompt.md`; `gate_sensitivity.py check` consequently refuses the unmutated docs/pytest baseline. |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace` | PASS locally on macOS. |
| `cargo deny check` | PASS with a temporary writable `CARGO_HOME` pointing to the existing registry; initial default-home attempt could not lock its read-only advisory DB. |
| Public CI for reviewed commit `e54a639` | `tests` run `36071425444`, `rust` run `36071425394`, `governance` run `36071425416`, and `codeql` run `36071425343` all completed successfully. This does not change the C3 version mismatch inside the passing Homebrew job. |

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/MIGRATION_PYTHON_RUST.md`; `docs/RELEASING.md`; `docs/CLI_RUST.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/security/SUPPLY_CHAIN.md`; `docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`; `CHANGELOG.md`; `project/cli_inventory.json`; `project/epics/E06.json`; `project/templates/SAFETY_VERDICT.md`; `project/evidence/E06-VERIFIER-REVIEW-ROUND14.md`; `project/evidence/E06-S04/EVIDENCE.md`; `project/evidence/E06-S04/VERIFIER_BRIEF.md`; `project/evidence/E06-S04/SAFETY_VERDICT.md`; `project/evidence/E06-S16/VERIFIER_BRIEF.md`; `project/evidence/RELEASE-v1.21.1.md`.

## Overall verdict and disposition

**FAIL — 1 of 2 stories rejected (50% yield).** E06-S16 is `done` with the stated provenance rerun residual; E06-S04 is `in_progress`; E06 remains `in_progress`. ADR-0025 requires another round while yield is at least 10%, and this is beyond its ordinary three-round cost ceiling, so further cost/disposition is an owner decision. No cutover is authorized or adopted, and no formula was changed by this review.
