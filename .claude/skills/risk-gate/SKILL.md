---
name: risk-gate
description: Resolve and run the exact verification gates a cancellAI change requires, from its Change Risk Level (CR0-CR4) and the files it touches. Use before declaring work complete, before ready_for_review, when asked "which checks do I run", "is this releasable", or when a change touches rust/, cancellai.py, project/, or a safety boundary.
argument-hint: [CR0|CR1|CR2|CR3|CR4]
allowed-tools: Bash, Read, Glob, Grep
---

# Risk gate

The Change Risk Level, not the diff size, sets verification depth. A one-line change to a safety
boundary is CR4. This skill resolves which gates apply and runs them honestly.

## What the change actually touches

!`git status --short 2>&1 | head -40`

!`git diff --stat HEAD 2>&1 | tail -20`

## Step 1 - classify

Assign the level from `docs/development/WORK_ITEM_MODEL.md`. Take the **highest** that applies:

- **CR0** documentation, metadata, non-executable templates.
- **CR1** observational behavior that cannot influence mutation authority (status, TUI view, metrics).
- **CR2** classification / planning / state semantics (attribution, capability results, policy
  schema, plan serialization).
- **CR3** reversible or conditionally mutating behavior.
- **CR4** irreversible mutation, the safety kernel, or an authority/trust/supply-chain boundary.

Escalate to CR4 on sight if the change touches any of: `cancellai-safety`, `cancellai-sealedfs`,
`cancellai-platform`, `cancellai-model`, a protected-name list, a root-resolution path, provider
trust, release signing/provenance, or anything that decides what is *permitted*.

State the level and the one sentence that justifies it. If you are between two levels, take the
higher one - ambiguity never escalates privilege, and it never lowers verification either.

## Step 2 - run the gates

Always, for every level: `python3 scripts/project_os.py check`.

**Python reference stage touched** (`cancellai.py`, `scripts/`, `tests/`, `project/`, `docs/`):
run the full "Current Python checks" list in `AGENTS.md`, or `pre-commit run --all-files`, which
runs the same set. Do not run a subset and report it as the set.

**`rust/` touched** - from `rust/`:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
```

**CR2 and above, additionally:** deterministic outputs where contracted; schema/fixture
compatibility (`scripts/check_schemas.py`, `scripts/check_fixtures.py`).

**CR3 and above, additionally:** adversarial and failure cases, not only happy paths - invoke the
`adversarial-cases` skill and run what it produces; rollback/recovery behavior; mutation-boundary
check (`scripts/check_mutation_boundary.py`).

**CR4 additionally:** everything above, plus the four release gates in
`docs/development/RELEASE_GATES.md` (G1 Functional, G2 Safety, G3 Compatibility, G4 Operability),
plus an **independent** Safety Verdict from `project/templates/SAFETY_VERDICT.md`. The executor
does not write that verdict. A CR4 story cannot close without one recording a pass.

**Differential stage:** if Python and Rust both implement the behavior, run
`python3 scripts/diff_harness.py check` and `python3 scripts/rust_python_parity.py check`.
An unexplained divergence on a `NORMATIVE` fixture is a failure, not a note (migration gate M6).

## Step 3 - report, do not launder

```
Level:      CR<n> - <justification>
Ran:        <command> -> pass | fail | UNAVAILABLE (<why>)
Failed:     <command> -> <the actual output, not a summary>
Not run:    <command> -> <why, and who runs it instead>
Done:       yes | no      (does it work and is it integrated/documented?)
Safe:       yes | no      (can it violate an invariant, lose data, misrepresent uncertainty,
                           or create an unverified authority path?)
```

Done=YES with Safe=NO is **not** merge or release eligible. Say that plainly rather than
softening it. A gate you skipped is reported as skipped; a tool that is unavailable locally is
recorded as unavailable, and CI must still run it before merge.
