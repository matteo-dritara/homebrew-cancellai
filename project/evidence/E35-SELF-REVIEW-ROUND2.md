Review-Scope: epic
Verifier: Claude (self-review, not independent)
Date: 2026-09-24
Review-Target: 72cc110..54cda22 (E35-S01 `54cda22`; E06-S15's `51ca9c4` read for the release delegation only), examined at `54cda22`

# E35 self-review, round 2 (not independent)

**SELF-REVIEW (not independent).** Claude executed E35 and also wrote this review, in a forked
context that did not see the executor's reasoning. This is not a counted round, and it does not
replace the independent reviewer. It re-attacks E35-S01 (CR2) after the repairs to
`project/evidence/E35-SELF-REVIEW.md`. The scope is `scripts/project_os.py` (`section_starts`,
`final_round_verdict`, `safety_verdict_passes`) and `scripts/release.py` `safety_verdict_passes`
(the delegation and the fence-stripped round precheck).

E35-S02 covers the pre-E32 reader differences: HTML comments, inline triple backticks, indented
code, and form feed or U+2028. They are out of scope here. The working tree was clean at
`54cda22`, and the only change is this file.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E35-S01 | FAIL | The setext repair places the section boundary on the underlined line's immediate predecessor, not on the first line of the paragraph the underline turns into a heading. In a multi-line setext heading, the earlier lines therefore stay in the final round's body. A `PASS` there decides: `## Round 10\n\nFAIL\n\nPASS\nOwner override\n--------------\n` returns `True` from `project_os.safety_verdict_passes` and from `release.safety_verdict_passes`. CommonMark renders this as round 10 = `FAIL`, followed by an H2 heading "PASS Owner override". The `PASS` source line lies in the section that heading opens, so AC2 requires a refusal (AC1, AC2). This is the round-1 self-review's own setext case, closed only for a one-line paragraph. AC3 holds exactly (54 files and 95 blobs, 0 differences). |

## E35-S01 - reproductions

Each case was written to a temporary `SAFETY_VERDICT.md` and judged three ways: by the `HEAD` gate,
by `0afe3ad`'s gate (from `git show 0afe3ad:scripts/project_os.py`), and by `scripts/release.py`'s
reader. "Required" is the result CommonMark rendering and AC1/AC2 call for. `F` stands for
`## Round 10\n\nFAIL\n\n`.

| Case | HEAD | `0afe3ad` | release.py | Required |
| --- | --- | --- | --- | --- |
| `F` + `PASS\nOwner override\n--------------\n` (setext H2, 2-line paragraph) | True | True | True | False |
| `F` + `PASS\nOwner override\n==============\n` (setext H1, 2-line paragraph) | True | True | True | False |
| `F` + `PASS\nper owner\nOwner override\n---\n\nApproved.\n` (3-line paragraph) | True | True | True | False |
| `## Round 10\nPASS\nnote\n---\n` (setext directly after the round heading; the body is empty when rendered) | True | True | True | False |
| `## Round 10\n\nPASS\nOwner note\n---\n` (no standalone verdict in the rendered body) | True | True | True | False |
| CRLF form of the first row | True | True | True | False |
| `## Round 2\n\n## Verdict\n\nFAIL\n\nPASS\nOwner\n---\n` (the attached `## Verdict` section is outvoted the same way) | True | True | True | False |
| `F` + `PASS\n---\n` (1-line paragraph, for contrast) | False | True | False | False |

These cases are not a regression, because `0afe3ad` passes them too. They show that the repair in
`54cda22` is incomplete for the heading class it claims to close. Its docstring says "setext (a
paragraph line underlined with `=` or `-`)". The round-1 self-review required "setext `=`/`-`
underlines" as section boundaries. In Markdown, the underline turns the whole preceding paragraph
into the heading, not just its last line.

**Classification: implementation bug.**

**Required repair.** In `section_starts`, a setext underline should start the section at the
first line of the paragraph it closes. That paragraph begins at the first non-blank line after the
last blank line, ATX heading or setext heading. The previous line should not be used. Add the
setext-H2, setext-H1, "directly after the round heading" and attached-`## Verdict` rows above to
`test_the_final_round_decides_and_a_later_section_cannot` as regression cases. The prototype below
shows that the repair is safe for AC3.

