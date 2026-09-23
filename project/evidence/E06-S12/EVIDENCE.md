# Evidence Packet - E06-S12

- Commit/PR: the E06-S12 commit on `main`
- Executor: Claude
- Independent verifier: pending - E06-S06's recheck round covers this repair
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - unreadable Codex lineage is incomplete and non-destructive; readable no-parent content is still no parent | `read_codex_parent_session_id`/`read_parent_from` now return `io::Result<Option<String>>`: an open error or a mid-read error is `Err`, recorded by `walk_rollouts` into the scope's `ReasonLog` (the rollout is still listed, with no parent, as the reference lists it). Tests: `an_unreadable_rollout_is_listed_but_makes_the_scope_partial`, `a_read_error_is_propagated_not_read_as_end_of_file`, `content_with_no_parent_is_still_a_valid_no_parent_result`. CLI level: the new NORMATIVE fixture `codex-unreadable-rollout` - Rust withholds, zero candidates, scan incomplete. | PASS |
| AC2 - more than MAX_RETAINED_REASONS companion failures: bounded, exact, Partial, withheld | `walk_companion_payload` takes the scope's `&mut ReasonLog` and records each failure as it happens; the intermediate `Vec` is gone. `more_companion_failures_than_retained_stay_bounded_exact_and_partial` (70 failures): `unobserved_count` = 70, 64 retained, Partial, companion attributed as degraded. CLI level: `many_companion_failures_withhold_the_claude_scope_with_an_exact_count` - `inspect` reports `error_count` 70, `clean` exits 4 and deletes nothing. | PASS |
| AC3 - both cases match the reference in default and custom root origins | Codex case: `python3 scripts/rust_python_parity.py check` - 14 NORMATIVE fixtures, including `codex-unreadable-rollout`, match in both root-origin scenarios. Claude high-count case: a characterization fixture was tried and removed, because the reference keeps its first 50 failures in filesystem listing order, so its recorded output differs between APFS and ext4 (CI's governance job caught it). The behaviour class - an unreadable companion withholds the whole tool - is pinned against the reference by `claude-partial-tree`, and the high count by the CLI test above. Against the unrepaired Codex adapter the same gate fails on `codex-unreadable-rollout` in both scenarios (Rust candidates = all three rollouts, `scan_complete` true), so the fixture detects the defect. | PASS (Claude high count: by class, not by fixture) |
| AC4 - the leaf-name race has an owner-visible disposition | Recorded in `project/evidence/E06-S06/SAFETY_VERDICT.md`, "Owner disposition - E21-S07 leaf-name race": accepted as a residual on 2026-09-23. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 / SI-009 | Unreadable rollout content under a readable directory | Scope Partial, tool withheld, parity with reference | PASS |
| SI-010 | A read error read as end of file; failures buffered without bound | Error propagated; failures written directly into the bounded log | PASS |
| SI-019 | A change reaching the mutation path | No mutation code touched; `check_mutation_boundary.py` passes | PASS |

## Mutation check

- Reverting the Codex adapter to `HEAD` makes `rust_python_parity.py check` fail on
  `codex-unreadable-rollout` in both scenarios, and `an_unreadable_rollout_is_listed_but_makes_the_scope_partial` fail.
- The Claude repair is structural (the buffer no longer exists), so no output-level test can
  distinguish the old code: both produced the same bounded observation at the end. The regression
  pins the observable contract; the absence of the buffer is visible in the diff.

## Verification Commands

```text
cargo test -p cancellai-provider-codex / -p cancellai-provider-claude   -> pass
python3 scripts/characterize.py generate && check                        -> 14 fixtures
python3 scripts/check_fixtures.py check                                  -> OK
python3 scripts/rust_python_parity.py check                              -> 14 NORMATIVE, both origins
cargo fmt / clippy (host + x86_64-pc-windows-gnu) / cargo test --workspace -> see commit
```

## Residual risks

- **The E21-S07 leaf-name race** remains on macOS and Linux, owner-accepted (see AC4).
- **Native Linux and Windows** permission behaviour for the two new fixtures is exercised by CI's
  parity job only where it runs; a root runner would make `chmod 000` ineffective and the recipes
  would not produce an unreadable path.

## Verifier verdict

pending
