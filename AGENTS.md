# AGENTS.md

Canonical engineering instructions for humans and AI coding agents working in cancellAI.

## Before any code change

Read, in order:

1. `docs/INDEX.md`
2. `docs/CONSTITUTION.md`
3. the selected story in `docs/BACKLOG.md` / `project/epics/*.json`
4. the architecture/security documents referenced by that story
5. `docs/development/ENGINEERING_SYSTEM.md`
6. `docs/development/AGENT_PROTOCOL.md`

Then run:

```sh
python3 scripts/project_os.py check
python3 scripts/project_os.py status
python3 scripts/project_os.py next
python3 scripts/project_os.py review
```

Do not begin implementation from chat context alone. The repository contract is authoritative.

## Current transition state

The shipping implementation is the stdlib-only Python file `cancellai.py`. It is being converted into an executable reference before a spec-first migration to Rust.

Until epic E01 is complete:

- implement only P0 trust-floor fixes or work explicitly listed in P0;
- do not perform a broad Python refactor for architectural cleanliness;
- do not add future product features to the Python monolith;
- preserve current user-facing behavior except where the story explicitly corrects a defect.

After E01 freezes Python, new product capability belongs in the Rust target architecture.

The old statement that the single-file Python architecture is a permanent non-negotiable no longer applies. ADR-0007 supersedes it.

## Constitutional non-negotiables

Never violate `docs/CONSTITUTION.md` or `docs/security/SAFETY_INVARIANTS.md` merely because a story or implementation shortcut appears to ask for it. A conflict means the story/spec needs escalation and an ADR/RFC/owner decision.

Key rules:

- unknown/partial/protected state is non-destructive;
- ambiguity never escalates privilege;
- mutation goes through one safety boundary;
- quarantine is preferred to purge when safely possible;
- network knowledge cannot directly authorize local deletion;
- provider/manifests are capability- and trust-bounded;
- UI/Guardian code contains no independent mutation logic;
- cached/local DB state is never destructive truth;
- CR4 requires independent verification and an owner-visible Safety Verdict.

## Work one story at a time

Every change must identify a story ID. Generate a role-specific repository brief with `python3 scripts/project_os.py brief <STORY-ID> --role executor|verifier`. If the requested change has no story:

1. decide whether it is a defect inside an existing story, or
2. add/update the project control plane before implementation.

Do not silently create product scope in code.

## Change Risk Levels

Follow `docs/development/WORK_ITEM_MODEL.md` and `docs/development/RELEASE_GATES.md`.

- CR0 docs/metadata
- CR1 observational
- CR2 classification/planning/state semantics
- CR3 reversible/conditional mutation
- CR4 irreversible mutation or authority/trust boundary

The higher risk level determines verification depth even if the diff is tiny.

## Executor / verifier separation

The default workflow uses one agent as executor and another as verifier. See `docs/development/AGENT_PROTOCOL.md`.

Executor:

- plan verification before code;
- implement the smallest coherent change;
- add tests/docs in the same work item;
- create evidence summary;
- set the story to `ready_for_review` and stop there.

`ready_for_review` is the executor's exit state. Never mark your own work `verification` or `done`, and never write your own CR4 Safety Verdict. Claude is the standing executor; Codex performs the independent review.

Review runs at **epic** scope, once every story in the epic is `ready_for_review`, and **at most twice per epic**. Findings that survive the second round become new backlog work items, not a third round.

**Closing an epic cuts a release.** `scripts/release.py check` fails when a closed epic has no release evidence, and it runs in `pre-commit` and CI. See `docs/development/WORK_ITEM_MODEL.md` and ADR-0014.

Verifier:

- work from story/AC/invariants/threats and final diff;
- independently seek counterexamples;
- do not simply confirm executor tests;
- produce CR4 Safety Verdict when required.

Do not pass private executor reasoning to the verifier as evidence.

## Current Python checks

For changes that touch the Python reference stage, install the pinned development tools when needed:

```sh
python3 -m pip install -r requirements-dev.txt
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_schemas.py check
python3 scripts/characterize.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

Or `pre-commit run --all-files`, which runs the same set. Install the hooks once with
`pre-commit install --install-hooks --hook-type pre-commit --hook-type commit-msg`; the
`commit-msg` hook enforces Conventional Commits, and CI enforces it again for every pull
request.

If a dev tool is unavailable locally, say so in evidence; CI must still run it before merge.

## Future Rust checks

Once E02 creates the workspace, the required commands are defined in the story/CI and expected to include:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
cargo audit (or equivalent RustSec gate)
```

Do not add a dependency merely to reduce implementation effort. Provider/safety/runtime dependencies require the review appropriate to their risk and supply-chain impact.

## Generated project docs

Do not edit these by hand:

- `docs/DECISION_REGISTER.md`
- `docs/ROADMAP.md`
- `docs/BACKLOG.md`
- `project/generated/PROJECT_STATUS.md`

Edit `project/*.json` / `project/epics/*.json`, then run:

```sh
python3 scripts/project_os.py generate
```

CI checks drift.

`docs/CLI.md` remains generated from the current Python CLI until the Rust CLI generator replaces it through an explicit story/ADR.

## Documentation impact

A behavior, safety, platform, provider, policy, persistence, or release change must update its associated docs in the same work item. The story lists expected documentation impact; add more if implementation changes more contracts.

Architecturally significant decisions get an ADR. Proposals with competing material options get an RFC first.

## Tests and fixtures

- tests must never target real `~/.claude`/`~/.codex` data;
- use temporary/synthetic filesystem trees;
- never commit real transcripts, prompts, source code, auth material, secrets, or home paths;
- CR3/CR4 tests must include adversarial/failure cases, not only happy paths;
- during migration, unexplained Python/Rust differential behavior is a failure.

## Changelog and commits

Keep a Changelog and Semantic Versioning remain the public release conventions. User-visible behavior changes update `CHANGELOG.md` under Unreleased.

Use Conventional Commit prefixes (`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`, `style:`, `ci:`).

Prefer small, short-lived branches/PRs and squash merges. Separate large refactors from behavior changes when practical.

## Security reporting

A way to bypass a Safety Invariant, escape an approved root, delete protected/unknown/active state, forge trusted provider/knowledge authority, or compromise release verification is a security issue. Follow `.github/SECURITY.md`.