**The repair was checked in the scratchpad only; no repository file was changed.** I replaced
`section_starts` with that paragraph-start rule and ran every row above, every case in the next
section, the template shapes, and the AC3 sweep. Every multi-line setext row became `False`. `---`
and `***` thematic breaks stayed inside the round (`True`). The template shapes kept their
judgement. The AC3 sweep over all 54 tracked files and all 95 historical blobs showed 0
differences. Leaving the paragraph open across a thematic break or a list item errs toward
refusing, which is the fail-closed direction.

## Shapes the brief named that behave as the contract requires

| Case (after `F` unless stated) | HEAD | Note |
| --- | --- | --- |
| `# Owner note #` (closing `#`s), `   ## Owner` (3 spaces), `#\tOwner`, bare `#` | False | ATX with 0-3 spaces, a tab separator, a closing sequence or empty content ends the round |
| `Owner override\n---` / `PASS` (1-line setext), CRLF `## Owner note`, CR-only `## Owner note` | False | `strip_fenced_code` normalises line endings to `\n` before any scan |
| `---` or `***` thematic break (after a blank line, or directly after the round heading), `- - -` | True | a thematic break is not a heading, so the round's own last line decides (AC1) |
| `\t## Owner` / `PASS`, `    ## Owner` / `PASS` | True | an indented code block, not a heading, in CommonMark (the code line itself is E35-S02) |
| `> ## Owner` / `PASS`, `- ## Owner` / `PASS` | True | a heading nested in a blockquote or list item does not end the document section; the round's own `PASS` decides |
| `## round 11`, `# Round 11`, ` ## Round 11`, `##  Round 11`, `##\tRound 11`, `Round 11\n--------`, each followed by `PASS` | False | not the round spelling, so it is a later section, and a verdict there refuses |
| `PASS (owner)` | False | not a standalone verdict (unchanged since E32-S01) |
| `## Round 10 FAIL` / `## Verdict` / `PASS` | True | the heading's tail is never a verdict line, in either direction (consistent with `## Round 10 PASS` refusing) |
| `## Verdict ##`, ` ## Verdict`, `Verdict\n-------` as the attached section | False | not the template's `## Verdict` spelling, so it is refused; this fails closed and is not a template shape |

**Legitimate template-shaped verdicts pass.** I filled `project/templates/SAFETY_VERDICT.md` with
`PASS` and owner `ACCEPT` and judged five shapes:

- appended whole under `## Round 2` after `## Round 1` / `FAIL`: `True`
- `## Round 2` inserted before `## Verdict`, with LF line endings: `True`
- the same with CRLF: `True`
- with a `PASS` body before `## Verdict`: `True`
- with owner `REJECT`: `False`, as required

A template followed by a setext owner note with `FAIL` also refuses. HEAD, `0afe3ad` and
release.py agree on every one of these.

## AC3 and the release delegation

- **Tracked files.** HEAD's gate and `0afe3ad`'s gate were compared on all 54 tracked
  `*VERDICT*.md` files. Both judge 42 as passing and 12 as refusing, with 0 differences. 10 of the
  files carry `## Round <n>` headings.
- **History.** Every blob of a `*VERDICT*.md` path in `git log --all` was also compared: 95 blobs,
  0 differences.
- **Release delegation.**
  - `release.safety_verdict_passes` loads `project_os.py` by file location and delegates to its
    gate.
  - Its round precheck now runs on `strip_fenced_code(text)`, and undecodable bytes return `False`.
  - Wherever an unfenced `## Round <n>` exists, the release reader agreed with the control-plane
    gate on every case in this review. It differs only by design, when no round heading is
    recognised (residual 1).

## Residual risks and out-of-story findings (not the cause of the FAIL)

1. **Spec gap: the E35 rule turns on one spelling of the round heading.**
   - **Mechanism.** `ROUND_HEADING_RE` is `^## Round \d+\b`. A file whose rounds are all spelled
     `# Round 10`, ` ## Round 10`, `##\tRound 10`, `##  Round 10` or `Round 10\n--------` renders
     them as round headings, but the gate recognises none of them. It falls back to E32-S01's
     last-line rule.
   - **Effect.** For example, `# Round 10\n\nFAIL\n\n# Owner note\n\nPASS\n` returns `True` from
     `project_os`. That is E06 round 10's owner-note bypass, and `release.py` returns `False` on
     the same file because of its precheck.
   - **BOM variant.** A UTF-8 BOM before a first-line `## Round 10` does the same, because `^`
     does not match after `﻿`. cmark skips a leading BOM when rendering.
   - **Why it is not an AC violation.** AC1 and `docs/development/AGENT_PROTOCOL.md` name
     `## Round <n>` literally, and no committed verdict uses another spelling. It is still the same
     heading-spelling class, applied to the round heading instead of the section boundary.
   - **Options.** Recognise every level-1/2 heading whose text is `Round <n>` as a round, or refuse
     a file that has such a heading in an unrecognised spelling. Either needs an AC3 re-sweep.
