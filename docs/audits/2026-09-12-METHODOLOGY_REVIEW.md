# Methodology review - 2026-09-12

This audit does to cEOS what cEOS does to the code: tries to break it.

The previous audits reviewed the implementation. This one reviews the thing that reviews the
implementation, against the practice of the industries that have been doing exactly this for
forty years - DO-178C and ARP4754A in avionics, ISO 26262 in automotive, IEC 61508 in industrial
control, NPR 7150.2 at NASA - plus the software-engineering measurement literature and the
published critiques of heavy assurance process.

**The finding in one sentence: cEOS has borrowed the artifact set of a safety standard without
the mechanism that makes it work, which in every one of those standards is independence applied
to the act of classification, and measurement applied to the process itself.**

That is not a verdict against the system. cEOS is more rigorous than almost anything on GitHub,
its instincts are repeatedly correct, and in two places it has independently invented techniques
this audit would otherwise have had to recommend. The findings below are about the gap between
what the system claims and what it can currently demonstrate.

Each finding gives: the claim cEOS makes, the question that falsifies it, the measurement in this
repository, and the change required. Measurements are reproducible from
[`project/generated/PROCESS_METRICS.md`](../../project/generated/PROCESS_METRICS.md) and the
commands shown.

---

## M-01 The Change Risk Level is assigned by the party whose work it governs — **critical, half repaired**

**The claim.** "The higher risk level determines verification depth even if the diff is tiny"
(`AGENTS.md`). CR0-CR4 selects the gate set, the need for adversarial tests, the need for
independent verification, and the need for a Safety Verdict.

**The falsifier.** *Who assigns the level, and what prevents them from being wrong in the
direction that reduces their own work?*

Every standard surveyed answers this the same way, and none of them answers it with "the
implementer decides":

- **DO-178C** does not contain a procedure for assigning the Software Level at all. The level is
  an *output* of the system safety assessment (ARP4761 FHA/PSSA/SSA, allocated under ARP4754A) and
  an *input* to software engineering. Level A carries 71 objectives, 30 of which must be met *with
  independence*, where independence means the verifier is not the author and that separation is a
  recorded artifact.
- **ISO 26262** determines ASIL in the HARA, and then - the decisive detail - subjects *the HARA
  itself* to a confirmation review at a graded independence level (I1/I2/I3). The classification
  is a reviewed work product. The standard does not trust it to be right because a competent
  person did it.
- **IEC 61508** scales required independence with SIL (person → department → organisation), and
  additionally raises it for *novelty*, *complexity*, and *lack of prior experience* - independent
  of the nominal level.
- **NPR 7150.2** is the most explicit: Software Assurance performs an **independent classification
  assessment**, the two classifications are **compared**, and a disagreement is a defined
  escalation event up two separate Technical Authority chains.

**The measurement here.** `change_risk` is validated in exactly one place,
`scripts/project_os.py:281-283`, and the validation is that the string is one of five permitted
values. Nothing derives a level from the change, nothing compares a second opinion, and no record
in `project/evidence/` shows a reviewer overturning a level upward. 48 of 113 stories are CR3 or
CR4 - the levels whose gates require independent verification - so the classification step is
load-bearing for nearly half the work.

**Why this is the critical finding.** Every other gate in the system is downstream of this one
field. Writing `"change_risk": "CR1"` where CR4 was correct removes, in one edit that passes every
check: the adversarial test requirement, the fault-injection requirement, the independent
verification requirement, the Safety Verdict, and the release-evidence obligation. It is the
cheapest possible bypass of the entire system, it requires no deception, and a plausible
misjudgement is indistinguishable from an honest one because nothing ever looks.

**Required change.** Two parts, and the second matters more than the first.

1. A **risk floor** derived mechanically from what the change touches: any diff reaching
   `cancellai-safety`, `cancellai-sealedfs`, `cancellai-platform`, `cancellai-model`, a
   protected-name list, root resolution, provider trust, or release signing is CR4 unless an
   explicit, recorded argument says otherwise. A declared level below the floor fails the gate.
