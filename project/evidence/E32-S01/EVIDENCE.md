# Evidence Packet - E32-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex, round 1 - **FAIL** (`project/evidence/E32-S01-VERIFIER-REVIEW.md`) -
  a verdict-shaped word inside a fenced code example could override a real current verdict;
  round 2 - **FAIL** (`project/evidence/E32-S01-VERIFIER-REVIEW-ROUND2.md`) - the round-1 regex
  repair still missed an unclosed fence and the `~~~` fence style; repaired below with a
  structurally different (line-by-line) parser
- Change Risk: CR2 (process/tooling gate logic in `scripts/project_os.py`; touches no runtime
  mutation code, no shipped Rust/Python product surface, no persisted schema)
- Spec version/commit: `docs/development/AGENT_PROTOCOL.md`'s CR4 Safety Verdict section (updated
  in this change); found by `project/evidence/E12-S04-VERIFIER-REVIEW-ROUND5.md`

## Outcome

PASS after repair (see "Repair - round 2 independent review finding: a line-by-line parser"
below)

## Scope

`scripts/project_os.py::safety_verdict_passes` decided whether a CR4 Safety Verdict file counted
as passing by checking "does a passing line exist anywhere, and does no failing line exist
anywhere" - which can never be true for an honest append-only round history once any round ever
failed, since the protocol requires that round's record to stay in the file rather than be
deleted once a later round repairs and passes it (E12-S04-VERIFIER-REVIEW-ROUND5.md found this
directly: the implementation was independently confirmed correct, but the closing gate refused it
because rounds 1-4's `FAIL`/`REJECT` lines were still present alongside a would-be round-5 pass).
The function now finds the most recent standalone verdict line by position in the file and reads
that one - a later `PASS`/`PASS_WITH_RESIDUALS` closes the story even if an earlier round failed;
a later `FAIL`/`REJECT` still blocks it even if an earlier round passed.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - "A Safety Verdict file whose only failing line(s) are followed, later in the file, by a passing line is accepted." | `test_a_cr4_story_closes_over_an_append_only_history_ending_in_a_pass`'s `repaired` fixture: `FAIL`/`REJECT` (round 1) followed by an inline (non-matching) mention, followed by a genuine `PASS_WITH_RESIDUALS` (round 3) - `safety_verdict_passes` returns `True`. | PASS |
| AC2 - "A Safety Verdict file whose only passing line(s) are followed, later in the file, by a failing line is still refused... the existing test for this must keep passing unchanged." | The pre-existing `test_a_cr4_story_cannot_close_over_a_failing_safety_verdict`'s `mixed` fixture (`PASS` then `REJECT`) still returns `False`, unchanged. The new test's `still_open` fixture (`PASS_WITH_RESIDUALS` then a later round's `FAIL`) also returns `False`. | PASS |
| AC3 - "A Safety Verdict file with no standalone verdict line at all is refused, as today." | The new test's `never_judged` fixture (prose only, no standalone verdict line) returns `False`, matching the function's pre-existing behavior for this case (an empty `verdicts` list). | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Regression test: an append-only fixture with an early FAIL round followed by a later passing round is accepted." | `test_a_cr4_story_closes_over_an_append_only_history_ending_in_a_pass`'s `repaired` case (see AC1). | PASS |
| "Regression test: the existing PASS-then-REJECT fixture stays refused." | `test_a_cr4_story_cannot_close_over_a_failing_safety_verdict`'s `mixed` case, unmodified, still passes. | PASS |
| "`project/evidence/E12-S04/SAFETY_VERDICT.md` itself is exercised as a real-world fixture at each of its own historical states." | Interactively confirmed (not a committed test, since the real file's content is expected to keep changing as E12-S04's own review continues): at the state committed alongside this change (rounds 1-5, ending in round 5's `REJECT`), `safety_verdict_passes` correctly still returns `False` - the fix does not spuriously open the gate, it only stops an early failure from permanently blocking a later, genuine pass. | PASS |

## Adversarial-cases pass (CR2)

| # | Axis | Case | Expected | Test |
| --- | --- | --- | --- | --- |
| 10 | Malformed/untrusted | A file with no verdict-shaped line at all (prose only) | Refused (`False`), not a crash or a default-accept | `never_judged` fixture, see AC3 |
| 7 | Boundary | Multiple verdict lines with the tie-breaking one (by position) being the deciding factor in both directions (pass-then-fail and fail-then-pass) | Only the later one controls, in both directions | `repaired` and `still_open` fixtures, see AC1/AC2 |
| second-path | Does a later `PASS` silently launder an earlier real safety rejection instead of representing an actual repair? | No new authority is created: this function only decides whether a *file a human/verifier wrote* reads as passing - it does not itself decide CR4 safety, verify code, or run gates. Making it read history correctly does not weaken the requirement that a real independent verifier round produced that later `PASS`; `verifier_handoff.py check`'s brief-checksum requirement is unaffected and still separately enforced. | Documented here; no code path in this change touches verdict authorship or attribution |
| 10 | Malformed/untrusted (round 1 repair) | A verdict-shaped word inside a balanced ` ``` ` fenced code block placed after a real current verdict, in both directions | Fenced content never overrides the real verdict, in either direction | `test_a_verdict_word_inside_a_fenced_code_block_is_not_a_verdict` |
| 10 | Malformed/untrusted (round 2 repair) | A fence that opens but is never closed before end of file (both `` ``` `` and `~~~`) | Everything after the opening fence stays code through end of file - fails closed, does not default to prose | `test_an_unclosed_or_tilde_fence_still_hides_its_verdict_shaped_content` |
| 10 | Malformed/untrusted (round 2 repair) | A balanced `~~~` fence (CommonMark's other delimiter, not just ` ``` `) | Recognized and stripped the same as a backtick fence | Same test, `closed_tilde_fence` case |

## Safety Evidence

Not applicable in the CR3+ sense (no `SI-xxx` obligation) - this is CR2 process tooling that reads
an already-written verdict file; it does not itself make a safety determination.

## Verification Commands

```text
python3 -m pytest tests/test_project_os.py -v                              # PASS - 30 tests, incl. 6 new
python3 -m pytest tests -q                                                 # PASS - 668 passed, 578 subtests
python3 -m ruff check .                                                    # PASS
python3 -m ruff format --check .                                          # PASS
python3 -m mypy scripts/project_os.py (+ full AGENTS.md target list)       # PASS
python3 scripts/gen_docs.py --check                                       # PASS
python3 scripts/project_os.py check                                       # PASS
python3 scripts/check_docs.py check                                       # PASS
python3 scripts/check_process.py check                                    # PASS (pre-existing baseline warnings only)
python3 scripts/check_evidence.py check                                   # PASS (pre-existing baseline warnings only)
python3 scripts/verifier_handoff.py check                                 # PASS
python3 scripts/check_ears.py check                                       # PASS
python3 scripts/gate_sensitivity.py check                                 # PASS
python3 scripts/release.py check                                          # PASS
```

Also ran the full AGENTS.md Python check list (`characterize.py`, `diff_harness.py`,
`check_rust_workspace.py`, `check_mutation_boundary.py`, `check_provider_compatibility.py`,
`check_provider_trust.py`, `check_platforms.py`, `rust_python_parity.py self-test`/`check`,
`check_repository_topology.py`, `check_agent_skills.py`, `process_metrics.py`,
`check_risk_classification.py`, `check_agent_toolchain.py`, `check_skill_content.py`,
`safety_oracle.py`) - all PASS, none touched by this diff, no new warnings introduced. Rust checks
were not run: this change touches no file under `rust/`.

## Compatibility

- Pure function behavior change in `scripts/project_os.py`, no CLI/schema/wire-format change. The
  only observable difference is which Safety Verdict files `python3 scripts/project_os.py
  generate`/`check` accepts as closing evidence for a CR4 story - strictly a superset of what
  the prior logic accepted (a single-round file's behavior is unchanged; only a genuine
  multi-round append-only history ending in a pass newly succeeds).

## Performance / operability

- `safety_verdict_passes` now runs two regex scans instead of two `search()` calls and sorts a
  small list of match positions (typically under ten per file) - no measurable cost change.

## Repair - round 1 independent review finding

Codex's round-1 review (`project/evidence/E32-S01-VERIFIER-REVIEW.md`) FAILed: `safety_verdict_passes`
scanned raw file text, so a verdict-shaped word inside a fenced code block - a reproduction
transcript quoting `` `PASS` ``, or a documentation example - was indistinguishable from a real
current verdict declaration. The independent reproduction: a file reading `` `REJECT` `` followed
by a fenced ```` ```text\n`PASS`\n``` ```` block returned `True` from the pre-repair function,
letting an illustrative example override a real, current rejection.

Repair (round 1): `safety_verdict_passes` stripped fenced code blocks with a regex
(`FENCED_CODE_RE`, matching `scripts/check_evidence.py`'s own `FENCED`/`prose_only` convention)
before scanning for verdict lines. Two adversarial tests covered both directions of a *balanced*
fence: a fenced `PASS` must not override a real `REJECT`, and a fenced `REJECT` must not override
a real `PASS`.

## Repair - round 2 independent review finding: a line-by-line parser

Codex's round-2 review (`project/evidence/E32-S01-VERIFIER-REVIEW-ROUND2.md`) FAILed the round-1
regex two further ways: an *unclosed* fence (opened, never closed before end of file) let its
content default back to prose because `^```.*?^```` simply never matched anything for it, and a
balanced `~~~` fence - CommonMark's other delimiter, which the regex never looked for - was
scanned as prose outright. Both reproductions let a later illustrative `PASS` override a real
current `REJECT`.

Per the orchestrator's policy (at most two review rounds per approach before the next attempt
must be a structurally different kind of fix), this repair replaces the single whole-text regex
with `strip_fenced_code`, a line-by-line scanner that tracks whether it is currently inside a
fence (opened by three or more `` ` `` or `~` characters, closed only by a line of at least that
many of the *same* character) and drops every line while inside one. Critically, a fence that is
never closed simply never lets the scanner exit "inside a fence" - its content stays excluded
through end of file by the shape of the algorithm, not by a pattern happening to match past it.
Three new adversarial tests cover an unclosed backtick fence, a closed `~~~` fence, and an
unclosed `~~~` fence (`test_an_unclosed_or_tilde_fence_still_hides_its_verdict_shaped_content`),
alongside the round-1 tests, which still pass unchanged.

## Documentation updated

- `docs/development/AGENT_PROTOCOL.md`: added a note under the verifier procedure's CR4 step
  stating the gate reads the most recent standalone verdict line, not "no failing line anywhere,"
  and citing this story as the reason.

## Method defects

- **What happened**: `scripts/project_os.py::safety_verdict_passes` was written (pre-E32) as "a passing line exists and no failing line exists anywhere in the file," correct only for a single-round Safety Verdict; E12-S01/S02/S03 never exercised the multi-round case within one file (their six epic-level review rounds lived in separate per-epic files), so the gate's blind spot for an honest append-only multi-round history went undetected until E12-S04 became the first CR4 story needing more than one round *and* recording that history in one file per the protocol's own append-only convention. **Prevented by**: a regression test exercising the append-only multi-round case when the append-only convention itself was written into `docs/development/AGENT_PROTOCOL.md`'s handoff section (E28-S02/E28-S03) - the existing suite covered a single pass, a single fail, and pass-then-later-rejection, but never fail-then-later-corrected-to-a-pass, precisely the shape the convention exists to support. **Disposition**: proposed 2026-09-20

## Residual risks

- None specific to this change. The gate's broader limitation (documented already in this
  repository's own review-round measurement work, E25/E29) - that verdict-line parsing is
  necessarily heuristic (freeform markdown, not a structured record) - is unchanged and out of
  this story's narrow scope.

## Verifier verdict

Round 1: **FAIL** (Codex) - `project/evidence/E32-S01-VERIFIER-REVIEW.md`.
Round 2: **FAIL** (Codex) - `project/evidence/E32-S01-VERIFIER-REVIEW-ROUND2.md`. Addressed by
a structurally different (line-by-line) parser, not a further regex patch.
Round 3: **PASS** (Codex) - `project/evidence/E32-S01-VERIFIER-REVIEW-ROUND3.md`. The parser
fails closed for both unclosed and `~~~` fences, including the round-2 reproductions.
