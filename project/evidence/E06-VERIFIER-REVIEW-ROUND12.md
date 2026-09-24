Review-Scope: epic
Round: 12
Verifier: Codex
Date: 2026-09-24
Review target: `cdea9e34d4683e71c4a325ba65b7fdb737412c67..02257d24fff44aa728c80e1a9bba1a9365473342`

# E06 formal verifier review, round 12

E06-S14 is the only story in this round's committed verifier handoff and the only E06 story at `ready_for_review`. E06-S04 remains blocked on it. No production code was changed, no commit was made, and no release was adopted.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S14 | FAIL | The exact-manifest repair rejects the round-11 duplicate-target counterexample; `tests/test_round12_adversarial.py` drives a simulated `finalize` and confirms 15 fault cases refuse with the live formula's bytes unchanged. But the story's committed `project/evidence/E06-S14/DESIGN_CONSULTATION_2.md` fails `ruff format --check .` at lines 187, 202, 212 and 228. The main `tests / lint` workflow for `06d775e` failed at that same gate (run 36046575202), and `.github/workflows/release.yml` requires it before `publish`. `docs/RELEASING.md` also still tells operators that cutover `finalize` uses `.sha256` sidecars, whereas this story removed that trust path. Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda. |

## Failure, reproduction, and required repair

1. **The release gate is red on the story's committed tree.** `ruff format --check .` reports one unformatted file: `project/evidence/E06-S14/DESIGN_CONSULTATION_2.md`, in its Python code fence at lines 187, 202, 212 and 228. The same committed defect failed the latest main `tests / lint` job (run 36046575202, step `Run python3 -m ruff format --check .`; the Python 3.10 and 3.14 test jobs, governance, codeql and Rust jobs passed). `release.yml`'s `verify` job runs this gate, and `publish` depends on `verify`; thus a tag cut from this tree cannot publish the formula asset required by E06-S14 AC1. **Repair:** format that committed code fence to the pinned Ruff version's output, rerun `ruff format --check .`, and obtain green tagged-release gate evidence before release eligibility is claimed. This also violates C-16/SI-019's evidence-gated CR4 delivery obligation.
2. **The release runbook describes the old authority source.** `docs/RELEASING.md`, “The cutover release,” says `finalize` uses each engine archive's `.sha256` sidecar. `scripts/release.py::engine_sha256s` now downloads and hashes each archive and refuses a digest mismatch with the closed manifest; it never reads a sidecar. **Repair:** update that runbook to describe the published formula/manifest/archive/provenance checks. This is the story's associated release documentation under AGENTS.md's documentation-impact rule and prevents an operator from treating a sidecar as adoption evidence. It bears on E06-S14 AC2–AC3 and SI-019's evidence requirement.

## Independent falsification evidence

- The release workflow's `publish` job verifies the manifest against `dist`, computes the tag archive digest, renders `cancellai.rb` and includes it in `gh release create`, in that order. `closed_manifest` now compares all manifest bytes with the generator's output; this rejects an extra differently named artifact with a duplicate target, a missing target, a wrong version, altered fields, and alternate serialization before adoption.
- `tests/test_round12_adversarial.py` creates synthetic archive files, builds a manifest from their real bytes through `release_manifest.build_manifest_from_files`, and drives `release.finalize` against a temporary live formula. Its positive control adopts the matching formula. Missing manifest, formula, or any of four archives; one changed formula or archive byte; wrong manifest version, missing or duplicate target, changed manifest digest; and provenance failure each raise `ReleaseError` with the original formula bytes intact (1 test, 15 subtests passed). The verifier's test stubs the network and attestation result; it does not claim to validate GitHub's cryptography.
- `tests/test_release.py`, `tests/test_round11_adversarial.py`, and `tests/test_release_manifest.py` passed (117 tests, 133 subtests). The GitHub REST response for a real successful tag run, 35875377083, has `path=.github/workflows/release.yml`, `head_branch=v1.21.0`, `event=push`, and `conclusion=success`; `check_release_run` accepted that actual response shape. The `gh attestation verify` local help confirms the JSON verification result's certificate is the authenticated source of `runInvocationURI`; an end-to-end cutover release has not yet occurred.
- The release boundary here is the Homebrew formula and published assets. No Rust deletion path or provider data was mutated by the review. `check_mutation_boundary.py check` passed.

## Gates actually run

| Command or group | Result |
| --- | --- |
| `project_os.py check/status/next/review`, `brief E06-S14 --role verifier`, `check_agent_toolchain.py report` | PASS at entry; review queue contained E06-S14 only; no toolchain decision overdue. |
| `gh run list --branch main --limit 5` | Unavailable because the harness has no `gh` authentication. The public GitHub REST workflow-run and job endpoints supplied the specific main run conclusions above. |
| `python3 -m pytest tests/test_release.py tests/test_round11_adversarial.py tests/test_release_manifest.py -q`; `python3 -m pytest tests/test_round12_adversarial.py -q` | PASS: 117 tests/133 subtests; 1 test/15 subtests. |
| `python3 -m pytest tests -q` | 808 passed, 6 skipped, 1 failed: `tests/test_docs.py` sees the harness's untracked `.opencode-run/prompt.md` with a broken relative link. This is outside the committed story diff and was not changed. |
| `python3 -m ruff check .`, `python3 -m ruff format --check .`, `python3 -m mypy ...` | The system Python lacks Ruff/Mypy modules. Their installed venv executables were run instead: `ruff check .` PASS; `ruff format --check .` FAIL on the committed design-consultation code fence; `mypy scripts/release.py scripts/release_manifest.py` and the AGENTS.md-listed set PASS. `ruff format --check tests/test_round12_adversarial.py` PASS. |
| AGENTS.md Python script checks, including `project_os`, workflow, manifest, process, release, parity, provider, platform, skills, toolchain, evidence, EARS, safety-oracle and generated-doc gates | PASS except `check_docs.py check` and `gate_sensitivity.py check`, both caused by the same untracked harness prompt's broken link. |
| `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo deny check` | PASS on the macOS host. No Rust source was changed by this story or review. |
| `git diff --check` | PASS. |

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`;
`docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`;
`docs/development/AGENT_TOOLCHAIN.md`; `docs/development/WORK_ITEM_MODEL.md`;
`docs/development/RELEASE_GATES.md`; `docs/development/VERIFICATION_STRATEGY.md`;
`docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`;
`docs/security/SUPPLY_CHAIN.md`; `docs/RELEASING.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
`docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`;
`project/epics/E06.json`; `project/templates/SAFETY_VERDICT.md`;
`project/evidence/E06-VERIFIER-REVIEW-ROUND11.md`;
`project/evidence/E06-S14/SAFETY_VERDICT.md`;
`project/evidence/E06-S14/EVIDENCE.md`;
`project/evidence/E06-S14/DESIGN_CONSULTATION_2.md`.

## Overall verdict

**FAIL**: one of one judged stories failed (100% finding yield). E06-S14 returns to `in_progress`; E06-S04 remains blocked. The current Python formula remains live. The owner's round-12 exception allowed this review beyond ADR-0025's cost ceiling; another round or an accepted residual after repair requires an owner decision, and this verifier does not close the story on a red release gate. No E06 release is cut.