2. A **second classification**, produced without seeing the first, and a recorded diff between
   them. NASA's mechanism, and the only one that catches a *plausible* misjudgement rather than an
   obvious one. **A disagreement rate of exactly zero over many stories is evidence that the second
   classification is not independent**, which makes the mechanism self-checking.

→ E25-S02.

---

## M-02 The evidence packet is prose that nothing validates — **high, repaired**

**The claim.** The evidence ledger is one of six owner-visible artifacts cEOS promises to keep
stable, and `project_os.py check` "refuses a `ready_for_review` story that has no committed
executor evidence."

**The falsifier.** *It refuses a story with no evidence. What does it do with a story whose
evidence is wrong?*

**The measurement here.** `scripts/check_process.py:170-187` checks that an evidence file names an
existing work-item ID, and - only for files named like a Safety Verdict - that certain section
headings are present. Nothing compares the packet against the story it claims to discharge.

The consequence is measurable, and the convention has already decayed without anyone noticing:
**69 of 84** story packets past `ready_for_review` carry a row per acceptance criterion. The
fifteen that do not include all five E20 stories - two of them **CR4**, the Windows native-identity
and volume-boundary work - and nine E00 stories. No gate reported this; this audit found it by
counting.

Worse than a missing row is a wrong one. The E24 review found **two acceptance-criteria rows that
claimed PASS on the strength of tests that could not have detected the defects present**: one
certified a rejection path that a mutation proved was not covered at all, the other certified
"every spelling is refused" from a suite that only ever fed one spelling. Both sat in
`ready_for_review` with all twenty-two gates green.

**Required change.** `scripts/check_evidence.py`: one row per acceptance criterion, quoted rather
than paraphrased; a non-empty residual-risk section for CR3 and above; a Safety Verdict present
for any CR4 story at `done`; and every command claimed in "Verification Commands" naming a target
that exists. Grandfather the pre-convention packets explicitly, with the reason recorded, the way
`REVIEW_ROUND_EXCEPTIONS` already does.

→ E25-S03.

---

## M-03 The process had never measured itself — **high, now repaired**

**The claim.** cEOS is an engineering operating system with gates, evidence and release authority.

**The falsifier.** *A process that has never caught a defect and a process with no defects to
catch produce identical evidence. Which one is this?*

**The measurement here.** Before this audit: none existed. Eighteen checkers measured the code;
zero measured the process. No defect-escape rate, no review yield, no rework rate, no record of
which gate has ever caught anything.

The first measurement, now generated by `scripts/process_metrics.py`, is the most important number
in this document:

> **47% of story verdicts in round-1 independent review are `FAIL`** - 32 of 68. Every one of those
> stories had been declared `ready_for_review` by an executor who ran the full gate set green.

The gates are not evidence of correctness. They are evidence that the executor did the work. That
distinction is the whole justification for the verifier role, and it is now a number rather than a
belief.

**Required change.** Done, and this document's numbers come from it. What remains is to use it: a
gate that has never caught anything is a candidate for deletion, and the burden is on the gate.

→ E25-S01 (done).

---

## M-04 The two-round review ceiling is a budget, not a stopping rule — **high, repaired**

**The claim.** ADR-0014 / PD-022 bound review to two rounds per epic; findings surviving round 2
become backlog items.

**The falsifier.** *What is the marginal yield of round 2? If the second round still finds as much
as the first, the ceiling is cutting while the review is productive.*

**The measurement here.**

| Epic | Round 1 yield | Round 2 yield |
| --- | --- | --- |
| E00 | 86% (6 of 7) | **100% (7 of 7)** |
| E06 | 75% (3 of 4) | 67% (2 of 3) |
| E20 | 100% (3 of 3) | 0% (1 judged) |

E00's second round rejected *more* than its first. E06's barely declined. E20's collapsed to zero.
The right number of rounds is plainly not a constant, and the constant is set at two.

