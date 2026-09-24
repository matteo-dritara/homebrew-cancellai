Review-Scope: epic
Round: 2
Verifier: Codex
Date: 2026-09-24
Review-Target: dc50a95020831e9dde6b960a27669710eadfe72f..cdf5dccff0829eed2af837e64c0873a74309690b

# E34 independent verifier review, round 2

All five stories were `ready_for_review` when this round began. I reviewed the committed briefs and the repair diff, then used disposable Git repositories and synthetic configuration trees to probe boundaries. No E34 story is CR4, so no Safety Verdict is due.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E34-S01 | FAIL | After a passing pre-review check, changing the record to `wrong header / Verifier: forged` leaves `import_problems` empty; `cmd_import` returns 0 and imports those bytes. Import does not re-run `check_record` or verify the run's model attribution. Brief-Checksum: bdd4184cfe0cb67b89f90f7dd271af87928b2c7075da6d0dc15d6de36a559daa |
| E34-S02 | FAIL | Both agents allow `python3 scripts/check_agent_toolchain.py updates`; that script's `cmd_updates` invokes `gh api` against GitHub. The deny patterns do not match this command, so the agents do not deny a web-reaching command. Brief-Checksum: 5e918fad438f54e004dbca7c33dedb69e3e699fddccb9db97126c70add803454 |
| E34-S03 | PASS | In a disposable evidence tree, one pre-review with a `FAIL` row appeared only in `ADVISORY`; three formal records counted as rounds with reviewer families `Codex`, `OpenCode/model`, and `unnamed`. Brief-Checksum: bd7b2ae65c48a06e3c3a8221bc71a721d401ed05a5f2f3409b8cc5fed654f18a |
| E34-S04 | FAIL | A synthetic `.opencode/agents/` holding `known.md` and `.hidden.md` reported only `subagent:opencode/known`; the unrecognised hidden entry was silently omitted. Brief-Checksum: 4eef8d625dfc4c40905296b5434dc66a63a729ff4896e09fe7f6741c716d3d8a |
| E34-S05 | FAIL | Re-rendering bodies A, B, A, B on four dates replaced the first archive of A (`Rendered-on: 2026-09-01`) with the later A (`Rendered-on: 2026-09-03`) at the same checksum path. The earlier brief was not kept byte for byte. Brief-Checksum: 4b9fbf7d73e21b96b246696bccd4e73eeb6b623cd3fec902aee66adc2391387e |

## E34-S01: import accepts a record that no longer passes

I made a disposable source repository at a committed base and a separate review clone. The review clone initially held a valid `E34-PRE-REVIEW-1.md`; `check_record` returned no problems. I then replaced the file with `wrong header\nVerifier: forged\n`. `check_record` reported both a wrong verifier and missing advisory header, while `import_problems` returned `[]`; with a passing run file, `cmd_import` returned 0 and copied the invalid record to the target. The changed-path list was unchanged, so the current import check did not notice the record-content change after the run check.

This violates S01 AC5: only a round that passes its checks may be imported. The same gap lets a changed record misstate its reviewer after the check, and import does not revalidate the stream-model evidence behind AC3. Required repair: re-run the record and attribution checks on the exact bytes to be imported, bind the run metadata to the checked output, and refuse any post-check change before copying. Include a regression that edits a passing record without changing its path list. The staged-rename and target-appearance cases from round 1 now have targeted checks. I could not execute the real OpenCode pre-review contract here because this worktree has no usable OpenRouter credentials.

## E34-S02: allowed script reaches the web

I applied the committed agent bash rules in order to `python3 scripts/check_agent_toolchain.py updates`; the decision was `allow` for both `verifier` and `pre-reviewer`. The script's `cmd_updates` invokes `gh api repos/...` when `gh` is available. The permission rule sees only the allowed Python command, not the network action inside it. This violates S02 AC1's explicit denial of commands that reach the web through an allowed interpreter. Required repair: remove this broad script grant or allow only reviewed script/subcommand pairs whose call graphs cannot perform the prohibited actions; enforce network isolation if arbitrary tests must run. Test a permitted-looking command that calls a network-capable script. The pinned model and default-deny rules remain present; I did not claim a live OpenCode smoke run.

