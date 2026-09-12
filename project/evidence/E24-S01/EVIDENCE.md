# Evidence Packet - E24-S01

- Commit/PR: working tree at the E24 checkpoint commit
- Executor: Claude
- Independent verifier: Codex (pending; review runs at epic scope once E24-S02 is also ready)
- Change Risk: CR0
- Spec version/commit: project/epics/E24.json at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - pack is in the Agent Skills format | Seven directories under `.claude/skills/`, each with a `SKILL.md` carrying `name` and `description` frontmatter. `tests/test_agent_skills.py::RealPackTests` parses the committed pack through the same code path the checker uses. Format per <https://agentskills.io>; no Claude-Code-only field is required for a skill to load. | PASS |
| AC2 - checker rejects each drift | `tests/test_agent_skills.py` - `DriftTests` covers frontmatter, name, description and pack-level rejections; `CitationRegressionTests` covers sixteen citation shapes, seven of which must be rejected; `FrontmatterShapeTests` covers the YAML shapes a skill author actually writes. **Corrected after review:** the first version of this row claimed one case per rejection path. It was not true - a mutation replacing `SCRIPT_COMMAND` with a regex matching nothing left the whole suite green, because the assertion used the substring `"does not exist"`, which the *path* error also produces. The assertion is now `"names a script that does not exist"` and the same mutant fails the suite. See `project/evidence/E24-VERIFIER-REVIEW.md`. | PASS after repair |
| AC3 - runs in pre-commit and CI | `.pre-commit-config.yaml` hook `agent-skills-check`; `.github/workflows/tests.yml` step `python3 scripts/check_agent_skills.py check`. `scripts/check_workflows.py check` passes, which is what enforces that a `repo: local` pre-commit gate is also present in `release.yml`'s `verify` job - so the command was added there too, and to `AGENTS.md`'s "Current Python checks" block, which that checker parses as the single source `release.yml` is compared against. | PASS |
| AC4 - AGENTS.md points at the pack and states the rule | `AGENTS.md`, new "Agent skill pack" section: "A skill is a runner over this contract, never a second copy of it." Links `.claude/skills/README.md`. | PASS |
| AC5 - adds no product capability and never blocks a story | The change touches no runtime code: `cancellai.py`, `rust/crates/**` and every product path are unmodified (see the commit diff). `AGENTS.md` states the pack is developer convenience, not authority, and that a skill never decides what is permitted. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | n/a | CR0. The change adds no runtime code path and no authority decision; the story names no safety obligation. A skill is prose loaded into an agent's context - it can be wrong, but it cannot execute a mutation, and nothing in the mutation boundary consults it. | n/a |

## Verification Commands

```text
python3 -m pytest tests -q                      -> 301 passed, 34 subtests passed
python3 -m ruff check .                         -> All checks passed
python3 -m ruff format --check .                -> 287 files already formatted
python3 -m mypy                                 -> Success: no issues found in 13 source files
python3 scripts/check_agent_skills.py check     -> agent skills OK: 7 skills
python3 scripts/gen_docs.py --check             -> pass
python3 scripts/project_os.py check             -> governance OK: 23 decisions, 25 epics, 112 stories
python3 scripts/check_docs.py check             -> docs OK: 250 Markdown files
python3 scripts/check_workflows.py check        -> pass
python3 scripts/check_fixtures.py check         -> pass
python3 scripts/check_schemas.py check          -> pass
python3 scripts/characterize.py check           -> pass
python3 scripts/diff_harness.py check           -> pass
python3 scripts/check_rust_workspace.py check   -> pass
python3 scripts/check_mutation_boundary.py check-> pass
python3 scripts/check_provider_compatibility.py check -> pass
python3 scripts/check_provider_trust.py check   -> pass
python3 scripts/check_platforms.py check        -> pass
python3 scripts/check_process.py check          -> pass
python3 scripts/release.py check                -> pass
python3 scripts/release_manifest.py check       -> pass
python3 scripts/check_repository_topology.py check -> pass
python3 scripts/rust_python_parity.py self-test -> comparator catches every injected divergence class
python3 scripts/rust_python_parity.py check     -> 13 NORMATIVE fixtures match across engines
```

