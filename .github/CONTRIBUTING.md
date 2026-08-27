# Contributing

cancellAI uses an evidence-gated engineering system because some changes can mutate or delete user data.

## Read first

- [AGENTS.md](../AGENTS.md)
- [Documentation map](../docs/INDEX.md)
- [Engineering Operating System](../docs/development/ENGINEERING_SYSTEM.md)
- [Work Item Model](../docs/development/WORK_ITEM_MODEL.md)

Every implementation change should map to a story ID from the project control plane.

## Setup for the current Python reference stage

The shipping tool remains stdlib-only. Development tooling:

```sh
python3 -m venv .venv
source .venv/bin/activate
pip install pytest ruff mypy
```

## Before a PR

At minimum for the current Python stage:

```sh
python3 scripts/project_os.py check
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py
python3 scripts/gen_docs.py --check
```

Then run any additional story-specific gates required by its Change Risk Level.

## PR scope

Prefer one coherent story or one independently reviewable slice of a story per PR. Keep branches short-lived. Refactors that materially obscure a behavior change should normally be separate.

## CR3/CR4 changes

Reversible/irreversible mutation or authority changes require an independent verifier. CR4 additionally requires a Safety Verdict using `project/templates/SAFETY_VERDICT.md`.

## Provider contributions

New providers use the capability/trust model in `docs/architecture/PROVIDER_MODEL.md`. A community contribution cannot self-assign Built-in Verified trust or irreversible capability. Synthetic fixtures and provenance are required for promoted support.

## Test data privacy

Never commit real agent transcripts, prompts, source code, auth/config secrets, or personal home-directory snapshots. Use synthetic/scrubbed fixtures.

## Documentation

Generated planning docs are updated by editing `project/*.json` and running `python3 scripts/project_os.py generate`. Do not edit generated views directly.

Public behavior changes update `CHANGELOG.md`.
