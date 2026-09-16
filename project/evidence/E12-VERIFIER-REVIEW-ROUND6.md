# E12 Independent Verifier Review — Round 6

Review-Scope: epic
Round: 6
Review target: uncommitted working-tree scope reduction on `82d30c1..f8dc4cc` (main)
Verifier: Codex
Date: 2026-09-16

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E12-S01 | PASS_WITH_RESIDUALS | `recover_pending_moves`, `PendingRecoveryOutcome`, identity witnesses, and recovery tests are absent from executable Rust. An independent probe confirmed pre-move write failure leaves source untouched; rejected no-clobber move cleans pending metadata; successful move finalizes the full record; and post-move finalize failure returns `Err` while retaining the exact record under `.pending`. Brief-Checksum: `eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734`. |
| E12-S02 | PASS_WITH_RESIDUALS | Restore retains atomic platform no-replace commit and explicit refusal where unavailable. It writes no sidecar and has no removed-scanner caller or dependency. Brief-Checksum: `012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0`. |
| E12-S03 | PASS_WITH_RESIDUALS | Archive retains write-before-move/finalize behavior plus SHA-256 integrity checks; no automated process can finalize incomplete metadata or authorize the still-unimplemented purge. Its residual is a correct manual-only `.pending` sidecar after a post-move finalize error. Brief-Checksum: `a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b`. |

## Removal and residual assessment

The removal is clean. Repository-wide searches for `recover_pending_moves`,
`PendingRecoveryOutcome`, `move-identity`, identity witness, and related scanner terminology
found no executable implementation or caller. Historical round records and evidence retain their
necessary account of the removed design; current module documentation, persistence documentation,
changelog, and current evidence explicitly say that automated recovery is absent. The Archive
reference to the protocol above is read with that immediately preceding disclosure: it shares the
write-before-move/finalize protocol and manual residual, not an automatic scanner.

The retained property is independently reproduced, not inferred from deletion: no sidecar write
means no move; failed no-clobber move removes prewritten pending state; success preserves full
metadata; and injected finalization failure reports `Err` and leaves the artifact intact with
full, correct content under `.pending`. No code later completes it on its own.

This residual does not make E12-S01 AC3 untrue: original identity/restore metadata is
contentlessly and durably recorded before a successful move, though pending rather than final
naming requires explicit manual completion after a finalization failure. It does not make
E12-S03 AC3 untrue: SHA-256 verification remains and no source-purge capability exists; pending
archive metadata cannot silently verify or purge anything. SI-020 is preserved because neither
path disguises an irreversible action or promotes incomplete state to destructive authority. The
residual is acceptable only as `PASS_WITH_RESIDUALS` for these unwired primitives, not as
authorization for future automated recovery or purge.

## Counterexample axes checked

- Path/identity, intermediate links, destination collision, and concurrent restore recreation: retained handle-relative identity checks and atomic no-replace moves remain covered.
- Crash/failure/retry: pre-write, failed-move, success, and post-move-finalize states were reproduced; the last has no retry/scanner path by design.
- Malformed/incomplete metadata and destructive authority: archive integrity fails closed on missing/malformed sidecars; purge is E12-S04 planned.
- Platform differences: Windows and unsupported Unix primitives refuse rather than fall back.

## Process exception assessment

`REVIEW_ROUND_EXCEPTIONS["E12"]` is accurate and candid. It distinguishes the three successive
scanner defects in rounds 3–5, records owner authorization beyond the cost ceiling, and states
that round 6 verifies removal rather than a fourth repair. It neither minimizes those failures
nor implies the removed mechanism still exists.

## Gates actually run

| Command(s) | Result |
| --- | --- |
| Independent Rust probe | PASS — exercised pre-move write failure, rejected move cleanup, normal success, and injected post-move finalization failure. |
| `python3 -m pytest tests -v` | PASS — 665 passed, 547 subtests. |
| `.venv/bin/ruff check .`; `.venv/bin/ruff format --check .`; full AGENTS.md mypy target list | PASS. |
| `gen_docs --check`; all AGENTS.md listed `scripts/*.py check` commands; parity self-test/check | PASS. Process/risk/deny emit only recorded informational warnings. |
| `cargo fmt --check`; native and Windows/Linux-target Clippy `-D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS. |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `docs/architecture/PERSISTENCE_MODEL.md`;
`docs/architecture/PLATFORM_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`;
`docs/development/RELEASE_GATES.md`; all five prior E12 verifier records; all three verifier
briefs; current Safety Verdicts and executor evidence; changed Rust and sealedfs source;
`CHANGELOG.md`; `scripts/check_process.py`; and `project/templates/SAFETY_VERDICT.md`.

## Overall verdict

`PASS_WITH_RESIDUALS` — all three reviewed CR4 stories pass their retained contracts with an
explicit, fail-closed manual-completion residual. E12-S04 remains planned, so the epic is not
ready to close; its top-level status is intentionally left unchanged.

## Six-round closing assessment

Stopping the automated-recovery attempt was the right call. Rounds 3, 4, and 5 each exposed a
real, distinct correctness flaw in successive designs for the same scanner. The retained
write-before-move protocol has a smaller independently demonstrated claim: it never loses or
fabricates sidecar content, and never makes a failed finalization look complete. That bounded
manual residual is safer and more truthful than a fourth automated state-machine repair in this
change. Any future automation must be a new story with direct state-transition, crash-boundary,
and adversarial tests before it is allowed to act.