2. **A pre-existing E32-S01 reader difference, not in E35-S02's list: soft line breaks.** Take
   `F` + `The owner says\nPASS\n`. CommonMark renders the paragraph "The owner says PASS", yet the
   gate reads `PASS` as a standalone line and returns `True`. `0afe3ad` does the same. This is the
   same "source line versus rendered line" class as E35-S02, and its AC should name it. It is also
   the underlying reason the multi-line setext case above exists.
3. **A second copy of the round pattern.** `scripts/release.py` defines its own
   `ROUND_HEADING_RE`, identical to `project_os.ROUND_HEADING_RE` today. Its docstring says "one
   rule, not two copies". If residual 1 is repaired in `project_os` only, the precheck will diverge
   silently. The precheck should use `module.ROUND_HEADING_RE`.
4. **Test audit.**
   - No test covers a multi-line setext heading. This is the FAIL above.
   - `tests/test_release.py` covers none of the new heading shapes and relies on the delegation.
     That is acceptable while the delegation stands, but a mutation that re-copies the rule into
     `release.py` would not be caught.
   - `test_every_committed_safety_verdict_keeps_its_judgement` still pins two files and `continue`s
     when they are absent, so AC3's "every committed verdict" remains a one-time measurement,
     re-made here.

## Gates run (at `54cda22`, by this reviewer)

- `.venv/bin/python -m pytest tests/test_project_os.py tests/test_release.py -q`: 102 passed,
  148 subtests passed.
- `scripts/project_os.py check`, `check_process.py check`, `release.py check`,
  `gen_docs.py --check`, `process_metrics.py check`, `check_evidence.py check`,
  `verifier_handoff.py check` and `check_docs.py check`: all exit 0. These ran before this file was
  written.
- `ruff check` and `ruff format --check` on `scripts/project_os.py`, `scripts/release.py`,
  `tests/test_project_os.py` and `tests/test_release.py`: pass. `mypy scripts/project_os.py
  scripts/release.py`: no issues.
- `gh run list --branch main` for `54cda22`:
  - `tests`: success
  - `governance`: success
  - `codeql`: in progress, so unknown
  - `rust`: in progress, so unknown
- Not run: the full pytest suite, the remaining AGENTS.md script gates, and `gate_sensitivity.py`.
  No `rust/` file is in the target.

## Documents opened

`AGENTS.md`; `project/epics/E35.json` (E35-S01 and E35-S02 ACs);
`project/evidence/E35-SELF-REVIEW.md`; `project/evidence/E35-S01/EVIDENCE.md`;
`project/evidence/E35-S01/CEILING_DECISION.md`; `project/templates/SAFETY_VERDICT.md`;
`docs/development/AGENT_PROTOCOL.md` (the Safety Verdict rule paragraph); `scripts/project_os.py`;
`scripts/release.py`; `tests/test_project_os.py` (the AC3 pin test and the `54cda22` cases);
`tests/test_release.py` (the round cases); `git show 54cda22`, `git show 51ca9c4`,
`git show 0afe3ad:scripts/project_os.py`.

## Round verdict

FAIL (self-review, not independent). The single judged story is rejected.

**What holds:**

- Every ATX spelling of a level-1/2 heading ends the final round.
- One-line setext headings end the final round.
- The round heading's tail no longer decides.
- The release precheck reads fence-stripped text.
- Legitimate template-shaped verdicts pass.
- AC3 holds exactly.

**What remains:** a multi-line setext heading still leaves its leading paragraph lines inside the
final round. A `PASS` placed there reopens the owner-note bypass in both the control-plane gate
and the release reader.

The repair is local to `section_starts`, and a scratch prototype shows it is AC3-safe. Under the
ceiling decision the story goes to the owner, who can either have it repaired or accept it as a
recorded residual. No status, production, generated or other file was changed, and nothing was
committed.
