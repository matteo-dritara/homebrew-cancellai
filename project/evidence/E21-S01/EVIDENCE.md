# Evidence Packet - E21-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR0
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`, findings `CR-TE-01`,
  `CR-TE-05`, `CR-TE-07`, `CR-TE-10`, `CR-TE-12`

## Outcome

PASS

## Scope

This story changes no runtime behaviour. It corrects every place found by the 2026-09-03
target-engine review where the repository states, or implies by omission, a stronger guarantee
than the target engine provides. It runs **first** in E21 deliberately: the repairs that follow
take several stories, and the record should not read as though the engine were sound while they
are in flight.

One documented claim is corrected inside a Rust source file
(`cancellai-platform/src/mutation.rs`). That is a module-doc edit only - no expression, type or
control flow was touched, which is why this remains CR0 despite the file's CR4 subject matter.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - `RELEASE_GATES.md` moves the Codex incomplete-scan gap from G1 to G2 and adds the Claude project-directory case | The G1 paragraph no longer lists incomplete-scan detection and states explicitly why it moved. G2 is rewritten to lead with the reproduced defect, names both providers, and states that E06-S02 repaired only the Claude *companion payload* branch while an unreadable **project** directory still passes silently - a case no document previously disclosed. G4 gains the `CR-TE-02` benchmark-target and `CR-TE-04` memory findings; the conclusion now states the reason is no longer only packaging and platform coverage. | PASS |
| AC2 - `mutation.rs` no longer justifies the unlink residual with a superseded premise | The claim that no reviewed FFI dependency exists is removed. A new section records that `cancellai-sealedfs` (ADR-0017) supersedes it, that adding `unlinkat` is inside that crate's mandate, and that the current risk ordering is inverted - configuration writes are handle-protected, irreversible deletion is not. It states plainly that this is a disclosed residual, not an unavailable capability, and names `E21-S07`. | PASS |
| AC3 - `CLI_RUST.md` Known gaps names the absent help surface, the unwired native delete backend, and the non-counting `error_count` | Four entries added at the head of the section: no `--help`/`-h`/`--version` (with the `main.rs` hand-rolled parser as cause and `E22-S03`/ADR-0019 as resolution); the unwired Codex native delete backend, stated as a behavioural divergence on user data rather than a missing flag; the completeness defect itself, marked as the current blocking cutover defect; and `error_count` computed as `u32::from(!scan_complete)`. | PASS |
| AC4 - the control plane declares the phase the project is actually in | `project/roadmap.json` `current_phase` corrected `P0` -> `P1`. `PROJECT_STATUS.md` regenerated: "Current phase: **P1**". Both P0 epics were `done` and P1 stood one epic from closing, so the previous value was false at generation time. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a (CR0) | - | No runtime behaviour is changed by this story. The invariants the corrected text concerns - SI-008, SI-009, SI-010, SI-013 - remain violated/residual exactly as before; this story makes that visible, it does not repair it. | n/a |

Stating this explicitly because a documentation story that touches safety text could otherwise
be mistaken for a safety fix. It is not. `E21-S03` and `E21-S07` carry the repairs.

## Verification Commands

```text
python3 scripts/project_os.py check                  OK
python3 scripts/check_docs.py check                  OK
python3 scripts/check_process.py check               OK
python3 scripts/check_workflows.py check             OK
python3 scripts/check_fixtures.py check              OK
python3 scripts/check_schemas.py check               OK
python3 scripts/characterize.py check                OK
python3 scripts/diff_harness.py check                OK
python3 scripts/check_rust_workspace.py check        OK
python3 scripts/check_mutation_boundary.py check     OK
python3 scripts/check_provider_compatibility.py check OK
python3 scripts/release.py check                     OK
python3 scripts/gen_docs.py --check                  OK
python3 -m pytest tests -q                           179 passed, 22 subtests
python3 -m ruff check . / ruff format --check .       clean, 189 files formatted
cargo fmt --check / clippy -D warnings / test         clean, 298 passed
python3 scripts/rust_python_parity.py check           10 NORMATIVE fixtures, both scenarios
```

`check_mutation_boundary.py` is called out specifically: it scans
`cancellai-platform/src/mutation.rs`, the file this story edits, and still finds exactly one
production file permitted to call a removal primitive.

## Compatibility

- No schema, CLI surface, exit code, or provider behaviour changed.
- `docs/CLI.md` remains generated from the frozen Python reference and is unaffected.

## Performance / operability

- No runtime path changed. The performance claims corrected in G4 describe measurements taken
  during the review (`CR-TE-02`, `CR-TE-04`); `E21-S05` and `E21-S06` act on them.

## Documentation updated

- `docs/development/RELEASE_GATES.md` - G1/G2/G4 and the cutover conclusion.
- `docs/CLI_RUST.md` - four gaps added to "Known gaps".
- `rust/crates/cancellai-platform/src/mutation.rs` - module docs only.
- `project/roadmap.json` - `current_phase`.
- Registered alongside this story, outside its own AC: `docs/audits/2026-09-03-CODE_REVIEW.md`,
  `docs/INDEX.md`, ADR-0018, ADR-0019, `project/epics/E21.json`, `project/epics/E22.json`,
  `project/epics/E06.json` (E06-S04 blockers), `CHANGELOG.md`.

## Residual risks

- Every defect this story discloses remains present in the engine. That is the intended state
  at the end of this story, not an oversight: `E21-S02` must produce fixtures that **fail**
  against the current engine before `E21-S03` repairs it.
- The disclosure is as complete as the review that produced it. A gap the review did not find
  is still undisclosed, and this story cannot claim otherwise.


## Round-1 independent review: FAIL, and its repair

`project/evidence/E21-VERIFIER-REVIEW.md` failed this story: the disclosures were present, but
by the time they were written they claimed the G2 blocking defect was repaired, and the verifier
reproduced it still open (see E21-S03). A disclosure that overstates the implementation is the
one failure mode this story exists to prevent, so the verdict is correct.

Repaired by fixing what the disclosure describes rather than by weakening the disclosure: the
Claude root escape is closed (E21-S03), and every claim in `RELEASE_GATES.md` G2/G4 and
`CLI_RUST.md` has been re-checked against a native reproduction rather than against the previous
prose. The one text change in this round is G4's performance paragraph, which now describes the
retargeted scheduled benchmark instead of the per-PR test alone.

## Verifier verdict

`FAIL` (round 1) - repaired above; owner-accepted closure without a round 2, see project/evidence/E21-CLOSURE.md
