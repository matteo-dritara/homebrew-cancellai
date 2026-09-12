# E25 / E26 review record - round 2

- Epics: E25 Engineering System Falsification, E26 Agent Toolchain Governance
- Review target: the E25/E26 completion work on `main`
- Date: 2026-09-12
- Round: 2 of a cost ceiling of 3 (ADR-0025). Round-2 yield was high, so under the yield rule this
  epic would require another round; the owner closed it here, which ADR-0025 requires be recorded
  as an owner decision rather than an automatic close. **This is that record.**

## Reviewer independence

**Self-review**, per `docs/development/AGENT_PROTOCOL.md`'s "What \"independent\" can mean here":
Claude reviewing Claude's own work in an isolated agent context that received none of the
executor's reasoning, with the owner having waived the Codex round. It may repair; it is not the
gate a CR3/CR4 story names. No story here is above CR2.

## Verdicts

| Story | Round-2 verdict | After repair | CR | Concrete evidence |
| --- | --- | --- | --- | --- |
| E25-S06 | **FAIL** | PASS | CR2 | The harness was measuring the export, not the gates. |
| E25-S07 | PASS_WITH_RESIDUALS | PASS_WITH_RESIDUALS | CR2 | Two predicates check only themselves; the protected-name *set* came from the code. |
| E25-S08 | **FAIL** | PASS | CR0 | Two acceptance criteria were unmet and one was unimplementable as built. |
| E25-S09 | PASS | PASS | CR2 | All 24 cited invariants exist; no undeclared claim. |
| E25-S11 | PASS | PASS | CR0 | Readership reproducible from committed records. |
| E26-S02 | PASS_WITH_RESIDUALS | PASS | CR1 | It printed a pin beside a date and called it a comparison. |
| E26-S03 | PASS_WITH_RESIDUALS | PASS | CR1 | No usage record existed, so two criteria were unexercised. |

## The three findings that mattered

**The harness was measuring the copy, not the gates.** `export_tree` excludes `.git`, and one test
asserted a property of this repository's commit history - so `pytest` **failed on an unmutated
export**, and every mutant it "killed" was killed by a pre-existing failure. Three of the four
invariants the report claimed coverage for were pytest-only. The review proved it with a null
mutant whose `find` and `replace` were identical: the report recorded it as killed.

Three repairs. The test now skips with no git history, which also fixed a real defect - **the suite
had been failing in any source export, including the released tarball**. A **control pass** runs
every gate on an unmutated copy first; a gate that fails there kills nothing, its kills leave the
coverage count, and `check` exits non-zero instead of reporting the problem in prose and passing
anyway. And `evaluate` no longer short-circuits, so a mutant's second gate is actually run.

**A mutant was dead and certified an invariant anyway.** `protected-name-case-sensitive` anchored
on the first `.lower()` in `cancellai.py`, which is `extract_uuid`'s session-id lowercasing. It
touched no protected-name code, survived the whole suite, and the report and the evidence packet
both recorded SI-006 as verified. Re-anchored on `protected_component`'s own `canonical_name`
folding, it now kills `test_decomposed_protected_directory_is_refused_at_deletion`.

**The oracle read the protected-name set from the code it was checking.** So deleting a name
removed it from *both* sides and the oracle stayed green - demonstrated with a mutant that deleted
`settings.json` and produced `safety oracle OK`. The documented list is now parsed from `README.md`
and a documented name the engine does not protect is an error. It checked the matching *rule* and
never checked *membership*.

## Every finding

