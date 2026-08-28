# Evidence Packet - E00-S07 (Trust Floor closure)

- Story: E00-S07 - P0 regression evidence packet
- Risk: CR2
- Epic: E00 - Trust Floor Remediation
- Closed by: project owner
- Date: 2026-08-28

## Outcome

`PASS_WITH_RESIDUALS`. Every P0 defect from the 2026-08-27 baseline review is repaired and
carries a permanent regression test. No runtime feature beyond P0 remediation was introduced.
Closure is an owner decision rather than a fourth independent verdict; that is recorded
explicitly here and in each CR4 story's owner acceptance.

## P0 defects and where their regressions live

| Defect | Story | Regression |
| --- | --- | --- |
| P0-01 protected names documented but not enforced | E00-S01 | `TrustFloorTests` (injection, nesting, scanner emission), `ReviewResponseTests` (symlink, `..`), `RoundTwoResponseTests` (case, Unicode) |
| P0-02 config root accepted on depth alone | E00-S02 | `RootAuthorityTests` (every root shape), `RoundTwoResponseTests.test_every_custom_root_shape_is_refused` |
| P0-03 aggressive ignored the age cutoff | E00-S03 | `TrustFloorTests.test_aggressive_respects_cutoff_for_legacy_and_cache`, `test_aggressive_cutoff_boundary` |
| P0-04 non-subcommand flags normalized to `clean` | E00-S04 | `TrustFloorTests.test_flags_without_a_subcommand_never_normalize_to_clean` |
| P0-05 observation errors collapsed to zero/empty | E00-S05 | `ScanCompletenessTests`, `ReviewResponseTests`, `RoundTwoResponseTests.test_observe_separates_absent_from_unreadable` |
| P0-06 safety-blocked work returned success | E00-S04 | `TrustFloorTests.test_exit_code_distinguishes_blocked_from_success_and_usage` |
| P0-07 concurrent Claude history rewrite | E00-S06 | `TrustFloorTests.test_history_trim_abandons_rewrite_on_concurrent_write` |

Two further defects were found during remediation and given their own stories rather than
folded in silently: E00-S08 (coverage vocabulary overclaimed) and E00-S09 (an unusable
process observation was read as absence of activity).

## Review record

| Round | Reviewer | Outcome |
| --- | --- | --- |
| 1 | Codex | 6 of 7 rejected - each repair had closed the reported instance, not the class |
| 2 | Codex | 7 of 7 rejected - one class defect found in every story examined |
| 3 | Codex | 1 of 7 rejected - Unicode normalization in the protected-name barrier |
| 4 | none | closure directed by the owner |

The trend across rounds is the most useful artefact this epic produced, and it changed how
work is executed here: `docs/development/WORK_ITEM_MODEL.md` now defines `ready_for_review`
as the executor's exit state, and the control plane refuses a handoff without evidence and a
CR4 closure without a passing verdict.

## Gate results

| Gate | Result |
| --- | --- |
| G1 Functional | 108 tests, generated docs consistent, changelog covers user-visible behaviour |
| G2 Safety | Safety Invariants preserved; CR4 adversarial tests pass; four owner-accepted verdicts recorded with residuals |
| G3 Compatibility | macOS only, as claimed; Python 3.10 and 3.14 in CI; unknown provider layouts degrade to `unknown` |
| G4 Operability | no persistent state to migrate; refusals are visible and non-destructive; exit taxonomy documented for automation |

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py
python3 scripts/gen_docs.py --check && python3 scripts/project_os.py check
python3 scripts/check_docs.py check && python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
pre-commit run --all-files
```

## Known residual risks

Carried forward from the story-level records, and accepted by the owner:

- closure without a fourth review round, after three rounds that each found a class defect;
- custom provider roots are inspection-only until the Rust core has provider-native identity;
- scan completeness is per tool rather than per scope;
- protected-name matching is over-inclusive on case-sensitive volumes;
- process detection is best-effort on success, failing closed only on observation failure;
- `status` still traverses provider roots more than once, deferred to E04.

## Consequence for E01

The Python behaviour repaired here is the reference contract E01 freezes. The residuals above
are the list E01's differential fixtures must encode as *expected* behaviour, so the Rust
core is measured against what this build actually does rather than what it was hoped to do.
