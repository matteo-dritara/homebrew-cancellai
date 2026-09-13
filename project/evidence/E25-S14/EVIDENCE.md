# Evidence Packet - E25-S14

- Commit/PR: the shallow-clone refusal on `main`
- Executor: Claude
- Independent verifier: CI itself produced the failing observation (governance run 34732117378, tests run 34732117373)
- Change Risk: CR2
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a shallow clone is refused, with the fix named | `is_shallow()` reads `git rev-parse --is-shallow-repository`; `attributable_paths()` raises `RiskClassificationError` naming `fetch-depth: 0`. Reproduced end to end: `git clone --depth 1` of this repository, gate run inside it - before the fix it reported `E22-S07: declared CR3 but rust/crates/cancellai-model/Cargo.toml sets a floor of CR4`, the exact CI failure; after it, exit 2 with the refusal. | PASS |
| AC2 - a root commit attributes nothing | The commit format carries `%P`, and a block with no parents is skipped. A diff against nothing is the whole tree, so a root commit would hand every file to whatever story its message names - which is precisely what the shallow tip was doing. | PASS |
| AC3 - CI fetches the history | `fetch-depth: 0` on `tests.yml`'s `test` and `lint` jobs and `governance.yml`'s `pre-commit` job, each with the reason inline. `homebrew` is untouched: it runs no history-dependent gate. | PASS |
| AC4 - the suite skips rather than fails without history | `skip_without_history` guards the three tests that read attribution. The existing no-git guard stays: a source export has no `.git` at all, which is a different condition from a shallow clone and was already handled. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The same defect pointing the other way: a shallow clone that hides a real violation | This is the version that would never have been found, and it is why the gate refuses instead of degrading to a warning. A gate that cannot see its evidence and passes anyway is worse than no gate, because its green is read as a check. | PASS |
| n/a | The refusal itself being untested | `test_check_refuses_a_shallow_clone_rather_than_attributing_everything_to_one_story` substitutes the git shim so the repository reports itself shallow, and asserts the raise - the refusal is exercised rather than described. | PASS |

## Verification Commands

```text
git clone --depth 1 file://$PWD /tmp/shallow
(cd /tmp/shallow && python3 scripts/check_risk_classification.py check)
   before -> error: E22-S07: declared CR3 but rust/crates/cancellai-model/Cargo.toml sets CR4
   after  -> RISK CLASSIFICATION ERROR: this is a shallow clone ... fetch-depth: 0
python3 scripts/check_risk_classification.py check   -> OK with full history
python3 -m pytest tests/test_risk_classification.py  -> 32 passed
python3 scripts/check_workflows.py check             -> 6 workflow files, permissions and SHAs OK
```

## Compatibility

- No product behaviour. The gate's verdict on a full-history checkout is unchanged; what changes is
  its behaviour where it previously guessed.

## Performance / operability

- `fetch-depth: 0` makes three CI jobs clone the full history. This repository's history is small;
  the cost is seconds and the alternative is a gate that reports fiction.

## Residual risks

- **Other history-dependent gates were not audited one by one.** `check_platforms.py` already had
  this fixed for its own reason (the v1.10.0 tag), and `process_metrics.py`'s committed report was
  deliberately made a pure function of committed artifacts. Nothing systematically checks the rest.
- **The refusal is a hard failure.** Anyone running the full gate set in a shallow CI elsewhere now
  fails rather than passing vacuously. That is the intended direction and is still a behaviour
  change for a hypothetical third party.
- **Attribution coverage is still partial** - 24 stories of 131, the rest batched into commits that
  name several. The gate says so; this story does not improve it.

## Verifier verdict

closed on the owner's waiver; no independent verdict
