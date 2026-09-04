# E20-S03 - Round 2 repair (independent verifier review round 1 findings)

- Story: E20-S03
- Round: repair after `project/evidence/E20-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04
- Process exception: **owner-authorized combined repair+self-verify+close round (2026-09-04) - see conversation record.** Same authorization recorded in `E20-S01/ROUND2-REPAIR.md`.

## Verdict this repairs

Round 1 verdict: `FAIL`, three findings:

1. **The checker accepted fabricated support.** An in-memory mutation setting WSL2 to `tier: 1`,
   both capabilities `"verified"`, and **zero** `evidence_tests` produced `validate(...) == []`.
   Replacing a real evidence entry with `observe_identity` (a production function, no `#[test]`
   anywhere) also produced `[]`.
2. **Evidence not tied to the specific capability it claimed.** macOS/Linux's cited "mutation"
   evidence, `establish_rejects_a_root_swapped_to_a_symlink_after_final_validation_but_before_
   the_bind`, tests sealed-root *establishment* and deletes nothing; real deletion tests exist
   elsewhere in the tree but were never cited.
3. **Runtime contradicted the matrix.** WSL2 was listed tier 2/unverified, but
   `confirmed_delete_file`/the sealed-root walk compiled and ran unconditionally for every
   `cfg(unix)` target including a WSL2 guest - the generated "non-tier-1 platforms remain
   inspect-only or refused" claim was not actually enforced there.

## What changed

**Findings 1 and 2 (fabricable evidence, wrong-capability citations)** - `scripts/
check_platforms.py` and `project/platforms.json` are both restructured:

- `capabilities.<name>` is now `{"state": ..., "evidence": [{"name", "file"}, ...]}`, not a flat
  per-platform `evidence_tests` list - evidence is now tied to the specific capability it claims
  to support, closing the "mutation evidence that doesn't delete anything" gap by construction
  (a reviewer or future editor can no longer accidentally cite identity evidence for a mutation
  claim, or vice versa, without it being visibly in the wrong bucket).
- `validate()` now requires: (a) a `"verified"` capability's `evidence` list is non-empty; (b)
  every evidence entry's `name` is a real `#[test]`-annotated function found at the *exact*
  cited `file` (`file_defines_test_fn`, walking backward from the `fn` line over blank/comment/
  other-attribute lines looking for `#[test]` - not merely a `fn` of that name found anywhere in
  the tree, which is what let both round-1 reproductions through).
- macOS/Linux's mutation evidence is corrected to real deletion tests:
  `system_executor_deletes_a_real_file_confirmed_by_identity`
  (`rust/crates/cancellai-platform/src/mutation.rs`) and
  `execute_deletes_when_identity_still_matches`
  (`rust/crates/cancellai-safety/src/mutation_executor.rs`).
- Both round-1 reproductions were re-run against the repaired script and now correctly produce
  errors (verified live during this repair, not merely reasoned about): the fabricated-WSL2-
  tier-1 case now reports three separate errors (empty identity evidence, empty mutation
  evidence, tier-1 bar not met); the `observe_identity`-as-evidence case now reports "no
  `#[test]`-annotated fn `'observe_identity'` found in ...".
- `tier: 1` additionally now requires a `verified_commit` (a real git ancestor of `HEAD`,
  checked via `git merge-base --is-ancestor`, offline; best-effort cross-checked via `gh run
  list --commit ... --workflow rust.yml` when `gh` can reach GitHub - a warning, not a hard
  failure, when it cannot, since this script may itself be running inside the very CI job whose
  outcome it cannot know yet). macOS/Linux's `verified_commit` cites this branch's own base
  commit (`54b8f356...`), independently confirmed via `gh run list` to have a real successful
  `rust.yml` run (`https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/
  33862492833`) - real, pre-existing, unaffected-by-this-epic evidence, not fabricated.
  Windows's `verified_commit` stays `null` (and `identity.state` stays `"unverified"`, not
  `"verified"`) until this repair's own commit has a real, confirmed CI run - see
  `E20-S01/ROUND2-REPAIR.md`.

**Finding 3 (WSL2 mutation not actually gated)** - repaired at the source, in
`cancellai-platform` (full detail in `E20-S02/ROUND2-REPAIR.md`): `confirmed_delete_file` now
calls `refuse_unverified_wsl2_mutation`, which refuses on a detected WSL2 guest. `project/
platforms.json`'s `wsl2.capabilities.mutation.state` is `"unsupported"` (matching Windows'
pattern: explicitly refused by enforced code, not merely "unverified" by inference), citing
`refuse_unverified_wsl2_mutation_refuses_on_wsl2` as evidence. `docs/PLATFORMS.md`'s generated
"What a non-tier-1 platform does today" section now names this WSL2-specific gate explicitly
alongside the pre-existing Windows/Unix ones.

## Verification

```text
python3 scripts/check_platforms.py generate
python3 scripts/check_platforms.py check
python3 -m ruff check scripts/check_platforms.py
python3 -m ruff format --check scripts/check_platforms.py
python3 -m mypy scripts/check_platforms.py
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
python3 scripts/project_os.py check
python3 -m pytest tests -q
```

All green. Both round-1 reproductions re-verified fixed live (see "What changed" above,
commands available in this session's transcript, scratch copies discarded and not committed).

## Residual risks (updated from round 1)

- `gh_confirms_successful_run`'s workflow-level "success" conclusion does not individually
  confirm every OS-matrix job inside that run succeeded (GitHub's API would need a second,
  per-run `gh run view --json jobs` call per platform to do that precisely) - `fail-fast: false`
  and no `continue-on-error` in `rust.yml` mean a workflow-level "success" does in practice
  require every matrix job green, but this is inferred from the workflow's own configuration,
  not independently re-verified per job by this script. A future hardening could add the
  per-job check; not done here to keep this repair's own scope bounded to the review's actual
  findings.
- `find test coverage via a bare `fn` name is still what identifies a *candidate* line to check
  for `#[test]`; a same-named `fn` at the same file that happens to also carry a `#[test]`
  attribute nearby but is semantically unrelated to the claimed capability would still pass -
  this checker verifies "a real test with this name exists at this file," not "this test's body
  actually exercises this capability," which remains a human/reviewer judgment this tooling
  narrows the space for rather than eliminates.
