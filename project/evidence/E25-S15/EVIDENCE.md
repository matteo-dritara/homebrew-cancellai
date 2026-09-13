# Evidence Packet - E25-S15

- Story: E25-S15 — Ambiguous history cannot erase a risk floor
- Risk: CR2
- Author: Codex, independent verifier repair
- Date: 2026-09-13

## Outcome

`check_risk_classification.py` no longer discards a multi-story historical commit from floor
evaluation. It preserves the honest absence of per-story ownership while enforcing the combined
commit floor against the lowest declared named story level. Historical commits that predate the
required `Story:` trailer are explicit reasoned baselines, and the E10 batch links to the CR4
review correction rather than silently passing.

## Verification

The focused unit tests cover failure at the lowest level, a visible reasoned baseline, and
refusal of a blank baseline reason. The full repository gate is rerun after the repair.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `AmbiguousCommitTests::test_a_multi_story_commit_must_meet_its_floor_at_the_lowest_named_level` fails the CR1/CR4 batch at CR4. | PASS |
| AC2 | `AmbiguousCommit` keeps only commit-level path facts; `evaluate_ambiguous` never assigns those paths to an individual story. | PASS |
| AC3 | The blank-reason test fails and `project/risk_floors.json` names every surfaced historical commit and its rationale. | PASS |

## Residual risk

The gate cannot reconstruct which named story owned an individual path in a pre-trailer batch.
Those commits remain explicit baselines with their identities and reasons; any new unbaselined
multi-story commit below its combined floor fails.
