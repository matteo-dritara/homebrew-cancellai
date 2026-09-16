# E12 Independent Verifier Review — Round 5

Review-Scope: epic
Round: 5
Review target: uncommitted working-tree repairs on `82d30c1..f8dc4cc` (main), including rounds 1–4 repairs
Verifier: Codex
Date: 2026-09-16

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E12-S01 | FAIL | Round 4's exact old-final-witness/new-partial-record case is closed independently: the stale record was discarded and the legitimate final record survived. But recovery does not itself enforce witness-last: when a matching group has a record whose final name is a directory, recovery records that record's finalization error, still finalizes its witness, and the next recovery discards the remaining pending record because its own proof is no longer pending. This leaves the completed artifact without its restore metadata. Brief-Checksum: `eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734`. |
| E12-S02 | PASS_WITH_RESIDUALS | Restore continues to use an atomic no-replace rename on supported Unix targets and refuses wherever that primitive is unavailable. Its direct SI-013 property passes, but E12-S01's rejected metadata lifecycle remains its prerequisite, so it is recorded blocked rather than done. Brief-Checksum: `012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0`. |
| E12-S03 | FAIL | Archive shares the same group-recovery loop. A failed record, length, or digest finalization can be followed by successful witness finalization; a retry then discards the unresolved legitimate archive metadata. SHA-256 integrity and the old-final-witness fix remain closed, but AC3/SI-020 are not. Brief-Checksum: `a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b`. |

## Failures and required repairs

### E12-S01

Reproduction: an independent `/private/tmp` Rust executable created a real artifact, a matching
pending `move-identity` witness, and a pending quarantine record using the production
`SealedRoot` writer. It made the record's final-name target a directory, so recovery could not
rename the record. The first `recover_pending_moves` produced `Error("could not finalize: Is a
directory")` for the record and `Finalized` for the witness. A second call produced `Discarded`
for the still-pending record. The artifact and final witness remained, but no final restore record
existed. The same sequence is reachable on an ordinary failed finalization or process death after
witness finalization: `read_dir` provides no ordering guarantee and the loop continues after a
sibling error.

Required repair: make recovery an explicit group state machine. Process every non-witness
sidecar first; finalize the witness only after all of them succeed; on any error, retain the
pending witness and unresolved sidecars for a retry. Add direct regressions for each recovery
failure/crash boundary, independent of `commit_move_with_sidecars`'s normal ordering. This is a
further narrowing of the same recovery-state hazard found in rounds 3 and 4, not a new unrelated
defect. It violates E12-S01 AC3 and SI-020.

### E12-S03

Reproduction: static inspection and the shared production function establish that Archive's
record, length, digest, and witness flow through exactly the same recovery group and error loop.
Any one of the three ordinary archive sidecars can follow the E12-S01 sequence.

Required repair: apply the same group state machine to all three Archive payload sidecars and add
failure/crash-boundary regressions for record, length, and digest. This is the shared continuation
of the rounds-3/4 recovery hazard, violating E12-S03 AC3 and SI-020.

## Gate status

| Command(s) | Result |
| --- | --- |
| Independent round-4 scenario | PASS — production recovery discarded a stale pending record that lacked its own pending witness and preserved `legitimate record`. |
| Independent failed-recovery scenario | FAIL — witness finalized after its sibling error; a retry discarded the legitimate pending record. |
| `python3 -m pytest tests -v` | PASS — 665 passed, 546 subtests passed. |
| `python3 -m ruff check .`; `python3 -m ruff format --check .` | NOT RUNNABLE — Ruff is not installed (`No module named ruff`). |
| Full AGENTS.md mypy target list, including `scripts/check_process.py` | NOT RUNNABLE — mypy is not installed (`No module named mypy`). |
| `gen_docs --check`; `project_os check`; every listed `scripts/*.py check`; parity self-test/check | PASS — each exits 0, with pre-existing documented warnings only. |
| `cargo fmt --check`; native and Windows/Linux cross-target Clippy `-D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS — all exit 0; cargo deny emitted its recorded advisory-database lock diagnostic only. |
| GitHub Actions baseline query | UNKNOWN — `gh run list --branch main --limit 5` could not connect to `api.github.com`; this is not treated as green. |

## Process exception assessment

`REVIEW_ROUND_EXCEPTIONS["E12"]` accurately explains the owner authorization for this fifth
round: round 3 found a genuine identity-unbound recovery defect and round 4 found the narrower
final-witness fallback in its repair. It does not authorize a sixth round. This round found a
further, shared recovery-state defect, so any additional repair/review requires a new explicit
owner decision and an updated exception record; the existing text must not be read as a blank
check to continue.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `docs/architecture/PERSISTENCE_MODEL.md`;
`docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`;
`docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`;
`docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
all three E12 verifier briefs; all four preceding E12 verifier review records; all three E12
executor evidence packets; `project/templates/SAFETY_VERDICT.md`; `scripts/check_process.py`;
and the changed `mutation.rs` and `sealedfs/lib.rs` sources.

## Overall round verdict

`FAIL` — E12-S01 and E12-S03 retain a genuine shared CR4 recovery defect (2 of 3 judged
stories; 67% finding/rejection yield). The exact round-4 fallback is closed, but the identity
witness mechanism is **not yet structurally sound** because its witness-last premise is enforced
only by the normal commit loop, not by recovery's own ordering and failure handling. This does
not yet require a fundamentally different identity-binding mechanism: a narrowly scoped,
directly-tested group state machine can enforce the intended invariant. Given three successive
rounds of variants, I recommend the owner authorize at most that targeted repair and a sixth
adversarial review, with direct state-transition tests as a prerequisite; do not accept this as a
residual and reconsider the design if that repair exposes another binding/recovery variant.
