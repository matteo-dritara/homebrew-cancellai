# Orchestration Baseline Evidence - 2026-08-27

## Scope

Bootstrap the cancellAI product/engineering foundation from the attached v1 repository without implementing the future runtime feature roadmap. The shipping `cancellai.py`, its existing runtime tests, and the Homebrew formula behavior are intentionally unchanged by this orchestration package.

Baseline source identity recorded by the supplied repository:

```text
origin: https://github.com/matteo-dritara/homebrew-cancellai.git
commit: 4b2df0130e62d83e3a10caaae73daa456211f92d
release line: v1.0.2
```

## Delivered control plane

- 18 accepted product decisions.
- 7 roadmap phases, P0 through P6.
- 20 epics.
- 88 story contracts with CR level, dependencies, Acceptance Criteria, Safety Invariants, verification, and documentation impact.
- 31 stable Safety Invariants.
- Product Constitution, threat model, target architecture, provider/platform/policy/persistence/Guardian models.
- cEOS engineering operating system and Claude/Codex executor-verifier protocol.
- ADR/RFC/evidence/Safety Verdict templates.
- Generated Decision Register, Roadmap, Backlog, and Project Status.
- Governance, documentation, and workflow-policy validators.
- CodeQL workflow and immutable GitHub Action pinning policy.
- repository-governance and incident-response runbooks.

## Runtime code boundary

No P0 defect was silently fixed during orchestration. All verified defects remain visible in `docs/audits/2026-08-27-CODE_REVIEW.md` and are implementation work in E00. This keeps the requested executor/verifier development boundary intact.

## Verification executed in the orchestration environment

PASS:

```text
python3 -m py_compile / compile checks
python3 -m unittest discover -s tests -v     31 tests passed
python3 -m pytest tests -q                   31 tests passed
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
Ruby YAML parsing for active workflows/config
ruby -c Formula/cancellai.rb
git diff --check
```

The governance validator additionally proves:

- unique/formatted decision, epic, and story IDs;
- known dependency references;
- acyclic dependencies;
- no dependency on a later roadmap phase;
- ready/in-progress/verification/done work cannot have unfinished dependencies;
- CR4 stories reference known Safety Invariants;
- documentation-impact paths exist;
- completed work must have committed evidence, with a Safety Verdict for CR4;
- generated planning views have no drift.

## Environment limitations

`ruff` and `mypy` were not installed in the supplied execution environment. A package-install attempt could not reach the package index because DNS/network access from the container was unavailable. Their checks remain configured in GitHub CI and pre-commit. No claim is made that those two external-tool gates were executed locally.

Homebrew is not available in this Linux execution container, so `brew audit`, `brew style`, and install smoke tests were not executed locally. The formula parsed successfully with Ruby and the existing macOS CI job retains the Homebrew gates.

A final live `git fetch` of the public remote was also prevented by the container's DNS restriction. The supplied repository's recorded `origin/main` was at the baseline commit above; public-market/standards research used web-access sources separately.

## Release recommendation

This package is **orchestration-ready, not runtime-release-ready**. The next implementation action is E00-S01 (or another explicitly owner-selected ready E00 story) under the cEOS executor/verifier protocol. P0 Trust Floor must close before E01 freezes Python and before Rust feature development begins.
