# Safety Verdict - E21-S03 (owner acceptance)

- Change: provider scan-completeness propagation
- Risk: CR4
- Decided by: **project owner**, not an independent verifier
- Date: 2026-09-03
- Independent review history: round 1 (`SAFETY_VERDICT.md`, Codex) returned `FAIL` on SI-008,
  SI-009, SI-010 and SI-014. Every finding is repaired; none was disputed.

## What this file is, and is not

This is not an independent verification. The owner directed closure of E21 after the round-1
findings were repaired, without spending the second round ADR-0014 permits. The independent
`FAIL` verdict is committed beside this file and is not altered. See
`project/evidence/E21-CLOSURE.md` for the decision and the residual risk it accepts.

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

Whether an incompletely observed provider scope can authorize irreversible deletion.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | A partial scope cannot produce irreversible actions | Both partial-tree fixtures pass the differential gate in both root-origin scenarios; `an_unreadable_project_directory_makes_the_scope_partial` and `an_unreadable_session_directory_makes_the_scope_partial` assert the reason carries the failing path | PASS |
| SI-009 | Unknown scan state is non-destructive | The verifier's own reproduction - a real mode-000 `projects/` - now exits `4` with the session intact, pinned natively by `an_unreadable_claude_projects_root_withholds_and_exits_four`. The counterexamples that keep this from degenerating into "always withhold" pass too: absent `projects/`, absent provider, symlinked `projects/` | PASS |
| SI-010 | Listing, file-type and metadata failures are visible | `walk_companion_payload` returns one `CompletenessReason` per failure with path and cause; `modified()` and companion `symlink_metadata` failures are recorded rather than swallowed; `describe()` reports the truthful total, never the bounded retained count | PASS |
| SI-014 | Safety withholding has a distinct status | The native reproduction exits `4` and prints "Nothing was cleaned: safety withheld the requested work."; the same holds for the `$HOME`-unreadable case found after the review | PASS |
| C-11 | cancellAI does not become an unbounded storage producer under failure | `ReasonLog` bounds retention at `MAX_RETAINED_REASONS` while counting every failure; `reason_retention_is_bounded_but_the_count_is_not` asserts both halves | PASS |

## Adversarial cases

Every case the round-1 verdict named, plus the ones this repair added:

- a real mode-000 Claude `projects/` root (the verifier's reproduction) - withheld, exit 4;
- a real mode-000 Claude *project* directory beside a readable one - scope Partial, readable
  session still reported, `degraded_companions` empty so it cannot pass via the E06-S02 channel;
- a real mode-000 Codex session directory, and the same under `archived_sessions/`;
- a companion payload directory that cannot be listed - reported on both channels;
- an unreadable `$HOME` - found by the executor after the review, in the same class, now withheld;
- the counterexamples: absent provider, absent `projects/`, symlinked `projects/`, empty readable
  tree, and a fully readable tree that must still yield delete candidates.

## Differential / compatibility evidence

`python3 scripts/rust_python_parity.py check` - 12 NORMATIVE fixtures, both root-origin
scenarios. `cargo test --workspace` - 327 passed. `cargo clippy -D warnings`, `cargo fmt --check`,
`cargo deny check`, `check_mutation_boundary.py`, `check_provider_compatibility.py` all pass.

## Known residual risks

1. No independent confirmation of these repairs: the verifier reproduced the defects, and the
   fixes are executor work verified by executor-written tests.
2. Reason retention is bounded at 64 named paths; the count remains truthful.
3. `io::ErrorKind` classification is coarser than the reference's `errno` on some platforms. It
   affects the message, never the verdict - any unclassified failure still withholds.

## Owner decision

Accepted as `PASS_WITH_RESIDUALS`. The residuals above are recorded in
`project/evidence/E21-CLOSURE.md` and none is a HIGH/CRITICAL unresolved safety risk: the
defect the round-1 verdict failed this story for is closed and pinned by a native regression
that reproduces the verifier's own scenario.
