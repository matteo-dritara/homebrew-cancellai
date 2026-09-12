# E24 review record - round 1

- Epic: E24 - Agent Execution Layer
- Review target: `d992383..` on `feat/e24-agent-execution-layer-v2` (commits `e355abd`, `52d5293`)
- Date: 2026-09-12
- Round: 1 of at most 2 (ADR-0014 / PD-022)

## Reviewer independence - read this before trusting the verdicts

`AGENTS.md` assigns the independent review to Codex. The owner explicitly waived that round for
this epic. **This review was therefore performed by Claude reviewing Claude's own work**, and the
verdicts below carry the weaker evidentiary weight that implies.

What was done to mitigate it, and what the mitigation does not buy:

- The review ran in **two separate agent contexts**, neither of which received the executor's
  reasoning, the intent behind the design, or any narrative about why the implementation was
  correct. Each reconstructed the required behaviour from `project/epics/E24.json` and the
  contract documents, then attacked the implementation. Each was told that a finding without a
  reproduction would be discarded.
- Context isolation removes *priming*. It does not remove **self-preference bias**, which is a
  property of the model, not of the conversation. The measured range in the literature is wide
  (roughly -38% to +90% depending on task and dataset), and the mechanism - a judge scores text
  of lower perplexity to itself more highly - applies whether or not the judge saw the author's
  reasoning.
- This repository has its own, smaller measurement of the same effect. Across the fifteen epics
  reviewed by the independent reviewer, **32 of 75 story verdicts in round 1 were `FAIL` - a 43%
  first-pass rejection rate**. The two epics reviewed by Claude instead (E09, E10) produced
  **zero** `FAIL` verdicts between them. Two epics is not a sample, and their content differs, but
  the direction matches the literature and it is the only local evidence available.

The honest reading: this round found real, reproducible defects, so it was not worthless. It is
not evidence that no further defects exist, and it must not be cited as an independent review.
Anything it certifies rests on the reproductions recorded below, not on the reviewer's authority.

## Verdicts

The `Verdict` column is the round's verdict, in the second column, because that is where
`scripts/process_metrics.py` reads it from. Every story here was repaired after the round and
re-verified; the repaired state is the third column.

| Story | Verdict | After repair | CR | Concrete evidence |
| --- | --- | --- | --- | --- |
| E24-S01 | FAIL | PASS_WITH_RESIDUALS | CR0 | Ten findings; four classes of broken pointer passed the checker and one class of valid pointer was wrongly rejected. All ten reproduced. Repairs verified by a 16-case citation matrix and two killed mutants. |
| E24-S02 | FAIL | PASS_WITH_RESIDUALS | CR0 | Five spellings resolving to the same inode as a generated document were allowed through, including the case variant `README.md` names as a mandatory matching rule on APFS. All reproduced with `stat -f %i`. |
| E24-S03 | PASS_WITH_RESIDUALS | PASS_WITH_RESIDUALS | CR0 | All five AC1 clauses present; AC2's justification confirmed against a real instance in `cancellai.py`. One prose tension with the Python reference freeze, repaired. |

## Findings and repairs

### E24-S01 - `scripts/check_agent_skills.py`

| # | Sev | Finding | Repair |
| --- | --- | --- | --- |
| 1 | high | **Placeholder bypass.** Any `*`, `<` or `$` anywhere in a token collapsed validation to "does the prefix before that marker exist", so `docs/*totally/fake/path.md` was validated as `docs`. | A citation containing a placeholder is now expanded as a **glob** and must match at least one real file. `project/epics/*.json` matches twenty-five; `docs/*totally/fake/path.md` matches nothing and is reported. This is strictly stronger than the segment-walk that was tried first, which still accepted the same token one segment deeper. |
| 2 | high | **`./` and `../` citations were invisible.** The regex lookbehind `(?<![\w/.-])` killed any match preceded by `.` or `/`, so a dot-relative pointer was never checked at all. | Lookbehind narrowed; a leading `./` or `../` is now part of the token, and `../` resolves relative to the skill's own directory. |
| 3 | med | **Marker-ordering bug.** `PLACEHOLDER_MARKERS` was scanned in tuple order rather than by position, so a token with two markers truncated at the wrong one and `project/evidence/<STORY-ID>/*.md` - the shape the code's own comment blessed - was wrongly rejected. | Subsumed by the glob expansion; no positional truncation remains. |
| 4 | med | **Root-level files were never validated.** The regex was anchored on directories only, so `AGENTS.md` - cited ten times across five skills, and the single file the pack exists to point at - was outside the checker's reach entirely. | `ROOT_FILES` alternation added, listed explicitly rather than inferred, so deleting one is detectable. |
| 5 | med | **The pre-commit hook did not fire on the drift event it exists to catch.** `files: ^(\.claude/skills/.*\|…)$` matches nothing in a commit that deletes a cited `docs/` file, and pre-commit does not pass deleted paths to hooks at all. CI was unaffected. | `always_run: true`, with the reason recorded inline. The checker reads seven files. |
| 6 | med | **The script-command check had zero effective coverage**, proved by mutation: replacing `SCRIPT_COMMAND` with a regex matching nothing left the whole suite green, because the test asserted on `"does not exist"`, which the *path* error also produces. | Assertion narrowed to `"names a script that does not exist"`. Re-run of the same mutant now fails the suite (1 failure); a second mutant neutering `_check_citation` fails it in 10 places. |
| 7 | low | Valid YAML frontmatter was rejected: quoted scalars failed as a name mismatch, folded/literal block scalars failed as "does not say when the skill applies" - a wrong diagnosis. | Quotes stripped; `>` and `\|` block scalars joined from their continuation lines. |
| 8 | low | A false positive contradicted the code's own comment: `docs/metadata`, the literal phrasing `AGENTS.md` uses for the CR0 category, was reported as a broken path. | A token counts as a pointer only when it names a file extension or already resolves to a directory. |
| 9 | low | BOM and an empty frontmatter block were both misdiagnosed as "missing or unterminated". | Three distinct messages: no frontmatter, unterminated, empty. BOM stripped. |
| 10 | low | Anchors (`docs/X.md#section`) were never validated, while `scripts/check_docs.py` validates them for `docs/`. | **Not repaired.** Anchor validation is a new capability rather than a repair, and `AGENTS.md`'s own diff-discipline rule says to flag rather than widen. Carried forward as E25-S05. |

