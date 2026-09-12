# Evidence Packet - E24-S02

- Commit/PR: working tree at the E24 checkpoint commit
- Executor: Claude
- Independent verifier: Codex (pending; review runs at epic scope)
- Change Risk: CR0
- Spec version/commit: project/epics/E24.json at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - generated planning documents are refused with the right instruction (**corrected after review**: the original row was certified by tests that fed one canonical, correctly-cased absolute spelling per document, so five spellings resolving to the same inode were invisible to the suite that certified it - see `project/evidence/E24-VERIFIER-REVIEW.md`. The guard now matches on the real, project-relative, case-folded path, and `test_every_spelling_of_the_same_file_is_refused` pins seven spellings.) | `tests/test_agent_hooks.py::GuardRefusesGeneratedDocumentsTests::test_the_generated_planning_documents_are_refused` (subtests for `DECISION_REGISTER.md`, `ROADMAP.md`, `BACKLOG.md`) and `::test_anything_under_project_generated_is_refused`. Each asserts exit 2 and that stderr names `python3 scripts/project_os.py generate`. `::test_the_refusal_names_the_source_to_edit_instead` asserts the source file is named too. | PASS |
| AC2 - docs/CLI.md names its own generator | `::test_the_generated_cli_reference_names_its_own_generator` - exit 2, stderr names `scripts/gen_docs.py`. | PASS |
| AC3 - every other path is unaffected | `GuardAllowsEverythingElseTests` - a Rust source file, the hand-written JSON sources, `docs/CLI_RUST.md`, a lookalike directory inside the project, and paths outside the project directory entirely. All exit 0. **Corrected after review:** the original row claimed this without qualification while the packet's own residual risks conceded that any path on the machine ending `/docs/BACKLOG.md` was refused - including the synthetic temp trees `AGENTS.md` requires tests to use. Matching on a project-relative path removed the over-blocking rather than the row being softened. | PASS after repair |
| AC4 - registered in `.claude/settings.json` | `.claude/settings.json` registers the `PreToolUse` hook on `Edit\|Write\|NotebookEdit`. `.gitignore` was changed to `.claude/*` plus negations so this file is versioned, while `settings.local.json`, worktrees, caches and locks stay ignored - verified with `git status --short --untracked-files=all .claude/`, which lists exactly the ten intended files and no session state. | PASS |
| AC5 - fails open on anything it cannot parse | `GuardFailsOpenTests` - malformed JSON, empty input, a payload with no `file_path`, a payload that is not an object, and a null `file_path`. All exit 0. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A guard that can halt the session on a malformed payload | `GuardFailsOpenTests`, five cases. The hook is a convenience guard and must fail open by construction; a guard that blocks unrelated work is worse than the mistake it prevents. | PASS |
| n/a | The hook silently becoming the authority on drift | `GuardDoesNotReplaceTheDriftCheckTests::test_the_authority_on_drift_is_still_the_governance_checker` runs `scripts/project_os.py check` as a subprocess with no hook involved and asserts it passes on its own. The hook shifts *when* one class of mistake is caught, never *whether* it is caught. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_agent_hooks.py -q   -> 14 passed, 6 subtests passed
python3 -m pytest tests -q                       -> 301 passed, 34 subtests passed
git status --short --untracked-files=all .claude/-> 10 files, no session state
```

Full gate set as recorded in `project/evidence/E24-S01/EVIDENCE.md`; the two stories were
implemented and gated together.

## Compatibility

- The hook is a POSIX shell script. `tests/test_agent_hooks.py` skips on Windows rather than
  failing there, and the skip message states what covers Windows instead: the CI drift check,
  which is the authority in every case.
- It shells out to `python3` only to read one field out of the payload, and swallows that
  failure - so a machine without `python3` on `PATH` gets a guard that always allows, never one
  that always blocks.

## Performance / operability

- One short-lived `python3` process per `Edit`/`Write` tool call. Measured inside the test run:
  the 14-test file, which spawns the hook 20 times, completes in 0.72 s.

## Documentation updated

- `AGENTS.md` - "Agent skill pack" section covers the pack and its hook.
- `.claude/skills/README.md` - the hook, what it does, and that it is opt-in configuration.
- `.gitignore`, `.claude/settings.json`.

## Residual risks

- ~~Path matching is textual, by suffix.~~ **Repaired after review.** Matching is on
  `os.path.realpath`, made relative to the project directory and compared case-insensitively -
  the rule `README.md` already states for protected provider names. Seven spellings of one inode
  are pinned as refused; three paths outside the project are pinned as allowed.
- **The guard covers the harness's own file tools only.** A write through `Bash` (`sed -i`,
  a heredoc, `python3 -c`) is not intercepted - review round 1 raised this as a matcher gap, and
  it is accepted rather than closed: matching `Bash` would mean parsing shell, which is not
  something a guard should attempt. The limit is now stated in the hook's header and in
  `.claude/skills/README.md`, and pinned by `GuardCoverageIsHonestlyBoundedTests`, so it cannot
  be quietly forgotten. The CI drift check remains the backstop and catches every path.
- **It is per-repository configuration, so it does not apply to an agent run from elsewhere**
  (a different `--add-dir` root, or CI). Again: backstop, not boundary.
- **`.gitignore` now uses negation.** If someone later re-adds a broad `.claude/` rule, the
  negations stop working silently and the pack falls out of version control.
  `scripts/check_agent_skills.py` would keep passing locally while CI lost the files - the
  narrow protection is that the CI step would then fail with "no agent skill pack at". Worth a
  reviewer's attention.

## Verifier verdict

pending - Codex, at epic scope, once every E24 story is `ready_for_review`
