# Evidence Packet - E06-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E06)
- Change Risk: CR4
- Spec version/commit: `docs/development/RELEASE_GATES.md` (new "Rust cutover gate status"
  section), `CHANGELOG.md`

## Outcome

PARTIAL - by design. See below for why this is the correct executor outcome for a CR4 gate
story, not an incomplete implementation.

## Scope

E06-S04's outcome is "promote Rust to stable only after functional, safety, compatibility, and
operability gates pass" - a **gate**, not a feature. Its acceptance criteria are:

1. "Owner-visible migration Safety Verdict is accepted."
2. "Python remains tagged/archiveable as reference for at least one transition window."
3. "Release notes state any intentional contract change."

AC1 is structurally not something an executor can satisfy. `docs/development/AGENT_PROTOCOL.md`
is explicit: "an executor's work is finished at `ready_for_review`... it does not set
`verification`/`done` for its own change, and it does not write its own Safety Verdict."
`docs/development/ENGINEERING_SYSTEM.md`'s "Ownership and transparency" section names the
Safety Verdict for CR4 as one of the artifacts "no agent is allowed to silently redefine... to
make implementation easier." Marking AC1 satisfied myself, or engineering the evidence to make
cutover look ready when the actual gate status does not support it, would be exactly that.

What an executor *can* do for a gate story - and what this change does - is produce the
concrete, evidence-backed checklist the owner and independent verifier need to make that
decision, and state plainly what it currently shows: **not ready**.
`docs/development/RELEASE_GATES.md`'s new "Rust cutover gate status (E06-S04)" section
enumerates G1 Functional, G2 Safety, G3 Compatibility, and G4 Operability against the real,
disclosed state of E06-S01/S02/S03's work (their own evidence packets are the source for every
claim in it - nothing new is asserted here that those packets do not already back). The
conclusion is explicit: cutover is not recommended at this time.

AC2 ("Python remains tagged/archiveable... for at least one transition window") is already
true and unaffected by this change - `cancellai.py` has not been touched, remains the shipping
Homebrew artifact, and nothing in E06-S01/S02/S03 modified or removed it. AC3 ("release notes
state any intentional contract change") does not apply yet - no cutover is being proposed, so
there is no contract change for release notes to state.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Owner-visible migration Safety Verdict is accepted | Not satisfied - not an executor decision. The gate checklist this change produces is the input the owner/verifier need to reach one. | NOT MET (by design - owner/verifier action required) |
| AC2 - Python remains tagged/archiveable as reference for at least one transition window | `cancellai.py`, `pyproject.toml`, `Formula/cancellai.rb` are unmodified by E06-S01 through this change; the existing Homebrew release process (`docs/RELEASING.md` "Current Python v1 release process") continues to tag/release it exactly as before. | PASS |
| AC3 - Release notes state any intentional contract change | Not applicable - no cutover/contract change is being made or proposed by this change. | N/A (no action required until a cutover is actually proposed) |

## Safety Evidence

Not applicable in the usual sense - this story makes no code change to the mutation boundary,
authority lattice, or any safety-relevant path. SI-019 (named in this story's safety
obligations) is unaffected: `execute_with_system_capabilities` (added at E06-S01) is the
existing, already-scanned single entry point; `scripts/check_mutation_boundary.py check` still
passes unchanged.

## Verification Commands

```text
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
python3 scripts/check_mutation_boundary.py check
```

All green (the last three unaffected by this change; re-run for completeness since this is the
last story before epic-scoped review).

## Compatibility

Not applicable - no code changed.

## Performance / operability

Not applicable - no code changed. The G4 gap analysis in `RELEASE_GATES.md`'s new section
*is* the operability finding this story produces: no packaged installer, no CLI-command
performance budget, no crash/recovery testing exists yet for the Rust command surface.

## Documentation updated

- `docs/development/RELEASE_GATES.md` (declared documentation impact) - new "Rust cutover gate
  status (E06-S04)" section: a living checklist, not a one-time snapshot - later work updates
  it in place as gaps close, rather than this story being rewritten.
- `CHANGELOG.md` (declared documentation impact) - a short, explicit "not ready, Python remains
  canonical" note, so the file does not read by omission as though cutover happened given how
  much E06 activity precedes it in the same Unreleased section.

## Residual risks

- The gate checklist is executor self-assessment of executor-produced work (E06-S01/S02/S03)
  - exactly the "verifier does not treat executor tests as proof" concern
  `AGENT_PROTOCOL.md` names. It is a starting point for independent review, not a substitute
  for it.
- Every gap the checklist names (see `RELEASE_GATES.md`) is real, disclosed backlog work: full
  Python CLI flag parity, companion-directory deletion, Codex-side incomplete-scan detection,
  confirmed tier-1 CI green (not merely structured to be OS-agnostic), a CLI performance
  budget, and Epic E17's packaged release factory. None of these are silently deferred - they
  are named explicitly so a future story picks them up deliberately.

## Verifier verdict

PENDING - epic E06 review runs once every story in E06 is `ready_for_review` (this is the
fourth and last). Per `docs/development/AGENT_PROTOCOL.md`, the verifier populates the CR4
Safety Verdict this story's own AC1 requires; this packet is not that verdict and does not
claim to be.