## E34-S03: advisory accounting passes

`process_metrics.load_rounds` counted the three synthetic formal records and excluded the pre-review despite its `FAIL` row. Its reviewer lines retained the two named families and reported the missing name as `unnamed`. This covers all three ACs. S03 remains `blocked` in project state while its dependency S01 is rejected.

## E34-S04: hidden entry is invisible

With `check_agent_toolchain.ROOT` pointed at a synthetic tree, `_opencode_components` found `.opencode/agents/known.md` but ignored `.opencode/agents/.hidden.md` because `_visible_children` drops every name beginning with a dot. This violates S04 AC2, which requires an unrecognised `.opencode/` entry to be reported instead of ignored. Required repair: enumerate these entries and classify them explicitly as recognised local state or unrecognised; add a synthetic hidden-child case. S04 is also dependency-held by failed S02.

## E34-S05: superseded bytes are overwritten

I patched the brief renderer in a disposable evidence tree to yield A, B, A, B on consecutive dates. On the second render it archived the first A under `VERIFIER_BRIEF.superseded-53bcc2a7644b.md` with `Rendered-on: 2026-09-01`. On the fourth render, `write_brief` wrote that same archive path again with `Rendered-on: 2026-09-03`. The original file's bytes were lost even though its checksum path still existed. This violates S05 AC1's byte-for-byte preservation of a superseded brief. Required repair: refuse to overwrite an existing archive, or retain distinct immutable archives when a checksum recurs with different header bytes; add an A→B→A→B regression.

## Gates and limits

- `python3 scripts/project_os.py check`, `status`, `next`, `review`, and all five `brief <ID> --role verifier` commands: passed at session start.
- `python3 scripts/check_agent_toolchain.py report`: passed; no component decision was overdue.
- `python3 -m pytest tests -q`: first run had 761 passes, 3 skips and two failures: S03's newly blocked status lacked `blocked_by`, and the pre-existing untracked `.opencode-run/prompt.md` had an invalid relative link. After adding explicit blockers and temporarily parking that log directory, the full suite passed: 763 passed, 3 skipped, 741 subtests passed.
- `pre-commit run --all-files`: first run failed `docs-check` on that same untracked log. With the log temporarily parked, all hooks passed, including ruff, mypy, project governance, documentation, process metrics, verifier handoff, fixture/parity checks and release checks. The log directory was restored unchanged.
- `python3 scripts/project_os.py check`, `python3 scripts/check_process.py check`, `python3 scripts/verifier_handoff.py check`, and `python3 scripts/process_metrics.py check`: passed after the status and generated-file updates. `check_process.py` printed its existing historical round-ceiling warnings but exited 0.
- `gh run list --branch main --limit 5`: exit 4 because GitHub CLI is unauthenticated. Main CI remains unknown.
- No Rust production path changed; the Rust workspace quality matrix does not apply to this process-only review target.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/AGENT_TOOLCHAIN.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `project/epics/E34.json`; `project/decisions.json`; `project/evidence/E34-VERIFIER-REVIEW-ROUND1.md`; `project/evidence/E34-S01/VERIFIER_BRIEF.md`; `project/evidence/E34-S02/VERIFIER_BRIEF.md`; `project/evidence/E34-S02/VERIFIER_BRIEF.superseded-f67a2ee4c768.md`; `project/evidence/E34-S03/VERIFIER_BRIEF.md`; `project/evidence/E34-S04/VERIFIER_BRIEF.md`; `project/evidence/E34-S05/VERIFIER_BRIEF.md`.

## Overall verdict

FAIL: four of five judged stories have reproduced defects, an 80% round yield. The findings overlap round 1 on S01, S02 and S04, so the zero-overlap escalation is not triggered. ADR-0025 requires another round after repair; round 3 is the cost ceiling, and any closure at that ceiling requires an owner decision. S01, S02 and S05 return to `in_progress`. S03 is `blocked` by S01. S04 is `blocked` by S02 and has its own failed verdict. The epic remains `in_progress`. This verifier changed no production code and made no commit.
