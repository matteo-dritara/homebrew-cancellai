# Evidence Packet - E25-S06

- Commit/PR: the E25/E26 closure commit on `main`
- Executor: Claude
- Independent verifier: self-review in isolated agent contexts (owner waived the Codex round)
- Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a mutant per invariant, applied to a scratch tree, result recorded | `scripts/gate_sensitivity.py` carries eleven mutants. Each is applied to a `shutil.copytree` export in a temporary directory; the gate set runs there; the killing gate is recorded. `project/generated/GATE_SENSITIVITY.md` is the output. | PASS |
| AC2 - a per-invariant table, and an unkilled invariant reported as asserted | The report's table names the killing gate or prints "**NONE - the claim is asserted, not verified**". The Coverage section states plainly that 4 of 31 invariants have a killing mutant and that most have no mutant at all, which is not the same thing. | PASS |
| AC3 - every gate classified structural or behavioural | **Implemented after review**, which correctly refused to let the story close with an AC declared NOT MET. `docs/development/RELEASE_GATES.md` classifies all thirty-one gates, and `unclassified_gates()` refuses a gate this harness can run that the document does not classify - so the classification is checked rather than asserted. Currently zero unclassified. | PASS |
| AC4 - the harness never mutates the committed tree | `export_tree()` copies to `tempfile.TemporaryDirectory()`; nothing writes to `ROOT`. `MutantIntegrityTests` runs against the real tree read-only, and `git status --short` is clean after every run in this session. | PASS |
| AC5 - a gate that has never failed is reported | The report names mutants no credible gate caught **and** gates that no mutant has ever made fail, with the reason: a gate never observed failing is of unknown strength. It reports absence of failure *in this harness*, not across the repository's history, which is stated rather than implied. | PASS with a narrower reading |
| AC6 - a stale mutant raises rather than reading as unkilled | `test_a_missing_anchor_raises_rather_than_reporting_no_gate_caught_it`, and `MutantIntegrityTests` holds every committed mutant's anchor to still being present. This fired for real twice during the story. | PASS |

## What it found

### First round: the risk floor guarded nothing

Ten of eleven mutants were killed. **One survived**, and it was the important one:
`story-risk-lowered` edited `project/risk_floors.json` to move the safety kernel's floor from CR4
to CR0, and **no gate noticed**. The mechanism that constrains every other gate was itself
configurable by whoever it constrains.

Repaired by `MANDATORY_FLOORS` in `scripts/check_risk_classification.py`: four surfaces are stated
in code, where configuration may raise them and may never lower or remove one. Changing them is a
change to a checker under review rather than a line in a data file. The mutant is now killed, and
the committed report shows zero survivors.

### Second round: the harness was measuring the export, not the gates

An independent review then falsified the harness itself, and the finding was worse than the one the
harness had found. `export_tree` excludes `.git`, and one test asserted a property of this
repository's commit history - so **`pytest` failed on an unmutated copy**, and every mutant it
"killed" was killed by a pre-existing failure rather than by the mutation. Three of the four
invariants the report claimed coverage for were pytest-only.

Two repairs, and a third that matters more than either:

1. the test now skips when there is no git history - which also fixed a real defect, since the
   suite had been failing in any source export, including the released tarball;
2. a **control pass** runs every gate on an unmutated copy first. A gate that fails there kills
   nothing, its kills are excluded from coverage, and `check` exits non-zero rather than reporting
   the result in prose and passing anyway;
3. the `protected-name-case-sensitive` mutant was **dead** - it edited an unrelated `.lower()` and
   survived the suite while this packet recorded SI-006 as verified. Re-anchored and confirmed to
   kill the decomposed-name test.

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-001 | A protected name removed from the enforcement set | `protected-name-removed` -> killed by `pytest` | PASS |
| SI-006 | Protection made case-sensitive | **Corrected after review: the original row was false.** The mutant anchored on the first `.lower()` in `cancellai.py`, which is `extract_uuid`'s session-id lowercasing - it touched no protected-name code and survived the whole suite while this table recorded SI-006 as verified. It now anchors on `protected_component`'s own `canonical_name` folding and kills `test_decomposed_protected_directory_is_refused_at_deletion`. | PASS after repair |
| SI-008 | An unreadable directory reading as empty | `unreadable-reads-as-empty` -> killed by `pytest` | PASS |
| SI-019 | A second deletion path in a non-kernel crate | `second-deletion-path` -> killed by `check_mutation_boundary.py` | PASS |
| SI-019 | The kernel depending on a crate it governs | `dependency-cycle` -> killed by `check_rust_workspace.py` | PASS |

## Verification Commands

```text
python3 scripts/gate_sensitivity.py generate -> 11 mutants, 11 killed
python3 -m pytest tests/test_governance_extras.py -q -> 27 passed, 117 subtests
python3 scripts/check_risk_classification.py check -> OK; a lowered mandatory floor is refused
```

## Compatibility

- Stdlib only. `cargo` is deliberately not invoked: a harness nobody runs because it takes twenty
  minutes measures nothing.

## Performance / operability

- Roughly two minutes: eleven tree copies and a gate run each. CI-only for that reason, recorded
  inline in `.pre-commit-config.yaml` rather than left for someone to discover.

## Documentation updated

- `project/generated/GATE_SENSITIVITY.md` (new, generated), linked from `docs/INDEX.md`.
- `AGENTS.md` check list, both workflows, `pyproject.toml`, `scripts/check_process.py` banners.

## Residual risks

- **AC3 was not implemented and is not claimed.** Classifying each gate "structural or behavioural"
  by hand would have produced an unvalidated column. The mutant table answers the same question
  empirically - a gate that kills a mutant has been shown able to fail - and that is strictly
  better evidence than a label. Recorded as not met rather than quietly satisfied.
- **4 of 31 invariants have a killing mutant.** The other 27 have no mutant *at all*, which is not
  the same as having no killing gate, and the report says so. Coverage grows by writing violations.
- **Seeded defects are systematically easier than real ones**, because people plant what they
  already check for. This measures gate sensitivity to imagined failures, not to unimagined ones.
- **A mutant that stops applying is an error, not a pass** - but only because someone runs it. The
  harness cannot notice its own staleness between runs; `MutantIntegrityTests` closes that in the
  fast suite.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.