Worse, E00's two rounds found **disjoint** sets of stories. Lincoln-Petersen is undefined on a
zero overlap, and that is the finding: two reviewers who share nothing have not bounded the defect
population, they have shown it is larger than either of them saw. E00 needed a third round, got
one by exception, and that round found more.

**Required change.** Replace the constant with a **yield-based stopping rule**: stop when a round's
rejection rate falls below a stated threshold, or when the Lincoln-Petersen residual estimate falls
below a stated bound - and require an owner decision, recorded, when it does not. Keep a hard
ceiling as a cost control, but make it visible when the ceiling rather than the evidence ended the
review. The measurement now exists to make this rule enforceable.

→ E25-S04.

---

## M-05 No gate has ever been shown to catch a planted violation — **high**

**The claim.** The gate set, the Safety Invariants and the adversarial-test requirement together
prevent a class of defect from reaching a release.

**The falsifier.** *Has anyone ever deliberately introduced a safety-invariant violation to see
whether the gates catch it? Which gate caught it, and how long did it take?*

**The measurement here.** Never, for the gate set as a whole - **but this repository already
invented the technique and applied it to one detector.** `scripts/rust_python_parity.py self-test`
injects a catalogue of known divergence classes (an extra delete candidate, a silently skipped
candidate, a withheld/not-withheld mismatch, a root-origin mismatch) and asserts the comparator
catches each one. `scripts/diff_harness.py check` does the same for the differential harness. This
is Mills' defect seeding, correctly implemented, in production, twice.

The E24 review demonstrated the same technique on a test suite rather than a detector: mutating
`SCRIPT_COMMAND` to match nothing left the entire suite green, proving that an assertion the
evidence packet cited as coverage was covering nothing. After repair, the same mutant fails.

The gap is not conceptual. It is that the idea was applied to two comparators and never
generalised to the thing that actually guards the product.

**Required change.** A **gate sensitivity harness**: for each Safety Invariant, a mutant that
violates it, applied to a scratch branch, run through the full gate set, with the result recorded -
which gate caught it, or that none did. The output is a sensitivity table per invariant, and an
invariant with no killing gate is an invariant that is asserted rather than verified. This is also
the answer to M-09.

→ E25-S06.

---

## M-06 The characterization oracle is circular — **medium**

**The claim.** `NORMATIVE` fixtures are the contract the Rust engine must match; an unexplained
divergence blocks cutover (migration gate M6).

**The falsifier.** *Was the expected output recorded from the implementation? If so, "the
implementation matches its characterization" proves the implementation matches itself.*

This is an **acceptability fallacy** in Greenwell's taxonomy of fallacies in system safety
arguments: a premise that is not independently credible because it is derived from the conclusion.
The fixtures are generated by `scripts/characterize.py generate` from `cancellai.py`'s observed
behaviour. Where that behaviour was wrong, the defect is in the oracle.

**In fairness, cEOS already saw half of this.** The `KNOWN_DEFECT` classification exists precisely
so a recorded behaviour can be marked "must not be reproduced", and a reclassification must update
the record and its rationale in the same change. That is a real and uncommon mitigation. What it
cannot do is find the defects nobody has noticed - those are recorded as `NORMATIVE` and become
requirements.

**The comparison worth making.** AWS's ShardStore work (SOSP'21) faced the same problem on 40,000
lines of Rust and answered it with an **independent executable reference model** - a separate,
deliberately simple implementation of the intended semantics, checked against the real one by
property-based testing and fuzzing. Not golden files recorded from the implementation: a second
statement of what the behaviour *should* be, written from the specification.

**Required change.** For the highest-risk behaviours only - root resolution, protected-name
enforcement, the age/keep-latest eligibility rule - state the rule as an independent predicate
derived from `docs/security/SAFETY_INVARIANTS.md` rather than from either implementation, and
check both engines against *it*. Everywhere else, keep the fixtures and be explicit in
`VERIFICATION_STRATEGY.md` that they are regression detectors, not correctness oracles.

→ E25-S07.

---

## M-07 Independence is claimed at a level the structure cannot supply — **medium, repaired**

