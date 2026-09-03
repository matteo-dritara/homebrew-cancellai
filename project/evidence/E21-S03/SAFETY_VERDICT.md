# Safety Verdict - E21-S03

- Change: provider scan-completeness propagation
- Risk: CR4
- Review target: working tree against `c00f16f56534651e304c12c5040303984317ac3d` (the requested `c00f16f..HEAD` range is empty; the implementation was uncommitted)
- Independent verifier: Codex (`/root`)
- Date: 2026-09-03

## Verdict

`FAIL`

## Safety surface changed

Claude and Codex discovery now return `ScopeCompleteness`, and planning converts incomplete
scopes into `Observe` actions. This controls whether incomplete observation can authorize
irreversible deletion.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | A partial scope cannot produce irreversible actions. | Partial-tree parity fixtures pass on the repaired worktree, but a Claude `projects/` root that cannot be listed is converted into a clean empty resolution. | FAIL |
| SI-009 | Unknown scan state is non-destructive. | Native run with `$HOME/.claude/projects` mode `000`: `clean --yes --days 1 --keep-latest 0 --tool claude` printed `Nothing to clean` and exited `0`, rather than reporting an incomplete scan and exiting `4`. | FAIL |
| SI-010 | Listing, file-type, and metadata failures are visible. | `directory_size_and_latest_mtime` reduces all nested failures to `fully_read = false`; its caller emits one generic companion-directory `Io` reason, losing each failing path/cause. `metadata.modified().ok()` and a failing companion `symlink_metadata` are silently collapsed. | FAIL |
| SI-014 | Safety withholding has a distinct status. | The native unreadable-`projects/` case exited `0`; no withholding was surfaced. | FAIL |

## Adversarial cases

- Reproduced the requested root-unreadable case with a real mode-`000` `projects/` directory.
  `discover_claude_sessions` correctly constructs `ScopeCompleteness::Unknown`, but
  `resolve_claude` returns its `empty()` `Complete` resolution whenever
  `SessionDiscoveryScope::Unavailable` is set.
- Inspected all Claude observation channels: companion nested walk failures lose exact reasons;
  companion `symlink_metadata` errors and `modified()` errors are discarded.
- Confirmed the ordinary counterexamples remain non-destructive only as intended: absent roots,
  empty readable trees, and a symlinked `projects/` root each produce no candidates.

## Differential / compatibility evidence

- Current worktree: `python3 scripts/rust_python_parity.py check` passed 12 NORMATIVE fixtures
  in both root-origin scenarios.
- An exact reversal of only the two adapter `session.rs` files cannot compile after E21-S04
  changed their public result types. A baseline `c00f16f` engine plus the E21-S02 fixtures and
  characterization rules produced the expected four divergences: both partial fixtures in both
  root-origin scenarios, including `withheld: python=True vs rust=False` and
  `scan_complete: python=False vs rust=True`.

## Known residual risks

`Vec<CompletenessReason>` grows once per failure without a bound. A hostile unreadable tree can
turn diagnostic collection into memory pressure. This is fail-closed for deletion, but needs a
bounded-diagnostics/counting design when the primary repair is made.

## Rollback / recovery

Do not close or cut over on this worktree. Repair the resolver and every silent Claude metadata
channel, add native root-unreadable and nested-failure regressions, then rerun parity and the
CR4 adversarial pass. No user data was touched by this review.

## Owner decision

`REJECT`

Owner note: E21-S03 is blocked behind failed E21-S01/E21-S02 dependency state and requires the
listed repair before the second review round.
