Review-Scope: epic
Verifier: Claude (self-review, not independent)
Date: 2026-09-24
Review-Target: c688121..72cc110 (E35-S01 commits `c688121`, `c16b797`, `72cc110`; E06-S15's `68070ae` read for the delegation only), examined at `72cc110`

# E35 self-review (not independent)

**SELF-REVIEW (not independent).** Claude executed E35. Claude also wrote this review, in a forked
context that did not see the executor's reasoning. The review worked from `project/epics/E35.json`,
`project/evidence/E35-S01/VERIFIER_BRIEF.md`, both independent round records, and the committed
code. This is not a counted round and does not replace the independent reviewer. E35-S01 reached
the owner's two-round cap. It closes on `project/evidence/E35-S01/CEILING_DECISION.md` plus this
review, so the FAIL below is a finding for the owner. It must not be read as a pass. The working
tree was clean at `72cc110`, and nothing was changed except this file.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E35-S01 | FAIL | Both independent rounds' counterexamples are closed. Across 95 historical verdict blobs and all 54 committed `*VERDICT*.md` files, no judgement differs from `0afe3ad`. However, the gate recognises a section boundary only at a line beginning `## ` (`SECTION_HEADING_RE`). A `# ` H1, an ATX heading indented by 1-3 spaces, or a setext heading after a failing final round therefore still counts as part of that round. The verdict below such a heading then decides: `## Round 10` / `FAIL` / `# Owner note` / `PASS` returns `True` from both `project_os.safety_verdict_passes` and `release.safety_verdict_passes`. That is E06 round 10's counterexample with one `#` fewer (AC1, AC2). |

## E35-S01 - reproductions

Every case was written to a temporary `SAFETY_VERDICT.md` and judged by the `HEAD` gate, by
`0afe3ad`'s gate (loaded from `git show 0afe3ad:scripts/project_os.py`), and by
`scripts/release.py`'s reader. In CommonMark each heading below closes the `## Round 10` section.
The `PASS` under it is therefore not "inside the final round's section" (AC1), and it is a
verdict-shaped line "in a section after the final round's" (AC2). The gate must refuse. It passes.

| Text after `## Round 10\n\nFAIL\n\n` | HEAD | `0afe3ad` | release.py |
| --- | --- | --- | --- |
| `# Owner note\n\nPASS\n` (H1) | True | True | True |
| ` ## Owner note\n\nPASS\n` (ATX, 1 leading space) | True | True | True |
| `Owner note\n----------\n\nPASS\n` (setext H2) | True | True | True |
| `Owner note\n==========\n\nPASS\n` (setext H1) | True | True | True |
| `## Owner note\n\nPASS\n` (the E06 round-10 shape, for contrast) | False | True | False |

This is not a regression. `0afe3ad` passed these shapes too, as it passed the H2 shape. It shows
that the class is closed for one heading spelling only.

**Classification: implementation bug.** It has a spec-gap edge: neither the AC nor
`docs/development/AGENT_PROTOCOL.md` says which Markdown constructs end a section.

**Required repair.** Treat every heading of level 1 or 2 as a section boundary after the final
round: ATX with 0-3 leading spaces, and setext `=`/`-` underlines. Alternatively, fail closed by
refusing when any such heading follows the final round and a verdict-shaped line follows that
heading. Add the four rows above as regression cases, and keep the E14-S04/E14-S05 template layouts
passing. Headings of level 3 and deeper (`### Owner note`) are subsections of the round in Markdown
and are covered by AC1 as written. See residual 1.

## Heading variants, whitespace and ordering the brief named (all as the contract requires)

| Case | HEAD | Note |
| --- | --- | --- |
| `## round 11` / `PASS` after the final round's `FAIL` | False | lowercase is a section, not a round, so a verdict there refuses |
| `## Round 10b` after `FAIL`, or after `PASS` with `FAIL` | False / False | `\b` rejects `10b` as a round, so it is a later section and refuses |
| `##Round 11` / `PASS` after `FAIL` | True | not a heading in CommonMark (no space), so it is the round's own body and its last line decides |
| `### Round 11` or `### Owner note` / `PASS` after `FAIL` | True | a subsection of the final round, so AC1's last line decides; see residual 1 |
| a second `## Verdict`, or `## Verdict` after an owner note | False | round 1's repair holds |
| round body `FAIL` and attached `## Verdict` `PASS` (or the reverse) | False | the agreement check holds |
| `FAIL`, prose, `PASS` inside the final round | True | round 2's repair holds |
| blank attached `## Verdict`, then `## Addendum` / `PASS` | False | |
| CRLF line endings on the E06 shape | False | `splitlines` normalises |
| `## Verdict   ` (trailing spaces) | True | |
| `## Verdict: PASS` heading after `FAIL` | False | not the template heading; no standalone verdict after it |
| `## Round 10` / `PASS` then `## Round 3` / `FAIL` | False | position decides, not the round number |
| fenced `PASS`, fenced `## Round 11`/`PASS`, fenced `## Verdict`, unclosed `~~~` after `FAIL` | False | fence stripping precedes round detection |
| lowercase `fail` body with `pass` in `## Verdict` | False | disagreement refuses |

**Legitimately passing template-shaped verdicts pass.** Two shapes were checked with
`project/templates/SAFETY_VERDICT.md` filled (`PASS`, owner `ACCEPT`): appended whole under `## Round 2`
after a `## Round 1` / `FAIL`, and with `## Round 2` inserted before its `## Verdict`. Both return
`True`. With owner `REJECT` the same file returns `False`, as it should. A shape that puts a
`## Summary` between the round heading and `## Verdict` is refused. That refusal fails closed and
is not the template's shape.

## AC3 and the release delegation

- A comparison of `0afe3ad`'s gate (identical to the pre-E35 `f81fe60~1`) with `HEAD` covered all
  54 tracked `*VERDICT*.md` files. Both judge 42 as passing and 12 as refusing, with 0 differences.
  10 of these files carry `## Round <n>` headings. Every blob of a verdict file ever committed was
  also compared: 95 blobs, 0 differences.
- `scripts/release.py` `safety_verdict_passes` loads `scripts/project_os.py` by file location and
  returns its `safety_verdict_passes`. On every committed file and every synthetic case above that
  has round headings, the release reader and the control-plane gate agree.

## Residual risks and out-of-story findings (not the cause of the FAIL)

1. **Spec gap: an H3+ subsection inside the final round decides.** `## Round 10` / `FAIL` /
   `### Owner note` / `PASS` passes, as AC1 literally requires. If an owner note must never decide,
   even as a subsection, the contract has to say so.
2. **Pre-existing E32-S01 parser classes, reachable through the round rule.** `0afe3ad` behaves
   identically on each of the following, and each affects files without rounds equally.
   - (a) `strip_fenced_code` opens a fence on any line that starts with three backticks. That
     includes a line of inline code such as three backticks, `x`, three backticks, ` is the
     command`, and a four-space-indented fence. It then swallows everything to EOF, so
     `## Round 9`/`PASS`, that line, then `## Round 10`/`FAIL` passes on round 9. CommonMark renders
     round 10 as a heading.
   - (b) HTML comments are not stripped, so a `FAIL` followed by `<!--` / `PASS` / `-->`, or by a
     commented `## Round 11`/`PASS`, passes invisibly.
   - (c) A four-space-indented code line `    PASS` counts as a verdict.
   - (d) `str.splitlines` splits on form feed and U+2028. `remarks<FF>PASS` becomes a standalone
     `PASS`, although it renders as one line.
   Suggested carrier: a new engineering-system story against `strip_fenced_code` / `last_verdict`.
3. **A new E35 quirk.** The round body slice starts right after `## Round <n>`, so the heading's own
   tail is read as a line. `## Round 10 PASS` with no verdict in its body passes, where `0afe3ad`
   refused it. No committed file has that shape, so AC3 is unaffected.
4. **E06-S15, not E35: the cutover reader's round precheck runs on the raw text.**
   `release.safety_verdict_passes` requires `ROUND_HEADING_RE` on the unstripped file. A `## Round 1`
   that exists only inside a fenced block satisfies that check. `project_os` then strips the fence,
   finds no rounds, and applies the last-line rule. So a fenced `## Round 1`, then
   `## Verdict`/`FAIL`, then `## Owner note`/`PASS` returns `True` from the release reader. The
   repair is to run the precheck on `strip_fenced_code(text)`. The same reader also raises an
   uncaught `UnicodeDecodeError` on non-UTF-8 bytes. It fails loudly, not open. The committed
   `project/evidence/E06-S04/SAFETY_VERDICT.md` is refused today in either case.
5. **Test and evidence audit.**
   - `test_every_committed_safety_verdict_keeps_its_judgement` pins only 2 files, and it `continue`s
     silently when they are absent. The "every committed verdict" claim in AC3 is a one-time
     measurement, not a standing test.
   - No test covers an H1, indented-ATX or setext heading after the final round.
   - `project/evidence/E35-S01/EVIDENCE.md`'s AC table still names `project_os.final_round_text`,
     which no longer exists.

## Gates run (at `72cc110`, by this reviewer)

- `python3 -m pytest tests/test_project_os.py tests/test_release.py -q`: 102 passed, 140 subtests passed.
- `python3 scripts/project_os.py check`, `check_process.py check`, `release.py check`,
  `gen_docs.py --check`, `process_metrics.py check`, `check_evidence.py check`,
  `verifier_handoff.py check`, `check_docs.py check`: all exit 0. These ran before this file was
  written.
- `.venv/bin/ruff check` and `.venv/bin/ruff format --check` on `scripts/project_os.py`,
  `scripts/release.py` and `tests/test_project_os.py`: pass. `.venv/bin/mypy scripts/project_os.py
  scripts/release.py`: no issues.
- Not run: the full pytest suite, the remaining AGENTS.md script gates, `gate_sensitivity.py`, and
  `gh run list`. Main CI is therefore unknown to this review. No `rust/` file is in the target.

## Documents opened

`AGENTS.md`; `project/epics/E35.json`; `project/evidence/E35-S01/VERIFIER_BRIEF.md`;
`project/evidence/E35-S01/EVIDENCE.md`; `project/evidence/E35-S01/CEILING_DECISION.md`;
`project/evidence/E35-VERIFIER-REVIEW-ROUND1.md`; `project/evidence/E35-VERIFIER-REVIEW-ROUND2.md`;
`project/evidence/E34-SELF-REVIEW.md` (format); `project/templates/SAFETY_VERDICT.md`;
`docs/development/AGENT_PROTOCOL.md` (the Safety Verdict rule paragraph); `scripts/project_os.py`;
`scripts/release.py`; `tests/test_project_os.py`.

## Round verdict

FAIL (self-review, not independent). The one judged story was rejected. Both independent rounds'
findings are repaired, and AC3 holds exactly. What remains is that the section boundary AC1/AC2
depend on is recognised for `## ` only, so a level-1, indented or setext heading reopens E06 round
10's owner-note bypass in both the control-plane gate and the release reader. Under the ceiling
decision this goes to the owner: repair as a follow-up story, or accept it as a recorded residual.
No status, production, generated or other file was changed, and nothing was committed.
