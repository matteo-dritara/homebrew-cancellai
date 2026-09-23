# E16 Verifier Review - Round 2

Review-Scope: epic
Round: 2
Verifier: Codex
Brief-Checksum: 1c3fa48786d70231c07a865f709041d444e19248af8a1139ca7d462b3979b308

- Review target: `d70945f~2..d70945f` (commits `87dd155` and `d70945f`), with verifier repairs in this review commit.
- Date: 2026-09-23.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E16-S08 | REPAIRED | Independent probes found that an embedded NUL raised `ValueError` out of the checker and `.`/`./` passed as fixture evidence. An existing Git-ignored file also passed the existence-only rule. The verifier added fail-closed resolution error handling, repository-root rejection, and Git ignore detection. Added regression tests; focused suite: 28 passed. The real trust registry passes. |

## Adversarial evidence

- Missing file/directory references fail and include the reference; a missing reference beside a real path still fails.
- POSIX absolute, Windows drive, and UNC forms are refused. Both slash and backslash `..` components are refused even if they would normalize back inside the root.
- A symlink to an in-tree target is accepted; a symlink outside the root is refused. Broken and looping symlinks are refused without crashing.
- `.` and `./` are refused as repository-root references. Empty and whitespace references are refused. Non-string entries fail the list-of-strings validation before path resolution.
- An embedded NUL is now converted to a validation error instead of escaping as `ValueError`. Symlink resolution errors are refused.
- An existing Git-ignored file is refused. The checker uses filesystem metadata and `git check-ignore`; it never opens or executes the referenced fixture. No fixture tree contents or Git state were modified by the check; tests use temporary repositories.
- The committed registry passes. `git diff` shows the checker only reads/resolves paths and queries Git; it does not write files.

## Gates run

Passed:

- `python3 -m pytest tests -v` (679 passed; 626 subtests passed)
- `python3 scripts/gen_docs.py --check`
- `python3 scripts/project_os.py check`
- `python3 scripts/check_docs.py check`
- `python3 scripts/check_workflows.py check`
- `python3 scripts/check_fixtures.py check`
- `python3 scripts/check_schemas.py check`
- `python3 scripts/characterize.py check`
- `python3 scripts/diff_harness.py check`
- `python3 scripts/check_rust_workspace.py check`
- `python3 scripts/check_mutation_boundary.py check`
- `python3 scripts/check_provider_compatibility.py check`
- `python3 scripts/check_provider_trust.py check`
- `python3 scripts/check_platforms.py check`
- `python3 scripts/rust_python_parity.py self-test`
- `python3 scripts/rust_python_parity.py check`
- `python3 scripts/check_process.py check`
- `python3 scripts/release.py check`
- `python3 scripts/release_manifest.py check`
- `python3 scripts/check_repository_topology.py check`
- `python3 scripts/check_agent_skills.py check`
- `python3 scripts/process_metrics.py check`
- `python3 scripts/check_risk_classification.py check`
- `python3 scripts/check_agent_toolchain.py check`
- `python3 scripts/check_skill_content.py check`
- `python3 scripts/verifier_handoff.py check`
- `python3 scripts/check_evidence.py check`
- `python3 scripts/safety_oracle.py check`
- `python3 scripts/check_ears.py check`
- `python3 scripts/gate_sensitivity.py check`

Unavailable:

- `python3 -m ruff check .` and `python3 -m ruff format --check .`: Ruff is not installed (`No module named ruff`).
- The prescribed `python3 -m mypy ...` command: mypy is not installed (`No module named mypy`).
- `gh run list --branch main --limit 5`: GitHub API connection failed; CI status is unknown.

The gate outputs included pre-existing baseline warnings for old review-round counts, evidence packets, and EARS classification; those checks exited successfully. Pre-commit hooks are installed in the worktree's shared Git hooks path.

## Documents opened

- `AGENTS.md` (provided session contract)
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E16.json`
- `project/evidence/E16-S08/VERIFIER_BRIEF.md`
- `project/templates/VERIFIER_PROMPT.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/architecture/PROVIDER_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `scripts/check_provider_trust.py`
- `tests/test_provider_trust.py`
- `project/evidence/E16-VERIFIER-REVIEW.md`
- `project/evidence/E16-S07-VERIFIER-REVIEW.md`

## Overall verdict

REPAIRED. E16-S08's original implementation failed closed on ordinary missing, absolute, parent-traversing, and out-of-root symlink references, but it crashed on NUL input and accepted the repository root and ignored local-only paths as evidence. The verifier repaired these defects on this branch and verified the regressions. The epic status remains `in_progress` for the owner's release step.
