Review-Scope: epic
Round: 13
Verifier: Codex
Date: 2026-09-24
Review target: `b36ce198943233b5f08760b581f7441c2db61bad..2ac50ebaeafae6a8e576f1ee43aaa99ada0c1c4c`

# E06 formal verifier review, round 13

E06-S14 was the only story at `ready_for_review` on entry; E06-S04 was blocked on it. This round judges the round-12 repair and independently rechecks the release boundary at HEAD. No release was adopted.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S14 | PASS_WITH_RESIDUALS | The publish job verifies manifest checksums against `dist`, then renders `cancellai.rb` from that manifest and the tag archive digest and includes it in `gh release create`. An independent CLI simulation generated a manifest from four synthetic archives, verified it, rendered the formula, then changed an archive byte: verification refused before publication and formula bytes stayed unchanged. `tests/test_round12_adversarial.py` drove a simulated `finalize`: valid assets were adopted; 15 missing, altered, duplicate/missing-target, wrong-version and provenance-failure cases refused with the live formula byte-identical. The closed manifest comparison, all-four-archive download/hash/provenance path, tag/run binding and published-formula byte comparison were inspected at HEAD. `ruff format --check .` now passes, `docs/RELEASING.md` describes the current authority path, and all jobs on origin/main commit `051ae5e` concluded success. Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda. |

## Independent falsification and safety evidence

- **AC1:** `.github/workflows/release.yml` makes `publish` depend on verification and attestation jobs, checks `release-manifest.json` against downloaded build artifacts again, computes the tag archive digest, calls `render-release-formula`, and publishes `cancellai.rb`. The verifier's separate temporary-directory CLI reproduction exercised generate, verify-checksums and render-release-formula with four actual archive files. Altering one archive made verify-checksums fail before the formula could be published.
- **AC2:** `adoptable_formula` derives the expected text from the downloaded and independently verified release assets, then requires exact UTF-8 bytes from the published `cancellai.rb`. The one-byte formula mutation in the simulated release refused adoption.
- **AC3:** `closed_manifest` refuses any manifest other than the generator's exact serialized profile, including extra or missing targets and another version. `engine_sha256s` downloads and hashes all four archives, verifies their attestations, requires a single successful tag-triggered release run, and compares the manifest with values derived from those bytes and the tag. The simulated `finalize` cases cover each missing asset, every changed archive, provenance failure, altered manifest and duplicate/missing target; every refusal precedes a write. An additional 1,000 malformed-manifest input probe produced 983 expected refusals and 17 self-consistent run-id values that the later attested-run comparison would reject if they disagreed; it found no unexpected exception or malformed acceptance at the shape boundary.
- **SI-019 / C-16:** This story changes the Homebrew release authority boundary, not the Rust filesystem mutation executor. `check_mutation_boundary.py check` passed. The latest CR4 Safety Verdict is appended separately. E06-S04 requires its own independent review and owner authorization.

## Gates actually run

| Command or evidence | Result |
| --- | --- |
| `project_os.py check/status/next/review`, `brief E06-S14 --role verifier`, `check_agent_toolchain.py report` | PASS at entry; one reviewable story; no toolchain decision past review date. |
| `python3 -m pytest tests/test_release.py tests/test_round11_adversarial.py tests/test_round12_adversarial.py tests/test_release_manifest.py -q` | PASS: 118 tests and 148 subtests. |
| `python3 -m pytest tests -q` | 809 passed, 6 skipped, 946 subtests passed, 1 failed. The failure is the docs test reading the harness's untracked `.opencode-run/prompt.md` with an invalid relative link; it is outside the committed review target. |
| `ruff check .`; `ruff format --check .`; `mypy` on the 29 AGENTS.md-listed Python sources | PASS. System `python3 -m ruff/mypy` is unavailable; the pinned project venv executables on PATH were used. |
| AGENTS.md Python script checks: generated docs, governance, workflows, fixtures, schemas, characterization, differential and parity checks, Rust workspace topology, mutation boundary, provider/platform checks, process/release/manifest, repository topology, agent skills/toolchain, metrics, risk, evidence, safety oracle and EARS | PASS except `check_docs.py check` and `gate_sensitivity.py check`: both fail because the same untracked harness prompt is scanned by the docs gate. `check_process.py check` passed with historical cost-ceiling warnings. |
| `gh run list --branch main --limit 5` | Unavailable: `gh` has no authentication. Public GitHub Actions REST responses for origin/main commit `051ae5e` showed every job in governance, tests, Rust and CodeQL successful, including lint, Python 3.10/3.14, Homebrew, three-platform Rust quality and MSRV checks. Current HEAD adds the local round-ceiling allowance after that commit. |
| `git diff --check` | PASS. |

The story changes Python release code, workflow and documentation; it does not touch `rust/`, so the local Rust workspace commands are not required for this round. The latest main Rust matrix passed as reported above.

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`;
`docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`;
`docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`;
`docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`;
`docs/security/SUPPLY_CHAIN.md`; `docs/RELEASING.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
`docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`;
`project/epics/E06.json`; `project/templates/SAFETY_VERDICT.md`;
`project/evidence/E06-VERIFIER-REVIEW-ROUND12.md`;
`project/evidence/E06-S14/SAFETY_VERDICT.md`;
`project/evidence/E06-S14/EVIDENCE.md`.

## Residuals and overall verdict

**PASS_WITH_RESIDUALS** for E06-S14: the tag-only release workflow and a real cutover release have not been exercised end to end in this review; the synthetic release and green main checks cover their components. A later change to a published asset or tag requires `verify-formula` again before installation. The local full-suite docs failure is caused by the untracked review prompt, not the committed change. No new defect was found in this round (0/1 finding yield). E06-S14 moves to `done`; E06-S04's obsolete `blocked_by` entry is removed and it returns to `ready_for_review` for the next round. E06 stays `in_progress`; no release is cut.
