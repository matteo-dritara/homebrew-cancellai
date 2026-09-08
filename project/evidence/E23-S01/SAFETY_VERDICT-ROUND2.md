# Safety Verdict - E23-S01 (round 2)

- Change: release-history availability gate - round 2 repair of the round 1 multi-checkout bypass
- Risk: CR4
- Review target: `dbb1ae9..<this commit>` (repairs `21d4379..dbb1ae9`, the range round 1 rejected)
- Independent verifier: none for this round - see "Process note" below; round 1 was Codex (`/root`)
- Date: 2026-09-08

## Verdict

`PASS`

Self-assessed by the executor against round 1's exact finding; not independently re-confirmed
by a separate agent - see "Known residual risks" below and
`project/evidence/E23-S01/ROUND2-REPAIR.md`'s Process note.

## Safety surface changed

Unchanged from round 1: the release workflow determines whether platform evidence, including
Windows mutation authority, may be published. This round repairs the static guard meant to
prevent a tagged release from losing the history required to verify that evidence, closing the
specific gap round 1 found in that guard.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | Release verification cannot be bypassed by a workflow layout that makes provenance evidence unavailable, including a multi-checkout layout. | `checkout_fetch_depth_for_run()` now tracks every `actions/checkout` step's target directory and depth in step order, and resolves the depth of the specific directory the gate's `run:` step (its own `working-directory:`, default workspace root) executes in - not merely the job's first checkout. Round 1's exact bypass (full-history checkout at an isolated `path:`, default shallow checkout in the workspace) is caught: `tests/test_workflows.py::ReleaseHistoryMultiCheckoutTests::test_full_history_checkout_at_an_isolated_path_does_not_satisfy_a_shallow_workspace`. | PASS |

## Adversarial cases

All in `tests/test_workflows.py::ReleaseHistoryMultiCheckoutTests` (7 tests), reproducing round
1's exact finding plus 6 additional counterexamples generated independently while repairing it:

- round 1's exact bypass (full-history checkout at `path: full-history-copy`, shallow default
  checkout in the workspace, gate runs in the workspace) - caught;
- a later full-history checkout overwriting an earlier shallow one at the workspace root -
  correctly accepted (proves the fix does not reject every multi-checkout workflow, only ones
  where the gate's own directory ends up shallow);
- the inverse ordering - full history at the root first, then an unrelated later shallow
  checkout clobbering it before the gate runs - caught;
- the gate's `working-directory:` pointed at a path that was checked out full (root stays
  shallow) - correctly accepted;
- the gate's `working-directory:` pointed at a path never checked out at all - caught;
- a leading `./` on `path:` vs. a bare `working-directory:` naming the same directory -
  recognized as the same directory, not a false failure;
- a full-history checkout that only happens after the gate step already ran - not credited
  retroactively.

Also reproduced round 1's original historical-failure evidence again (unchanged): a real
`git clone --depth 1 --branch v1.10.0` against this repository still fails
`check_platforms.py check` exactly as `gh run 34252829459` did, and the repaired checker still
accepts the real, current `release.yml`.

## Differential / compatibility evidence

`scripts/check_platforms.py`'s `git_is_ancestor()`/`validate()` remain unchanged across both
rounds - only the workflow-policy checker (`check_workflows.py`) and its tests changed in this
repair. Full Python gate set (204 tests, ruff, mypy, all `scripts/*.py check` commands, `release.py
check`) passes; no Rust source file changed since round 1, whose Rust gates (`fmt`, `clippy -D
warnings`, `cargo test`, `cargo deny check`) already passed and are unaffected by this diff.

## Known residual risks

- This verdict was produced by the same agent (Claude) that implemented both the original fix
  and this repair, under explicit owner instruction to close without a second independent
  review round (see Process note). It is evidence of adversarial self-testing against a
  concretely known bypass class, not an independent falsification pass by an agent unprimed by
  the executor's own reasoning, which is what `AGENT_PROTOCOL.md` normally requires for CR4.
- The static checker is still a source-text pattern matcher, not a YAML/GitHub-Actions-
  semantics engine (consistent with every other check in this file, by established repository
  convention - see the module docstring). A sufficiently unusual, YAML-valid workflow
  construction not covered by the adversarial tests above (e.g., a `run:` step's command built
  from a matrix expression rather than a literal string) could still evade it; this mirrors the
  same accepted class of limitation `release_gate_drift_errors()` already has for its own
  literal-command comparisons, elsewhere in this same file.
- AC4 (a replacement release tag verified successfully before publication) remains open until a
  real tag is pushed with this repair in place and its `verify` job is observed to pass on
  real GitHub Actions CI - tracked as part of this same closure, not deferred further.

## Rollback / recovery

If a real tagged release still fails `check_platforms.py check` after this repair, do not
force-publish: `verify`/`publish` staying red is `release.yml` doing exactly its job (E22-S01).
Revert this repair, reopen E23-S01, and treat any further finding as a new round.

## Owner decision

`ACCEPT`

Owner note: explicit in-session instruction, 2026-09-08 - "procedi subito con l'implementazione
di E23-S01 ... Analizza la situazione ed applica le fix necessarie. Ricontrolla affinché sia
tutto ok (reitera se necessario). Al termine, non è necessaria un'ulteriore review, puoi mettere
il task in done. Commit atomico, tag e push." This authorizes closing E23-S01 without a second
independent verifier round, on the condition (met above) that the repair concretely addresses
round 1's finding with adversarial regression coverage and every required gate re-run green.
