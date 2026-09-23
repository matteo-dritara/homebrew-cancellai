# E16 Verifier Review - Round 3

Review-Scope: epic
Round: 3
Verifier: Codex
Brief-Checksum: 1c3fa48786d70231c07a865f709041d444e19248af8a1139ca7d462b3979b308

- Review target: `87dd155..4c8c9eb` (fixture-reference rule and tests at HEAD `bfffef5`).
- Date: 2026-09-23.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E16-S08 | FAIL | `fixture_reference_error()` accepts fixture paths when Git is absent, the root is not a Git repository, or `git check-ignore` times out/raises `OSError`; a nonzero Git error status (128/2) is also treated as “not ignored.” Direct probes returned `None` for all these cases. It also accepts untracked files in a repository, although the check's error text and story outcome require evidence committed in the repository. Required repair: make repository/evidence validation fail closed on Git absence, invocation errors/timeouts and unexpected exit codes, and require a fixture to be tracked/committed (while continuing to allow tracked paths even if an ignore pattern matches). Add tests for each failure mode and untracked versus tracked-but-ignored paths. This violates AC1/AC3's repository fixture evidence requirement and the outcome's claim that a reference is committed evidence. |

## Adversarial evidence

- Missing, absolute POSIX/Windows-drive, parent-component, empty, repository-root, embedded-NUL, and out-of-root symlink cases are refused by existing tests; embedded-NUL is now returned as a validation error rather than escaping from the helper.
- Direct probes on an existing path: Git absent => accepted (`None`); mocked `git check-ignore` raising `TimeoutExpired` => accepted; mocked `OSError` => accepted; return codes 128 and 2 => accepted; return code 0 is refused as ignored and return code 1 is accepted. A plain temporary directory without a Git repository accepted a fixture path.
- A newly initialized repository accepts an untracked fixture. A staged path matching `.gitignore` is accepted, which is correct for a tracked fixture; tests do not distinguish this from untracked content. Leading dash and newline in a fixture name are passed safely because the command uses `--` and an argument list.
- A chained symlink resolving outside the repository is refused. Windows-specific path behavior and case-sensitive filesystem behavior were not executable on this macOS host; the unit tests cover Windows drive and backslash parent forms, but not UNC/device paths or a Windows runner.
- No malformed fixture string tested escaped `main()`; embedded NUL and resolution errors are converted to validation errors. Git invocation exceptions are swallowed, but the resulting fail-open acceptance is the defect above. An injected unrelated `RuntimeError` from `validate()` does escape `main()`, but it is not reachable through the tested fixture-reference inputs and is not the basis of this verdict.
- The rule only resolves/stats paths and queries Git; it does not open or execute fixture contents. The focused tests pass, but they omit Git-unavailable, Git-error/timeout, non-repository, and untracked-path cases.

## Gates run

- `python3 -m pytest tests -q` — PASS (681 passed, 629 subtests passed).
- `python3 -m pytest tests/test_provider_trust.py -q` — PASS (28 passed).
- `python3 -m ruff check .` — UNAVAILABLE (`No module named ruff`).
- `python3 -m ruff format --check .` — UNAVAILABLE (`No module named ruff`).
- Prescribed `python3 -m mypy ...` command — UNAVAILABLE (`No module named mypy`).
- `python3 scripts/gen_docs.py --check` — PASS.
- `python3 scripts/project_os.py check` — PASS before review edits.
- `python3 scripts/check_docs.py check` — PASS.
- `python3 scripts/check_workflows.py check` — PASS.
- `python3 scripts/check_fixtures.py check` — PASS.
- `python3 scripts/check_schemas.py check` — PASS.
- `python3 scripts/characterize.py check` — PASS.
- `python3 scripts/diff_harness.py check` — PASS.
- `python3 scripts/check_rust_workspace.py check` — PASS.
- `python3 scripts/check_mutation_boundary.py check` — PASS.
- `python3 scripts/check_provider_compatibility.py check` — PASS.
- `python3 scripts/check_provider_trust.py check` — PASS (the real committed registry passes).
- `python3 scripts/check_platforms.py check` — PASS.
- `python3 scripts/rust_python_parity.py self-test` — PASS.
- `python3 scripts/rust_python_parity.py check` — PASS.
- `python3 scripts/check_process.py check` — PASS before review edits.
- `python3 scripts/check_process.py check` after recording round 3 — FAIL: counts four E16 review filenames (`E16-S07-VERIFIER-REVIEW.md` plus the three E16 epic-round files) against the three-round ceiling. `process_metrics.py` correctly treats the E16-S07 record as story-scoped and reports E16 rounds 1–3. This is a pre-existing counting inconsistency exposed by this record; no unrelated gate code was changed.
- `python3 scripts/release.py check` — PASS before review edits.
- `python3 scripts/release_manifest.py check` — PASS.
- `python3 scripts/check_repository_topology.py check` — PASS.
- `python3 scripts/check_agent_skills.py check` — PASS.
- `python3 scripts/process_metrics.py check` — PASS before review edits.
- `python3 scripts/process_metrics.py check` after regeneration — PASS; generated output matches the review history and classifies three E16 epic rounds.
- `python3 scripts/check_risk_classification.py check` — PASS.
- `python3 scripts/check_agent_toolchain.py check` — PASS.
- `python3 scripts/check_skill_content.py check` — PASS.
- `python3 scripts/verifier_handoff.py check` — PASS before review edits.
- `python3 scripts/check_evidence.py check` — PASS before review edits.
- `python3 scripts/safety_oracle.py check` — PASS.
- `python3 scripts/check_ears.py check` — PASS.
- `python3 scripts/gate_sensitivity.py check` — PASS.
- Session-start `python3 scripts/project_os.py status`, `next`, `review`, and `python3 scripts/check_agent_toolchain.py report` ran; `next`/`review` identified E16-S08 for review and the toolchain report had no overdue decisions.
- `gh run list --branch main --limit 5` — UNAVAILABLE; GitHub API connection error, so CI is unknown.
- Commit signing is unavailable in this sandbox's configured GPG environment; the review commit will be created with repository-local signing disabled, without changing configuration.

## Documents opened

- `AGENTS.md` (session contract)
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E16.json`
- `project/evidence/E16-S08/VERIFIER_BRIEF.md`
- `project/templates/VERIFIER_PROMPT.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/architecture/PROVIDER_MODEL.md`
- `docs/PROVIDERS.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `scripts/check_provider_trust.py`
- `tests/test_provider_trust.py`
- `project/evidence/E16-VERIFIER-REVIEW-ROUND2.md`

## Overall verdict

FAIL. E16-S08 remains open and is returned to `in_progress`. The third and final review round found that the Git evidence check fails open when Git cannot establish repository status, and accepts untracked files despite describing evidence as committed. Required repair is specified in the story row. Because the three-round cost ceiling has been reached, the surviving defect must be carried forward as owner-visible backlog work; this round does not close the story or alter the epic status.
