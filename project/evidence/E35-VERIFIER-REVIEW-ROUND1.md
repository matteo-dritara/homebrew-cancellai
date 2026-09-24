Review-Scope: epic
Round: 1
Verifier: Codex
Date: 2026-09-24
Review-Target: f81fe60e2cc37a6ddfcf8ee84aa854ccb193861a..c6881213b379626762305427574ecbaea9fd2e33

# E35 independent verifier review, round 1

E35 has one story. E35-S01 was `ready_for_review` when review began. I derived the expected result from its acceptance criteria and the append-only Safety Verdict contract, then tested the final diff. No CR4 story is in E35, so no story Safety Verdict is due.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E35-S01 | FAIL | A final `## Round 10` containing standalone `FAIL`, followed by `## Owner note` and then a newly appended `## Verdict` containing standalone `PASS`, makes `safety_verdict_passes` return `True`. The added adversarial test fails. Brief-Checksum: 49a80428b0062bf2e0499d346956c4d0b864942a58ab5f5001438af98d9e54bc |

## E35-S01 finding

Reproduction, using a temporary `SAFETY_VERDICT.md`:

```markdown
## Round 10

FAIL

## Owner note

No decision here.

## Verdict

PASS
```

`scripts.project_os.safety_verdict_passes(path)` returns `True`. `final_round_text` accepts **every** later `## Verdict` section, even one after an owner-note section. It also accepts a second `## Verdict` section after a first one. Therefore a later section can supply the decisive `PASS` despite the final round's `FAIL`. This violates AC1 (the final round's own verdict decides) and AC2 (a verdict-shaped line in a later section must make the file refuse). Because this parser gates CR4 closure, it also weakens C-16's evidence-gated delivery.

Required repair: treat a template `## Verdict` as attached to the final round only when it immediately follows that round, allow at most one such section, and refuse verdict-shaped lines in any subsequent section, including another `## Verdict`. A standalone final-round `FAIL` must not be overridable by a later passing section. Preserve the already committed E14-S04/E14-S05 template layouts and the no-round E32-S01 behavior. Add regression cases for an owner note followed by `## Verdict`, duplicate verdict sections, and a contradictory final-round body and template section.

The executor's new tests cover a direct owner-note `PASS` and one immediate template `## Verdict`; they never put a `## Verdict` after another post-round section. The added `test_an_owner_note_cannot_add_a_later_verdict_section` exercises that missing case: `python3 -m pytest tests/test_project_os.py -q` reports 1 failed, 30 passed, 15 subtests passed. Independent comparison against the parent commit's parser on all 53 tracked verdict-named Markdown files found 42 passing before and after, 11 refusing before and after, and no changed judgement. AC3's committed-file compatibility is supported; it does not cure AC1/AC2.

## Gates and limits

- At session start, `python3 scripts/project_os.py check`, `status`, `next`, `review`, `brief E35-S01 --role verifier`, and `python3 scripts/check_agent_toolchain.py report` passed. Toolchain report found no overdue decision.
- `gh run list --branch main --limit 5` exited 4 because GitHub CLI is unauthenticated. Main CI is unknown.
- `python3 -m pytest tests -q` initially failed: 802 passed, 6 skipped, 884 subtests passed, with two failures. One is the deliberate adversarial regression above; the other is the documentation link test reading the pre-existing untracked `.opencode-run/prompt.md`, whose relative ADR link does not resolve from that directory. With that untracked directory temporarily moved out and restored, the full suite failed only on the adversarial regression (803 passed, 6 skipped, 885 subtests passed); `python3 -m pytest tests/test_docs.py -q` passed (2 tests).
- `python3 -m ruff check .` and `python3 -m ruff format --check .` could not start on the default interpreter because those modules are absent. The pinned existing virtual environment ran `ruff check .` (pass), `ruff format --check .` (pass after formatting the new test), and the full AGENTS.md `mypy` source list (pass, 29 files).
- `python3 scripts/check_docs.py check` initially failed only on the same untracked `.opencode-run/prompt.md`. With that directory temporarily moved out and restored, it passed, including this new review record (593 Markdown files).
- These standalone checks passed: `python3 scripts/gen_docs.py --check`, `project_os.py check`, `check_workflows.py check`, `check_fixtures.py check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check`, `check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py check`, `check_provider_trust.py check`, `check_platforms.py check`, `rust_python_parity.py self-test`, `rust_python_parity.py check`, `check_process.py check`, `release.py check`, `release_manifest.py check`, `check_repository_topology.py check`, `check_agent_skills.py check`, `process_metrics.py check`, `check_risk_classification.py check`, `check_agent_toolchain.py check`, `check_skill_content.py check`, `verifier_handoff.py check`, `check_evidence.py check`, `safety_oracle.py check`, and `check_ears.py check` (each invoked with `python3 scripts/` and `check` unless a different subcommand is shown).
- `python3 scripts/gate_sensitivity.py check` failed because its unmutated-tree baseline ran the deliberately failing pytest case and the untracked-file docs failure. With that untracked directory temporarily moved out and restored, it still failed on the deliberate pytest regression. Its mutant-kill result is therefore unavailable for this round.
- No `rust/` files changed in the review target; the Rust workspace gate set does not apply to this CR2 process-parser change.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `project/epics/E35.json`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `project/templates/SAFETY_VERDICT.md`; `project/evidence/E35-S01/EVIDENCE.md`; `project/evidence/E14-S04/SAFETY_VERDICT.md`; `project/evidence/E14-S05/SAFETY_VERDICT.md`; `project/evidence/E34-VERIFIER-REVIEW-ROUND1.md`; `requirements-dev.txt`; `scripts/project_os.py`; `scripts/release.py`; `scripts/verifier_handoff.py`; `scripts/process_metrics.py`; `tests/test_project_os.py`.

## Overall verdict

FAIL: one of one judged stories was rejected, a 100% finding yield. ADR-0025 requires another independent round after repair. E35-S01 returns to `in_progress`; the epic remains `in_progress`. No production code was changed and no commit was made in this formal round.
