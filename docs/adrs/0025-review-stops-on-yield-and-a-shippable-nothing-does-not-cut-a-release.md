# ADR-0025: Review stops on measured yield, and an epic that changes nothing shippable does not cut a release

- Status: Accepted
- Date: 2026-09-12
- Owners: project owner / cEOS
- Related: ADR-0014, PD-021, PD-022, C-16, E25-S04, E25-S10

## Context

ADR-0014 decided two things about cadence and was right about both problems. It was decided
without any measurement of the process, because none existed. E25-S01 built the measurement, and
two of ADR-0014's answers now look different in the light of it.

**The two-round ceiling was chosen without yield data.** ADR-0014 states the cost honestly - "a
second round can miss what a third would have caught" - and accepts it because the alternative was
an unbounded process. The measurement now available says the right number of rounds is not a
constant:

| Epic | Round 1 yield | Round 2 yield |
| --- | --- | --- |
| E00 | 86% (6 of 7) | **100% (7 of 7)** |
| E06 | 75% (3 of 4) | 67% (3 judged, 2 rejected) |
| E20 | 100% (3 of 3) | 0% (1 judged) |

E00's second round rejected *more* than its first, and E00's two rounds found **disjoint** sets of
stories, which makes the Lincoln-Petersen residual estimate undefined: two reviewers who share no
findings have not bounded the defect population, they have shown it is larger than either saw.
E00 needed a third round, got one by owner exception, and that round found more. E20's second round
found nothing, and running it was pure cost. A fixed ceiling of two is wrong in both directions,
and it was wrong in a way nobody could see until the yield was counted.

**"Closing an epic cuts a release" has no case for an epic with nothing to ship.** E24 - the agent
skill pack, a hook, and a prose rule - changes no byte of the artifact a user installs. Closing it
would have cut a version with nothing in it. Worse, `v1.13.0` was prepared and not yet tagged, and
`scripts/release.py check` correctly refused a second in-flight release. The rule and the tooling
disagreed and the tooling was right. The state taken was stories `done`, epic `in_progress`, which
passes every gate, is honest, and is a state ADR-0014 does not describe. Leaving it as folklore is
how a rule quietly stops meaning what it says.

## Decision

### Review stops on yield, and the ceiling becomes a visible cost control

A round's **yield** is the fraction of judged stories it rejects. After each round:

- yield **at or above 10%** - another round is required;
- yield **below 10%** - review may close;
- **zero overlap between two rounds' findings** - the population is unbounded; escalate to the
  owner rather than closing, whatever the yield.

The ceiling from ADR-0014 rises to **three** rounds and changes meaning. It is no longer the
stopping rule; it is a cost control. A review that ends because the ceiling was reached rather than
because the yield allowed it is recorded as such in the review record, and it is an **owner
decision** to close there rather than an automatic close. Findings that survive still become
backlog items with the story id that carries them, exactly as ADR-0014 decided.

`scripts/process_metrics.py` computes the yield and the overlap, so the rule is evaluable rather
than asserted.

### An epic that changes nothing shippable does not cut a release

An epic whose stories are all `done` may hold one of two terminal states:

- **`done`** - it changed the artifact users install, and it cuts a release, as ADR-0014 decided.
- **`done_no_release`** - every story is complete and nothing in the shipped artifact changed. No
  version, no tag, no release evidence. The epic is closed and the state says why.

An epic whose stories are all `done` but whose release is blocked by an in-flight release train
stays `in_progress` and is reported as awaiting the release train, so it is distinguishable from
work that is not finished.

The rule that a *product* epic must cut a release is unchanged, and
`scripts/release.py check` continues to refuse two in-flight releases. That refusal is the
behaviour that surfaced this gap; it is not weakened to accommodate it.

## Alternatives considered

### Leave the ceiling at two and accept the residual

What ADR-0014 decided, and defensible while no yield data existed. It is no longer defensible for
E00-shaped epics specifically, where the data says the second round was still finding defects at
undiminished rate. Keeping a constant that the measurement contradicts would be the first step of
normalisation of deviance: the standard compared against becomes the previous iteration rather than
the requirement.

### Make review unbounded and stop on a clean round

Rejected for the reason ADR-0014 gives: it produced real findings and no completion criterion, and
the third E00 round was stopped by the owner rather than by the process. The yield rule keeps a
criterion the process can evaluate while allowing the number of rounds to follow the evidence.

### Let an epic with nothing shippable cut a release anyway

Semantically empty version bumps train everyone, including the tooling, to treat a version as
bookkeeping. SemVer then stops carrying information about the artifact, which is the one thing it
is for.

### A separate "internal" epic class that never releases

Rejected as too coarse. An epic is not internal or external by nature; a single epic can contain
both. The test is what the change did to the shipped artifact, and that is a per-epic fact recorded
at closure rather than a category chosen in advance.

## Consequences

### Positive

- The number of review rounds follows the evidence, so an epic that is still producing findings is
  not closed by a constant, and one that is producing none is not reviewed again out of ritual.
- A zero-overlap result stops reading as "both rounds passed", which is the exact inversion of what
  it means.
- A closed epic that shipped nothing is visible as such, and the release history stops carrying
  empty versions.
- The state of an epic waiting on the release train is distinguishable from unfinished work.

### Negative / cost

- A third round is now possible, so an epic can take longer. This is the cost ADR-0014 paid in the
  other direction, moved to where the measurement says it belongs.
- The yield threshold of 10% is a judgement, not a derivation. It is recorded here so it can be
  argued with and revised against data, rather than being embedded in a tool.
- Two terminal epic states are more to hold in mind than one.

### Neutral / follow-up

- The threshold should be revisited once enough rounds exist to fit it rather than to assert it.
- `scripts/project_os.py` and `scripts/release.py` need to understand `done_no_release`; E25-S10
  carries that work.

## Safety and compatibility impact

- Change Risk implication: CR0. This changes process, not product behaviour, and no gate is
  weakened - the release checker's refusal of two in-flight releases is explicitly preserved.
- Safety Invariants affected: none directly. The indirect effect is on CR3/CR4 stories, whose
  independent-verification gate now ends on evidence rather than on a count.
- Migration/rollback: no artifact changes. Reverting means restoring the constant ceiling and
  removing one epic state; no data migration exists to undo.

## Supersession

ADR-0014 is **not** superseded. Its decision that closing an epic cuts a release, and that review
is bounded rather than unbounded, both stand. This ADR amends the stopping rule and names the case
ADR-0014 did not have. If replaced later, keep this ADR and mark it superseded by ADR-XXXX.
