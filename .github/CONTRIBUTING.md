# Contributing

cancellAI uses an evidence-gated engineering system because some changes can mutate or delete user data.

## Read first

- [AGENTS.md](../AGENTS.md)
- [Documentation map](../docs/INDEX.md)
- [Engineering Operating System](../docs/development/ENGINEERING_SYSTEM.md)
- [Work Item Model](../docs/development/WORK_ITEM_MODEL.md)

Every implementation change should map to a story ID from the project control plane.

## Setup for the current Python reference stage

The shipping tool remains stdlib-only. Development tooling is pinned in
[`requirements-dev.txt`](../requirements-dev.txt).

```sh
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements-dev.txt
pre-commit install --install-hooks --hook-type pre-commit --hook-type commit-msg
```

**Install the hooks.** They are not decoration: they run the same checks CI runs, and the
`commit-msg` hook is what keeps commit messages machine-readable. If you skip this step CI
will simply fail later, having wasted a round trip - the `pre-commit` job runs the whole
hook set on every pull request regardless of what is installed on your machine.

## Before a PR

The full gate set for the current Python stage:

```sh
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

Or, equivalently and in one command:

```sh
pre-commit run --all-files
```

Then run any additional story-specific gates required by the story's Change Risk Level.

## What is enforced, and where

Nothing here relies on remembering to do it. Each rule has an owner in code:

| Rule | Enforced by |
| --- | --- |
| Tests, lint, format, types | `pre-commit`, `tests` workflow |
| Generated docs match their source data | `scripts/project_os.py check`, `scripts/gen_docs.py --check` |
| Documentation links resolve and nothing is orphaned | `scripts/check_docs.py check` |
| Safety Invariant IDs referenced by stories exist | `scripts/project_os.py check` |
| Story lifecycle, evidence at handoff, passing Safety Verdict before `done` | `scripts/project_os.py check` |
| ADR numbering, status, forward links; decision supersession | `scripts/check_process.py check` |
| Evidence names a real work item; Safety Verdicts carry their sections | `scripts/check_process.py check` |
| Conventional Commit messages | `commit-msg` hook, `commit-convention` CI job |
| Versions agree across source, packaging and formula | `scripts/release.py check` |
| A closed epic has a release | `scripts/release.py check` |
| A release is verified at its own tag | `release` workflow |
| Actions pinned to SHAs with least-privilege tokens | `scripts/check_workflows.py check` |

If you find a rule in prose that no check enforces, that is a defect: either automate it or
delete it.

## PR scope

Prefer one coherent story or one independently reviewable slice of a story per PR. Keep branches short-lived. Refactors that materially obscure a behavior change should normally be separate.

## CR3/CR4 changes

Reversible/irreversible mutation or authority changes require an independent verifier. CR4
additionally requires a Safety Verdict using
[`project/templates/SAFETY_VERDICT.md`](../project/templates/SAFETY_VERDICT.md) that records
`PASS` or `PASS_WITH_RESIDUALS`; `scripts/project_os.py` refuses to let a story close over a
committed `FAIL`.

Implementers stop at `ready_for_review` and never mark their own work verified or done. See
[the work item model](../docs/development/WORK_ITEM_MODEL.md) and
[the agent protocol](../docs/development/AGENT_PROTOCOL.md).

## Code of conduct

Participation is covered by the [Code of Conduct](CODE_OF_CONDUCT.md).

## Provider contributions

New providers use the capability/trust model in `docs/architecture/PROVIDER_MODEL.md`. A community contribution cannot self-assign Built-in Verified trust or irreversible capability. Synthetic fixtures and provenance are required for promoted support.

## Test data privacy

Never commit real agent transcripts, prompts, source code, auth/config secrets, or personal home-directory snapshots. Use synthetic/scrubbed fixtures.

## Documentation

Generated planning docs are updated by editing `project/*.json` and running `python3 scripts/project_os.py generate`. Do not edit generated views directly.

Public behavior changes update `CHANGELOG.md`.