**The claim.** "Executor / verifier separation", "independent adversarial verifier", "independent
verification" as a CR3/CR4 gate.

**The falsifier.** *Count the independent parties. Not roles - parties.*

**The measurement here.** One human owner, one executor model, one reviewer model. On IEC 61508's
ladder that is at most **"independent person"**, the SIL-1 rung - not independent department, not
independent organisation. It is genuinely better than same-model review, because Claude and Codex
are different model families with different failure modes, and the 47% round-1 rejection rate is
evidence that the separation does real work. But the word "independent" in this repository does not
mean what it means in the standards those gates were borrowed from.

And the recent trend runs the other way: of the last four epics, **three were self-reviewed** -
E09, E10 and E24 - with the owner waiving the independent round. E24's self-review found real
defects and its own record says on its first page that it is not independent, which is the correct
handling. It is still not the gate the contract names.

**Required change.** State the achievable independence level explicitly in `AGENT_PROTOCOL.md`,
using the standards' own vocabulary, and define what a **self-review** may and may not certify: it
may repair, it may not close a CR3/CR4 story, and it is never recorded under a filename that
implies independence. The naming rule is already the practice (`E24-SELF-REVIEW.md`); make it a
rule, because `scripts/process_metrics.py` now depends on it to classify rounds correctly.

→ E25-S05.

---

## M-08 Acceptance criteria are prose — **medium**

**The claim.** "AC are testable" is a Definition-of-Ready condition.

**The falsifier.** *Testable in principle, or machine-checkable? Name the CI check that fails if
this criterion is violated, and the commit or mutant at which it was observed to fail.*

**The measurement here.** Acceptance criteria are free English. Many are excellent; none is
classified, and nothing links a criterion to the check that discharges it. Some run to eighty
words and bundle three claims.

