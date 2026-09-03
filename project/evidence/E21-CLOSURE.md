# E21 Closure - owner-directed, without a second review round

- Epic: E21 - Target Engine Trust Remediation
- Decided by: **project owner**
- Date: 2026-09-03
- Independent review history: round 1 (`project/evidence/E21-VERIFIER-REVIEW.md`, Codex) returned
  `FAIL` for E21-S01/S03/S04/S05/S06 and `PASS_WITH_RESIDUALS` for E21-S02/S07.

## What this file is, and is not

This is not an independent verification, and it does not alter the round-1 verdicts, which stand
committed beside it. ADR-0014 allows two review rounds; the owner directed closure after the
round-1 findings were repaired, without spending the second. That is the owner's decision to
make, and this file records it rather than letting the epic close with a `done` status nobody can
trace to a decision.

The honest description of E21's assurance level is therefore: **one independent adversarial round,
all of whose findings were reproduced, repaired, and pinned by regression tests written against
the verifier's own reproductions - and no independent confirmation of those repairs.**

## What round 1 found, and what closes it

| Finding | Story | Repair | Regression pinning it |
| --- | --- | --- | --- |
| An unreadable Claude `projects/` root was converted to a clean empty scan; `clean --yes` exited 0 where the reference exits 4 | E21-S03 | `SessionDiscoveryScope` splits `Unavailable` (absent/symlinked) from `Unobservable` (exists, unreadable); `resolve_claude` carries the observation through | `an_unreadable_claude_projects_root_withholds_and_exits_four` (native CLI, exit 4 + session survives), `..._is_reported_incomplete_with_a_real_count` |
| Nested Claude observation failures collapsed to a boolean; `modified()` and companion `symlink_metadata` errors discarded | E21-S03 | `walk_companion_payload` returns a `CompletenessReason` per failure with path and cause; both former `.ok()`/`if let Ok` sites record | `a_degraded_companion_is_reported_on_both_channels`, plus the per-reason assertions in `an_unreadable_project_directory_makes_the_scope_partial` |
| `Vec<CompletenessReason>` unbounded on a hostile tree (C-11) | E21-S03 | `ReasonLog` retains at most `MAX_RETAINED_REASONS` while counting every failure; `ScopeObservation` carries classification and truthful total as one value | `reason_retention_is_bounded_but_the_count_is_not` |
| The falsely `Complete` value was transported intact by the planning interface | E21-S04 | Closed upstream; `ProviderResolution` now stores `ScopeObservation`, so completeness, reason and count are three views of one value | `compile_fail` doctest retained; parity gate unchanged across the refactor |
| Scheduled 10k/100k benchmarks still measured the unreachable `scan_scope` | E21-S05 | `performance_scheduled_shipped.rs` carries the datasets against `resolve_claude`/`resolve_codex`, same trend schema; `rust-benchmark.yml` emits it as the primary artifact | 10k smoke run: 20,000 artifacts in 1.74s against a 60s threshold |
| The bounded reader accepted `MAX_PARENT_SCAN_BYTES + 1` | E21-S06 | `read_total` bounds actual reads exactly; a budget-truncated record is not parsed | `a_single_enormous_line_cannot_pull_the_file_in_through_the_back_door` now asserts `<= MAX_PARENT_SCAN_BYTES` |
| Disclosure claimed a repair that was still open | E21-S01 | Repaired by closing the defect, not by softening the text; every claim re-checked against a native reproduction | The native reproductions above |

## Found after the review, by the executor, in the same class

The round-1 finding was one instance of a pattern - completeness computed and then discarded a
layer up - and the pattern had a second instance the verifier did not reach: both resolvers
gated on `root.exists()`, and `Path::exists()` answers `false` for "not installed" *and* for
"not allowed to look". With an unreadable `$HOME` the engine reported a clean empty scan and
exited `0`.

Closed by removing the gate (discovery's own observation already distinguishes the two cases),
pinned by `an_unreadable_home_withholds_rather_than_reporting_nothing_to_clean` with
`a_home_with_no_provider_installed_is_complete_and_exits_zero` as its counterexample.

It is recorded here rather than folded silently into E21-S03 because it is evidence about the
review's coverage, and because it is the strongest argument on the record for spending a second
round rather than closing here.

## Residual risk the owner accepts

1. **No independent confirmation of the round-1 repairs.** Every repair above is executor work
   verified by executor-written tests. The verifier reproduced the defects; nobody independently
   reproduced the fixes.
2. **The `fstatat`/`unlinkat` window** (E21-S07) remains open by construction and is documented
   in ADR-0017 rather than claimed closed.
3. **Bounded reason retention** means an operator inspecting a catastrophically broken tree sees
   64 named paths and a truthful total, not every path.
4. **The audit that produced this epic was written by the same agent that implemented it.** A
   gap `docs/audits/2026-09-03-CODE_REVIEW.md` did not find is still undisclosed. E22 does not
   change that.

## Consequence for the cutover gate

E06-S04 lists `E21` among its blockers. This closure satisfies that dependency in the control
plane, and `docs/development/RELEASE_GATES.md` G2 records what it does and does not mean: the
reproduced authority defect is repaired, and the gate still awaits the independent CR4 pass and
owner-visible Safety Verdict that a cutover requires. Closing E21 does not make E06-S04 ready.
