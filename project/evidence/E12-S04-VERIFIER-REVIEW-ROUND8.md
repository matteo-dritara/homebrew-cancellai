# E12-S04 Verifier Review — Round 8 (final)

Review-Scope: story
Review target: `31cf69d..74d1398` on `main`
Verifier: Codex
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Date: 2026-09-20

## Verdict

`PASS_WITH_RESIDUALS`

E12-S04's store implementation remains unchanged since the round-4 repair commit
`31cf69d`; `git diff --exit-code 31cf69d..74d1398 -- rust` is empty. The final review
re-executed the CR4 Rust gate set and independently verified the repaired E32-S01 closing
gate before issuing this genuine new passing round.

E12-S04 is eligible to close. The E12 epic itself remains `in_progress` because
`scripts/release.py check` refuses an epic marked `done` without release evidence naming it;
preparing that release is explicitly reserved for the orchestrator and is not performed here.

## Independent result

| Contract | Result | Evidence |
| --- | --- | --- |
| AC1 / ADR-0033 | PASS_WITH_ACCEPTED_RESIDUAL | The four descriptive annotation fields remain absent at the persistence boundary; the three linkage fields use the identifier-shape check through the sole writer. `do-not-purge-this` remains the explicitly owner-accepted, disclosed residual. |
| AC2 / SI-020 | PASS | Public `EventLedger::append` refuses every `Purged` event; the crate-private writer remains reachable only after `Delete + Irreversible` is checked. |
| E13-S02 mutation-reference contract | PASS | The shared write boundary still refuses missing/blank plan IDs and empty evidence IDs before insertion. |
| CR4 closing gate | PASS | Before this section was appended, the actual current Safety Verdict's last standalone verdict was its historical `REJECT`, and the repaired parser returned `False`; it did not spuriously flip. The E32-S01 final review then passed its unclosed-backtick and tilde-fence reproductions, so this new final verdict is the only reason the gate may now accept this file. |

The E12 evidence packet refers to a Round 7 review record, but no committed
`E12-S04-VERIFIER-REVIEW-ROUND7.md` or Round 7 section exists in the Safety Verdict. This
round does not treat that absent record as evidence: it independently reran the relevant gates
and checked the unchanged Rust surface. The historical verdict ledger itself is retained
append-only.

## CR4 gates

- PASS: `cargo fmt --check`.
- PASS: `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- PASS: `cargo check --workspace --all-targets`.
- PASS: `cargo test --workspace`.
- PASS: `cargo deny check` (only its pre-existing non-failing license/duplicate warnings).
- PASS: all shared Python, governance, evidence, documentation, handoff, EARS, and gate-sensitivity
  checks recorded in E32-S01 Round 3.

## Residual risks

ADR-0033's residual remains accepted rather than resolved: an arbitrary caller can supply a
short, ordinary phrase that is identifier-shaped in `artifact_id`, `plan_id`, or `evidence_ids`.
A future orchestrator must derive those values from trusted purge, plan, and evidence records.
No real purge path is wired to this primitive, so crash/retry coupling of an OS purge and a
tombstone write remains a future integration concern.