**The practice.** EARS (Mavin et al., RE'09) constrains requirements to five patterns -
**ubiquitous**, **state-driven** (`While`), **event-driven** (`When`), **optional** (`Where`), and
**unwanted behaviour** (`If/Then`). The value here is not tidiness; it is that the unwanted-behaviour
pattern forces failure triggers to be written as first-class requirements rather than left as
"error handling". For a tool whose defining risk is deleting the wrong thing, **the ratio of
`If/Then` criteria to happy-path criteria is a direct measure of whether the requirements describe
the feature or the hazard.**

**Required change.** Adopt EARS for acceptance criteria on CR2 and above, add the pattern as a
field in the story schema, and have `process_metrics.py` report the unwanted-behaviour ratio per
epic. Do not retrofit closed epics.

→ E25-S08.

---

## M-09 Most gates check that a step happened, not that a property holds — **medium**

**The claim.** Twenty-two commands constitute the verification set.

**The falsifier.** *Which gates check that a step was performed, and which check that a property
holds? A process audit verifies steps; the Nimrod safety case was fully compliant and the aircraft
was not safe.*

**The measurement here.** Of nineteen checkers, roughly a dozen assert structure, consistency or
existence - schema validity, link resolution, generated-document drift, banner presence, version
agreement, workflow pinning, evidence-file naming. Roughly six assert a behavioural property -
mutation-boundary exclusivity, provider-trust non-escalation, compatibility-matrix agreement,
characterization reproducibility, differential parity. Two self-test.

That mix is not wrong - structural checks are cheap and catch real drift, and this audit used
several of them. The finding is that **the ratio is invisible**, so nobody knows which half of the
gate set is doing the safety work, and a green run reads as equally meaningful either way.

**Required change.** Classify every gate in `RELEASE_GATES.md` as *structural* or *behavioural*,
and report, per gate, how many times it has failed and how many of those failures were real defects
rather than the gate needing adjustment. A gate whose failures are predominantly resolved by
modifying the gate has negative value: it trains the operator to route around it. Folds into the
sensitivity harness of M-05.

→ E25-S06.

---

## M-10 The hazard analysis presumes an adversary — **medium**

**The claim.** `docs/security/THREAT_MODEL.md` covers assets, actors, trust boundaries, misuse
cases and mitigations.

**The falsifier.** *Where is the loss that occurs when no component failed and no attacker
existed?*

**The measurement here.** The threat model is adversary-shaped, and good at that. STPA (Leveson)
addresses the other half: hazards that arise from *interactions* between components that each did
what they were told. Its characteristic finding is a controller acting on a stale or wrong
**process model** - its internal belief about the state of the thing it controls.

For this product that is not an abstract concern, it is the central hazard: cancellAI's process
model is its belief about which artifacts are protected, unknown, active or disposable. Every
serious defect in the E00 audit was a process-model defect - a protected name believed enforced
that was not, a root believed validated that was accepted on path depth, an unreadable directory
believed empty. No adversary, no failed component, and the tool deletes the wrong thing.

**Required change.** One STPA pass over the mutation control loop: draw the control structure,
enumerate the four Unsafe Control Actions for each mutating control action (not provided when
needed / provided when unsafe / wrong timing or order / wrong duration), and check each against the
existing Safety Invariants. Where a UCA has no invariant, that is a gap in the invariant set.

→ E25-S09.

---

## M-11 "Closing an epic cuts a release" collides with reality — **low-medium, repaired**

**The claim.** ADR-0014: closing an epic cuts a release, enforced by `scripts/release.py check`.

**The measurement here.** Encountered directly while closing E24. E24 changes no shipped artifact -
it is documentation, agent tooling and a test suite - so a release would be a version bump with
nothing in it. And v1.13.0 was prepared but not yet tagged, so `release.py check` correctly refused
a second in-flight release. The rule and the tooling disagreed, and the tooling was right.

The resolution taken was to leave the stories `done` and the epic `in_progress`, which passes every
gate and is honest, but is a state ADR-0014 does not describe.

**Required change.** Name the case: an epic whose stories are all `done` but whose closure is
blocked by the release train, or which changes nothing shippable, is a legitimate state and should
be visible in `project_os status` rather than indistinguishable from work in progress. Either an
epic class that closes without cutting a release, or an explicit "awaiting release train" status.

→ E25-S10.

---

## M-12 Governance outweighs the product — **low**

**The claim, implicitly.** The documentation set is proportionate to the risk.

**The falsifier.** Haddon-Cave's proportionality principle, and his list of what a safety case
degenerates into: bureaucratic length, obscure language, archaeological documentary exercises,
audits of process only, prior assumptions of safety, decorative shelf-ware. *Which documents has
nobody read since they were written?*

**The measurement here.**

| | Lines |
| --- | --- |
| Governance prose (`docs/` + `project/`) | 33,643 |
| Shipping Rust | 29,637 |
| Governance tooling (`scripts/`) | 6,102 |
| Python reference | 2,146 |

**1.14 lines of governance prose per line of shipping code.** For a destructive local tool that is
defensible; there is no threshold in the literature that says otherwise. What is missing is any
evidence of *readership*: no document records when it was last consulted, and 260 Markdown files
are reachable in the documentation graph.

There is also a second-order risk specific to this project. Haddon-Cave identifies **outsourcing**
of the safety case as causal in the Nimrod failure. An AI-generated engineering contract, reviewed
by an AI, is the limiting case of outsourcing. The owner is the only party who is harmed if it is
wrong, and the only one who can make it "home-grown" in Haddon-Cave's sense.

**Required change.** Nothing urgent, and deliberately no restructuring: a docs taxonomy (Diátaxis
or otherwise) has no empirical support and would be churn. Instead, two cheap things - record which
documents a review actually opened, in the review record; and once a phase, list the documents no
review has opened and ask the owner whether they should exist.

→ E25-S11.

---

## What cEOS already gets right, and should not lose

A falsification exercise that lists only failures misrepresents its subject. Five things here are
better than standard practice and two are better than the standards:

1. **Risk proportional to authority rather than to diff size.** "The higher risk level determines
   verification depth even if the diff is tiny" is the correct principle, stated plainly. Most
   projects gate on lines changed.
2. **Done and Safe as separate questions.** `Done=YES, Safe=NO` being explicitly non-releasable is
   exactly the distinction DO-178C draws between functional completion and certification
   eligibility, and almost nobody outside regulated industry states it.
3. **Unknown is protected; ambiguity never escalates privilege.** Constitutional, not advisory. This
   is a stronger formulation than most safety standards manage, because it is a rule about the
   *default direction of error* rather than a list of prohibited states.
4. **Seeding applied to detectors.** `rust_python_parity.py self-test` and `diff_harness.py check`
   independently reinvent Mills' error seeding to prove a comparator can fail. Generalising it
   (M-05) is the highest-value change in this document, and the pattern to generalise is already
   in-tree.
5. **The E00 review history, committed.** Three rounds, each finding that the previous repair closed
   the reported case and left the class open - preserved rather than tidied away. That record is
   worth more than any document in the repository, because it is the only artifact here that
   contains a *failed* attempt to be right, which is what Leveson argues a safety case almost never
   does.

---

## Priority

| Order | Finding | State |
| --- | --- | --- |
| 1 | M-01 risk classification | **Half repaired** (E25-S02). The mechanical floor exists and is gated: `project/risk_floors.json` plus `scripts/check_risk_classification.py`. It found five stories already below their floor, one of them - E20-S04, declared CR2 while touching the only crate exempt from `forbid(unsafe_code)` - exactly the case this finding predicted. The second half, an independent second classification, is structurally supported and **has never been produced**, so the strongest part of the standards' mechanism is present in form only. |
| 2 | M-05 gate sensitivity | **Open** (E25-S06), and now the highest-priority open finding. Until a planted invariant violation has been shown to be caught, nothing the gates claim is tested. |
| 3 | M-02 evidence validation | **Repaired** (E25-S03). `scripts/check_evidence.py` requires a row per criterion, a real residual-risk section at CR3+, a Safety Verdict for a CR4 story at `done`, and that claimed commands exist. Fifteen pre-convention packets are a printed baseline that can only shrink. |
| 4 | M-04 stopping rule | **Repaired** (E25-S04, ADR-0025). Review continues while a round rejects 10% or more, escalates on zero overlap, and treats three rounds as a cost control whose use is an owner decision. |
| 5 | M-07 independence honesty | **Repaired** (E25-S05). `AGENT_PROTOCOL.md` states the achievable rung in the standards' vocabulary and defines what a self-review may and may not certify. |
| — | M-11 epic with nothing shippable | **Repaired** (E25-S10, ADR-0025). `done_no_release` names the state; it is an epic status only. |
| — | M-03 process measurement | **Repaired** (E25-S01). |
| 6+ | M-06, M-08, M-09, M-12 | Open, real, none urgent. E25-S07, S08, S09, S11. |

## What the repairs found

Two things worth recording, because both are the mechanisms working on their authors:

**The risk floor caught its own commit.** One commit after being written, it flagged E25-S04 for
touching `scripts/release.py`, which E25-S04 never touched. This repository writes several stories
in one subject as `E25-S04/S05/S10`, and a story-id regex finds exactly one id in that string - so
a three-story commit read as *unambiguous* and every file in it was attributed to the first story.
That is precisely the confident wrong attribution the design refuses to make, arriving through a
commit convention nobody had considered.

**The evidence gate found a decayed convention, not a hypothetical one.** The fifteen packets were
not a risk; they were already there, two of them CR4, and no gate had ever said so.

## Method and limits

Written by the same agent that is the standing executor of this repository, using two isolated
agent contexts for the E24 review it draws on and a separate research pass over the standards
literature. **This is a self-audit and carries the weaker evidentiary weight that implies** - the
same caveat M-07 raises about self-review applies to this document. Every measurement is
reproducible from committed artifacts; every recommendation is falsifiable by running the change
and measuring whether the numbers move. Standards clause numbers are from secondary sources, since
the standards themselves are paywalled; verify before citing normatively.
