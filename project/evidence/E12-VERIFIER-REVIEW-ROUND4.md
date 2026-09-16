# E12 Independent Verifier Review — Round 4

Review-Scope: epic
Round: 4
Review target: uncommitted working-tree repairs on `82d30c1..f8dc4cc` (main), including rounds 1–3 repairs
Verifier: Codex
Date: 2026-09-16

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E12-S01 | FAIL | The round-3 fully formed conflicting group is now discarded: the new regression passes. But I independently created an existing artifact, its final legitimate record and matching **finalized** identity witness, then only a later conflicting operation's pending record (crash after its first pre-move write, before its witness). `recover_pending_moves` fell back to the old final witness, returned `Finalized`, and replaced `legitimate record` with `conflicting stale record`. Brief-Checksum: `eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734`. |
| E12-S02 | PASS_WITH_RESIDUALS | Restore remains an atomic no-replace move on supported Unix platforms, refusing where the primitive is unavailable. Its direct SI-013 obligation passes; Windows/unverified platforms remain explicit refusal residuals. The story's prerequisite metadata lifecycle is open in E12-S01. Brief-Checksum: `012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0`. |
| E12-S03 | FAIL | Archive shares E12-S01's group recovery function, so the same old-final-witness/new-partial-group sequence can overwrite archive record, length and digest metadata. The SHA-256 repair still detects equal-length corruption; this failure is the recovery binding. Brief-Checksum: `a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b`. |

## Failures and required repairs

### E12-S01

Reproduction: an isolated external Rust program used the production `SealedRoot` writer to create
`artifact.txt`, final record `legitimate record`, and matching final `move-identity`. It then
created only `artifact.txt.quarantine-record.json.pending` with `conflicting stale record`.
This is reachable when a new conflicting operation crashes after writing its first sidecar and
before writing its own witness; its no-replace move never succeeds. Recovery returned `Finalized`,
deleted the pending entry, and the final record became `conflicting stale record`.

Required repair: a pending group must not borrow an earlier finalized witness as proof of a newer
operation. Require the group's own pending witness and order finalization so that it occurs last,
or add a durable operation identifier binding a finalized witness to every sibling. Missing,
malformed, unreadable, or mismatched proof must discard the whole group without touching existing
final metadata. Add this exact partial-write regression. This violates E12-S01 AC3 and SI-020.

### E12-S03

Reproduction: the same shared recovery function accepts the old final witness for a partial new
archive group, allowing stale archive record/length/digest sidecars to replace valid metadata for
the existing artifact.

Required repair: apply the E12-S01 group-bound proof/ordering repair to archive's full multi-sidecar
group and add the corresponding crash/partial-group regression. This violates E12-S03 AC3 and
SI-020.

## Gate status

| Command(s) | Result |
| --- | --- |
| Independent recovery reproduction | FAIL as described above; exact round-3 fully formed mismatch, solo finalize, and no-move discard regressions pass. |
| `python3 -m pytest tests -v` | PASS after the narrowly fixed process-test fixture and generated control-plane refresh: 665 passed, 543 subtests passed. |
| `python3 -m ruff check .`; `python3 -m ruff format --check .`; full AGENTS.md mypy target list plus `scripts/check_process.py` | NOT RUNNABLE: this environment has no Ruff or mypy module. |
| `gen_docs --check`; all listed `scripts/*.py check`; parity self-test/check | PASS before this record; post-record process/governance checks are listed below. |
| `cargo fmt --check`; native and Windows/Linux cross-target Clippy `-D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS. `cargo deny` emitted only its recorded non-failing allow-list/duplicate warnings. |

## Process exception assessment

`REVIEW_ROUND_EXCEPTIONS["E12"]` accurately records the reason for this owner-authorized fourth
round: rounds 1 and 2 found real CR4 defects, and round 3 found a real identity-unbound recovery
defect in the repair rather than an accepted residual. It does not itself claim that round 4 would
pass, so it remains accurate despite this round's further finding.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `docs/architecture/PERSISTENCE_MODEL.md`;
`docs/security/THREAT_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`;
`docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
the three E12 verifier briefs; `E12-VERIFIER-REVIEW.md`, `-ROUND2.md`, and `-ROUND3.md`;
the three executor evidence packets; `project/templates/SAFETY_VERDICT.md`; the changed Rust
sources and `scripts/check_process.py`.

## Overall round verdict

`FAIL` — E12-S01 and E12-S03 retain a genuine CR4 defect (2 of 3 judged stories; 67% finding and
rejection yield). The **exact** round-3 fully formed mismatched-witness reproduction is closed,
but round 3's underlying requirement — bind every pending group to the operation that produced it
before finalizing — is not closed: a partial new group can borrow an old final witness. No product
code was repaired by the verifier. E12-S01 returns to `in_progress`; E12-S02 and E12-S03 are
`blocked` on E12-S01 despite their respective direct verdicts, because control-plane dependency
validation correctly refuses an active dependent state while that prerequisite is open; E12-S04
remains `planned`.