The parity gate is reported here because `AGENTS.md` lists it, not because this change could
affect it: no Python reference or Rust source file is touched. It was run rather than assumed.

`cargo` gates (`fmt`, `clippy`, `test`, `deny`) were **not run**: the change touches no file
under `rust/`, and `scripts/check_rust_workspace.py check` plus
`scripts/check_mutation_boundary.py check` - which do read `rust/` - both pass. CI runs the
Rust set regardless.

A defect the executor's own tests found, recorded because it is the useful part: the first
version of the path regex truncated `project/epics/*.json` at the glob, producing
`project/epics/`, which exists - so the real pack passed while a synthetic repository failed.
The repair captures the placeholder as part of the token and checks the directory the pattern
lives in, which is strictly stronger than skipping the citation. `test_a_pattern_under_a_
directory_that_does_not_exist_is_rejected` pins it.

## Compatibility

- The pack is plain Agent Skills; nothing in it is Claude-Code-specific, which is the point -
  `AGENTS.md` assigns the independent review to Codex.
- `scripts/check_agent_skills.py` is stdlib-only and imports nothing beyond `argparse`, `re`,
  `sys`, `pathlib`, like every other governance checker, so it survives the Python -> Rust
  migration in the same way they do.
- The frontmatter parser is deliberately not a YAML parser; richer fields (`hooks`, `metadata`)
  are skipped rather than misparsed. See the docstring.

## Performance / operability

- `python3 scripts/check_agent_skills.py check` completes in well under a second on the
  committed pack; it reads seven files and runs two regexes over each.

## Documentation updated

- `AGENTS.md` - new "Agent skill pack" section, plus the command in "Current Python checks" and
  in the mypy file list.
- `.claude/skills/README.md` - the pack's own contract and the rule it follows.
- `.pre-commit-config.yaml`, `.github/workflows/tests.yml`, `.github/workflows/release.yml`,
  `pyproject.toml` - gate registration.
- `.gitignore` - `.claude/*` with negations, so the pack, the hooks and the shared settings are
  versioned while per-machine session state stays ignored.
- `CHANGELOG.md` is deliberately not updated: no user-visible product behavior changed.

## Residual risks

- **The checker validates that a pointer resolves, not that it is the right pointer.** A skill
  citing `docs/CONSTITUTION.md` where `docs/security/SAFETY_INVARIANTS.md` was meant passes.
  Only review catches that.
- **A pattern is checked by expansion, so it is only as strong as what the tree happens to hold.**
  `project/epics/*.json` passes because twenty-five files match. A pattern that would be broken
  in a fresh checkout but matches one stale file here still passes. Review round 1 found that the
  first implementation did not check patterns *at all* - it validated `docs/*totally/fake/path.md`
  as `docs` - which is the stronger version of this same limitation and is now repaired.
- **A citation with no file extension that does not resolve to a directory is skipped**, because
  `docs/metadata` is prose in `AGENTS.md`, not a path. A genuinely deleted directory cited without
  an extension is therefore not caught.
- **Anchors are not validated.** `docs/X.md#gone` passes, while `scripts/check_docs.py` validates
  anchors for `docs/`. Carried forward as E25-S05 rather than fixed here, per the diff-discipline
  rule: it is a new capability, not a repair.
- **Prose drift is not detected.** The rule "a skill points at the contract, never restates it"
  is enforced only at the level of paths and commands. A skill that paraphrases a rule in its
  own words still passes. Making that mechanical would require semantic comparison; it is
  accepted as a review obligation instead, and stated as such in `AGENTS.md`.
- **Skill descriptions are an activation heuristic, not a contract.** Whether a harness loads
  the right skill at the right moment is a model decision. The gates, not the pack, remain the
  authority - which is why AC5 exists.
- **`WHEN_CLAUSE` accepts any "use when/at/before/after/for/during" phrasing**, including a
  useless one. It catches an omission, not a bad description.
- **Adding a pre-commit gate obliged adding the same command to `release.yml`.** That is the
  repository's own mechanical rule (E22-S01), followed rather than worked around, but it does
  mean a release now re-runs a check that has no bearing on the released artifact. Flagged for
  the reviewer as a deliberate choice, not an oversight.

## Verifier verdict

pending - Codex, at epic scope, once every E24 story is `ready_for_review`