### E24-S02 - `.claude/hooks/guard-generated-docs.sh`

| # | Sev | Finding | Repair |
| --- | --- | --- | --- |
| 1 | high | **Case variants were allowed.** `docs/backlog.md` and `DOCS/BACKLOG.md` are inode `138097367`, the same file as `docs/BACKLOG.md`, and passed. `README.md` states protected names are "matched case-insensitively, because macOS mounts APFS case-insensitively and a barrier must not depend on how a name happens to be spelled" - this guard depended on exactly that. | Matching now happens on `os.path.realpath`, made relative to the project directory, lowercased. |
| 2 | high | `docs//BACKLOG.md` allowed - same inode, but the `case` glob required a literal single separator. | Same repair; `realpath` normalises it. |
| 3 | med | `docs/adrs/../BACKLOG.md` allowed. The guard survived only the `..` forms that happened to leave the tail intact. | Same repair. |
| 4 | med | A bare relative path `docs/BACKLOG.md` was allowed while `./docs/BACKLOG.md` was refused - inconsistent handling of one target. | Same repair; relative paths are joined to the project directory first. |
| 5 | med | `docs/cli.md` allowed. | Same repair. |
| 6 | med | **Matcher gap.** `Edit\|Write\|NotebookEdit` omits `Bash`, which writes files freely. The hook is never invoked for the most common write path an agent has. | **Not closed, and deliberately so** - matching it would mean parsing shell, which a guard should not attempt. The limit is now stated in the hook's header, in `.claude/skills/README.md`, and pinned by `GuardCoverageIsHonestlyBoundedTests`. The CI drift check catches Bash writes as it catches every other path. |
| 7 | low | Over-blocking: any path on the machine ending `/docs/BACKLOG.md` was refused, including the synthetic temp trees `AGENTS.md` requires tests to use. | Fixed for free by matching on a project-relative path: paths outside the project directory no longer match. Pinned by `test_a_path_outside_the_project_is_never_refused`. |

### E24-S03 - `AGENTS.md` "Diff discipline"

| # | Sev | Finding | Repair |
| --- | --- | --- | --- |
| 1 | low | The cleanup exception carried no file-scope caveat, while the Python reference freeze bars a cleanup-only change to `cancellai.py` unconditionally. A reader could take the exception as authorising one. | Caveat added: the exception does not reach `cancellai.py`, and a cleanup story cannot grant what the freeze withholds. |

## Corrections made to the executor's own evidence

The review found two claims in the committed packets that were true of what was tested but did not
support the acceptance criterion as written. Both packets were corrected rather than left standing:

- `E24-S01`, AC2 row claimed "16 cases, one per rejection path", including the missing-script
  path. The mutation above showed that path was not covered. Corrected, with the mutation result
  recorded.
- `E24-S02`, AC1 row was certified by tests that fed one canonical, correctly-cased absolute
  spelling per document, so findings 1-5 were invisible to the suite certifying the AC. AC3's
  matrix row also overstated "every other path is unaffected", which the packet's own residual
  risks then contradicted. Both corrected.

## Gates re-run after repair

```text
python3 -m pytest tests -q                      -> green (50 passed, 38 subtests in the two E24 files)
ruff check . / ruff format --check . / mypy     -> green
python3 scripts/check_agent_skills.py check     -> agent skills OK: 7 skills
python3 scripts/check_docs.py check             -> docs OK: 258 Markdown files
python3 scripts/project_os.py check             -> governance OK
full governance set (22 commands, AGENTS.md)    -> green
```

Two mutants were run against the repaired suite to check it discriminates rather than accompanies:
neutering `SCRIPT_COMMAND` fails 1 test; neutering `_check_citation` fails 10. Both were green
against the pre-repair suite.

## Round verdict

**PASS_WITH_RESIDUALS**, subject to the independence caveat at the top of this record.

Residuals accepted and carried forward as backlog work in E25: anchor validation (S01/10), the
Bash write path (S02/6), and the structural gap this review exposed in the process itself - that
an executor's evidence packet is prose nothing validates, which is what allowed two overstated AC
rows to sit in `ready_for_review` with every gate green.
