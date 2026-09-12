# Evidence Packet - E25-S09

- Executor: Claude | Independent verifier: self-review | Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the control structure is drawn | `docs/security/HAZARD_ANALYSIS.md`, "Control structure": user, CLI/TUI, policy, safety kernel, platform seam, filesystem, with the feedback channels that populate each controller's process model. | PASS |
| AC2 - four UCAs per mutating control action | Five control actions (authorise delete, perform delete, authorise configuration write, report outcome, raise effective authority), twenty unsafe control actions, each typed *not provided / provided when unsafe / wrong timing or order / wrong duration or scope*. | PASS |
| AC3 - each UCA mapped to an invariant, and a gap recorded as a gap | Every UCA carries its constraining SI, except UCA-1.7 which is marked **Gap** and carried to G-1 with the reason it has no constraint. | PASS |
| AC4 - loss scenarios include process-model inconsistency | Five loss scenarios, all of the form "the controller acted on a belief that did not match the world" - the class a threat model structurally cannot reach. | PASS |
| AC5 - an unconstrained UCA is a gap, not an accepted risk | UCA-1.7 is marked **Gap** in the table and carried to G-1 with the reason it has no constraint; G-2 and G-3 record the two related gaps. None is written as accepted. | PASS |

## What it found

**G-1: no invariant constrains over-refusal.** Every invariant in the set pushes toward refusing,
which is the correct default for a destructive tool and is deliberate. The consequence is that one
failure mode is invisible to the whole set: a tool that refuses too readily is a tool the user
works around with `rm -rf`, and the loss then happens outside the system entirely. That is a real
hazard with no constraint and no gate, and finding it is what the analysis was for.

G-2 and G-3 are its cheaper relatives: nothing requires a withholding to be *actionable*, and
nothing requires a scan error to be visible in the summary a user actually reads.

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a (analysis) | Each of the seven E00 P0 defects should map to a UCA the analysis produces | Every one maps to UCA-1.1, 1.2, 1.4, 1.6 or 4.1, and LS-1 through LS-3 name the process-model inconsistency behind them. The analysis would have produced them. | PASS |

## Verification Commands

```text
python3 scripts/check_docs.py check -> the document is reachable and its links resolve
```

## Compatibility

- Prose. No code, no gate.

## Documentation updated

- `docs/security/HAZARD_ANALYSIS.md` (new), linked from `docs/INDEX.md`, with its relationship to
  `THREAT_MODEL.md` stated so the two do not duplicate or contradict.

## Residual risks

- **An analysis is only as good as the control structure it draws.** A controller nobody drew has
  no UCAs, and the Guardian and remote-control loops (E14, E15, E18) are represented by a single
  control action rather than analysed.
- **G-1 has no answer here.** How much refusal is too much is a product decision, not an
  engineering one, and inventing a threshold would be worse than naming the gap.
- **Nothing gates this.** The UCAs are prose next to prose. Each is a candidate mutant for
  `gate_sensitivity.py`, which is where an unconstrained UCA becomes a measured one.
- **The analysis was written by the same agent that wrote much of the code it analyses**, so its
  blind spots and the implementation's may be the same ones.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.