| # | Story | Sev | Finding | Repair |
| --- | --- | --- | --- | --- |
| 1 | S06 | **High** | `pytest` failed on an unmutated export, so its kills were artefacts. | Test skips without git history; control pass added; `check` fails when a control gate fails. |
| 2 | S06 | **High** | The SI-006 mutant was dead and the packet said PASS. | Re-anchored on the real folding site; confirmed to kill the SI-006 test. |
| 3 | S06 | Med | Coverage contradicted the Control section it sat under. | Coverage counts only kills by gates the control cleared. |
| 4 | S06 | Med | AC3 unimplemented, AC5 partial. | `RELEASE_GATES.md` classifies all 31 gates and `unclassified_gates()` checks it; the report names gates no mutant has ever made fail. |
| 5 | S06 | Low | `evaluate` short-circuited, so a declared second gate never ran; two dead `GATES` entries. | All declared gates run; dead entries removed. |
| 6 | S07 | Med | Rust is never checked. | Not repaired. AC2 remains PARTIAL and says so. |
| 7 | S07 | Med | Root-capability and retention compare the predicate to itself; the monotonicity branches were tautologies (`age > days+1` implies `age > days`). | Monotonicity rewritten over independently drawn parameters; the module docstring now states which predicate has an engine comparison and which two do not. |
| 8 | S07 | Med | The protected-name **set** came from the implementation. | Parsed from `README.md`; a documented name the engine does not protect fails. |
| 9 | S07 | Low | "2000 cases" claimed for all three predicates; only one ran 2000. | Per-predicate counts reported. |
| 10-11 | S08 | **High** | AC1 not implemented; AC2 unimplementable, because a prose classifier falls back to `ubiquitous` and can never fail to classify. | **Contract amended with the reason**, not marked PASS: a declared field means rewriting 357 criteria across 27 epics, most in closed contracts. Replaced by the substantive rule. |
| 12 | S08 | Med | Any incidental "if" satisfied the rule. | Pattern tightened to a leading condition or a "when X fails" form. The failing set rose from 28 to 82 - the worse number is the true one. |
| 13 | S08 | Med | `ENFORCED_AT` excluded CR2 though the AC said CR2+. | CR2 included. |
| 14 | S08 | Med | AC3 named `process_metrics.py`; the ratio lived elsewhere. | A "Requirements shape" section in the generated report, fed by `check_ears.py ratio` as data rather than by importing across two checkers. |
| 15 | S08 | Low | `planned` stories were skipped, so the gate never bound at authoring time. | `planned` is judged; only `cancelled` is skipped. |
| 16 | S02 | Med | `cmd_updates` never compared the pin. | `releases/latest` fetched; reports `current` / `behind` / `could not compare`. |
| 17 | S03 | Low | No usage record existed, so AC2/AC3 were unexercised. | Seeded and committed. |

## What the repairs found in turn

Four of E25's own stories failed the tightened EARS rule - **written in the same session as the
rule**. Their contracts were repaired rather than baselined: each gained the unwanted-behaviour
criterion it always needed, with the matching evidence row. That is the second time this session a
mechanism caught its author within the hour of being written, and the reason both are recorded
rather than quietly fixed.

## Claims corrected in the executor's own evidence

Five, all corrected in the packets. The outright false one was E25-S06's Safety Evidence row
certifying SI-006 from a mutant that did nothing. The rest were true of what was tested and did not
support the criterion as written - including an AC4 row claiming `git status --short` was clean
when it was not, since the harness does not verify cleanliness and the criterion requires it.

## Gates after repair

```text
python3 -m pytest tests -q                  -> 450 passed, 375 subtests
30 fast + slow gates (AGENTS.md check list) -> green
python3 scripts/gate_sensitivity.py check   -> 11 mutants, 11 killed, every gate clean on an unmutated tree
python3 scripts/safety_oracle.py check      -> the documented list, the engine's own decision
python3 scripts/check_ears.py check         -> 357 criteria, 9 unwanted (3%), 82 baselined
```

## Documents opened

`project/epics/E25.json`, `project/epics/E26.json`, `docs/security/SAFETY_INVARIANTS.md`,
`docs/security/HAZARD_ANALYSIS.md`, `docs/development/RELEASE_GATES.md`,
`docs/development/VERIFICATION_STRATEGY.md`, `docs/development/AGENT_PROTOCOL.md`,
`docs/audits/2026-09-12-METHODOLOGY_REVIEW.md`, `README.md`, `AGENTS.md`.

## Round verdict

**PASS_WITH_RESIDUALS.** Residuals carried forward: the Rust engine is still not checked against
the safety predicates (E25-S07, AC2), 82 contracts still describe only the happy path, 27 of 31
invariants still have no mutant, and the `Story:` trailer that would make risk attribution cover
more than 29 of 127 stories is still unwritten.
