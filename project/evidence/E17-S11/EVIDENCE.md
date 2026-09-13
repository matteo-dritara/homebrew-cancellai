# Evidence Packet - E17-S11

- Commit/PR: the toolchain report fix on `main`
- Executor: Claude
- Independent verifier: none - the owner waived further review for this stretch. The finding is
  Codex's, reproduced here before repair (`project/evidence/E17-S08-VERIFIER-REVIEW.md`, F5).
- Change Risk: CR1
- Spec version/commit: `project/epics/E17.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - members count as managed | Reproduced first: `check_agent_toolchain.py report` listed all eight approved skills under "Installed but unmanaged - decide or remove". `render_report` now subtracts declared `members` as well as top-level ids, which is what `check` already did; the section is gone. | PASS |
| AC2 - an undeclared skill is still unmanaged in both | `test_an_undeclared_member_is_still_reported` adds a third skill to the installed set and asserts it appears while the two declared members do not. The enforcement path is untouched, so `check` behaves exactly as before. | PASS |
| AC3 - report and check agree on the committed manifest | `test_report_and_check_agree_on_the_real_repository` asserts both against the repository itself rather than a fixture: `check` returns 0, so the report must find nothing unmanaged. A fixture-only test would have passed while the real manifest failed, which is how this shipped. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The fix loosening the approval boundary | It cannot: only `render_report` changed. The gate that refuses an unmanaged component (`check`) is untouched, and its own tests - including the enumerator's coverage of every component kind - pass unchanged. | PASS |
| n/a | A false positive in the report the owner reads at session start | That is the defect. E26-S04 made the session start with this report; a report that asks the owner to re-decide eight components they already approved is one nobody finishes reading, and the real unmanaged component would sit in the same list. | Corrected |

## Verification Commands

```text
python3 scripts/check_agent_toolchain.py report   -> no unmanaged section; 11 components listed
python3 scripts/check_agent_toolchain.py check    -> 11 managed, 9 present, nothing unmanaged
python3 -m pytest tests/test_agent_toolchain.py   -> 32 passed, 21 subtests
python3 -m mypy scripts/check_agent_toolchain.py  -> clean
```

## Compatibility

- Reporting only. No manifest schema change, no change to what is approved or enforced.

## Performance / operability

- Not applicable.

## Residual risks

- **`report` and `check` still compute their answer separately.** They agree now, and one test
  pins that agreement on the committed manifest, but nothing forces them to share the computation.
  A third caller would be free to get it wrong again.
- **The report is advisory.** Nothing consumes it programmatically; its only reader is whoever
  runs the session-start ritual.

## Verifier verdict

closed on the owner's waiver; no independent verdict. The finding was Codex's.
