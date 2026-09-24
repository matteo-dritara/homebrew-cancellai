Review-Scope: epic
Round: 10
Verifier: Codex
Date: 2026-09-24
Review target: `8ea967e..0afe3ad`

# E06 formal verifier review, round 10

The committed review queue contained E06-S14, E06-S15 and E33-S03 at
`ready_for_review`. E06-S04 and E33-S01 remained blocked on these repairs, so
this round does not claim to close either dependent story. Main CI could not be
read because `gh run list --branch main --limit 5` requires authentication here.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S14 | FAIL | The independent `tests/test_round10_adversarial.py` supplies a published manifest claiming `source_sha: 000…000` while matching archive bytes, formula bytes and a same-repository, same-workflow, same-tag-ref attestation are supplied. `finalize` adopts the formula instead of refusing; the test fails at `assert refused`. `provenance_command` supplies `--source-ref` but not `--source-digest`, although `gh attestation verify --help` supports it. `manifest_digests` ignores `source_sha` and `build_identity.run_id`. Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda. |
| E06-S15 | PASS | The real authorization parser requires one each of `Authorized-by`, version and verdict SHA-256, binds the identity claim to `.github/CODEOWNERS`, and checks the final round section. `tests/test_release.py` (68 passed) exercises missing, changed, wrong-version and failing-final-round cases; independent inspection found no story-status path into `cutover_authorization_problems`. `finalize` checks authorization before writing the formula. Same-user identity spoofing is the explicit owner-accepted ADR-0040 limit. Brief-Checksum: 13575c2d3f9fdea6c85190d0e7daae57f1299d780b4857a7236d69da5e915ab3. |
| E33-S03 | PASS | `transact` holds `BEGIN IMMEDIATE` across row read, caller replay/decision and at most one insert; an existing database without `events` is refused. `cargo test --workspace` passed, and the stable-channel containment integration suite passed all 16 cases, including concurrent install/refresh, kill-before-commit rollback and corrupt-history authority cap. Store tests cover foreign schema, unknown event kind, oversized and zero-byte files. Same-user ledger edits remain the owner-accepted ADR-0039 limit. Brief-Checksum: 06f4b8e341065cb1b07f471d44b7fe59f0964ded2c3c6aa937c285f4c6365e92. |

## Required repair for E06-S14

The release manifest's `source_sha` can disagree with the source commit in an
archive's verified attestation while every comparison made by finalize passes.
The manifest is a published release identity document, and the formula is adopted
on its authority. This violates E06-S14 AC3's build-provenance requirement and
the supply-chain part of SI-019. Bind each archive attestation's authenticated
source digest to the manifest `source_sha`, and bind that SHA to the source tag
being adopted; reject a mismatch before touching the live formula. Exercise a
manifest/source mismatch and a moved-tag attestation with a simulated release,
asserting that finalize leaves the formula byte-identical.

## Gate status

- `python3 scripts/project_os.py check`, `status`, `next`, `review`, and the three
  `brief <ID> --role verifier` commands: pass at session start.
- `python3 scripts/check_agent_toolchain.py report` and `check`: pass; no overdue
  toolchain decision.
- `gh run list --branch main --limit 5`: unknown; `gh` is unauthenticated.
- `python3 -m pytest tests/test_release.py -q`: 68 passed.
- `python3 -m pytest tests -q`: 797 passed, 6 skipped, 1 failed because the
  pre-existing untracked `.opencode-run/prompt.md` contains a broken relative link.
- `python3 -m pytest tests/test_round10_adversarial.py -q`: 1 failed as the
  intended source-provenance counterexample.
- `python3 -m ruff check .`, `python3 -m ruff format --check .`, and
  `python3 -m mypy ...`: unavailable in the system Python. The pre-commit
  `ruff-check` and `ruff-format` hooks passed for the new test; the pre-commit
  `mypy` hook passed for `scripts/release.py`.
- `python3 scripts/check_docs.py check`: failed on that same untracked
  `.opencode-run/prompt.md`; `scripts/gate_sensitivity.py check` could not draw a
  result because docs and pytest already fail on the unmutated review tree.
- Python project gates run and passed: `gen_docs.py --check`,
  `check_workflows.py check`, `check_fixtures.py check`, `check_schemas.py check`,
  `characterize.py check`, `diff_harness.py check`, `check_rust_workspace.py
  check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py
  check`, `check_provider_trust.py check`, `check_platforms.py check`,
  `rust_python_parity.py self-test` and `check`, `check_process.py check`,
  `release.py check`, `release_manifest.py check`,
  `check_repository_topology.py check`, `check_agent_skills.py check`,
  `process_metrics.py check`, `check_risk_classification.py check`,
  `check_skill_content.py check`, `verifier_handoff.py check`,
  `check_evidence.py check`, `safety_oracle.py check`, `check_ears.py check`.
- Rust gates run and passed: `cargo fmt --check`, workspace Clippy with all
  targets/features and `-D warnings`, workspace `cargo check`, workspace
  `cargo test`, and `cargo deny check` (warnings only). The stable-channel CLI
  containment integration command with `kill-points,test-curl` passed 16 tests.
- `git diff --check`: pass before the record/status update.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`;
`docs/BACKLOG.md` (story contracts via the generated briefs and project control
plane); `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`;
`docs/development/WORK_ITEM_MODEL.md`;
`docs/development/RELEASE_GATES.md`;
`docs/development/VERIFICATION_STRATEGY.md`;
`docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`;
`docs/architecture/PERSISTENCE_MODEL.md`;
`docs/security/SAFETY_INVARIANTS.md`;
`docs/security/THREAT_MODEL.md`; `docs/security/SUPPLY_CHAIN.md`;
`docs/security/INCIDENT_RESPONSE.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
`docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md`;
`docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`;
`project/epics/E06.json`; `project/epics/E33.json`;
`project/templates/SAFETY_VERDICT.md`;
`project/evidence/E06-S04/SAFETY_VERDICT.md`;
`project/evidence/E06-S14/ROUND10_FINDINGS.md`.

## Overall verdict

FAIL: one of three judged stories failed (33% finding yield), so ADR-0025
requires another owner decision about review continuation above the normal cost
ceiling. E06-S14 returns to `in_progress`; E06-S04 remains blocked. E06-S15
and E33-S03 pass this round. E33-S01's obsolete blocker was cleared to
`ready_for_review` as an administrative consequence of E33-S03 passing; it has
not been judged in this round. Neither E06 nor E33 closes or cuts a release.
