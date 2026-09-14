# Evidence Packet - E27-S05

- Commit/PR: the subtractive pass on `rework/codebase-health`, one commit per removal
- Executor: Claude
- Independent verifier: none - branch work, CR0
- Change Risk: CR0
- Spec version/commit: `project/epics/E27.json` at this commit
- Owner criterion: cost/benefit including context cost, measure before proposing, one reversible
  commit per removal, and nothing guarding a Safety Invariant removed for cost

## What the measurement said before anything was removed

| Candidate | Measurement | Decision |
| --- | --- | --- |
| Gate set | 26 gates, **13 seconds total**; across 20 checkers no two assert the same property - the only overlaps are shared directories | **Nothing removed.** The cost of this repository is attention, not CPU |
| `AGENTS.md` | **234 lines on 8 September, 395 today** - a 69% growth in six days, in the one file loaded into every session | **21 lines removed** |
| `docs/ARCHITECTURE.md` | A second index of `docs/architecture/`; 1 inbound link, 0 code references, and **already drifted** - `INDEX.md` lists nine architecture documents, this listed eight | **Removed**, unique content folded into `INDEX.md` |
| The other 25 "never named" documents | Four are **generated** (a review reads the source, not the output); most carry 8-26 inbound links; the rest are not loaded into any context | **Nothing removed.** See below |

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every removed sentence still exists where the rule lives | Checked by search before each cut, not after. The dependency rings and the two cross-ring constraints are in `ADR-0019` lines 38-65 (including "second path to a safety decision", confirmed present); the licence allow-list is in `ADR-0015`; the four-day MSRV story is in `ADR-0026` and `E26-S04`'s packet; the `rust-toolchain.toml` argument was **moved into ADR-0028 first**, then removed from AGENTS.md. | PASS |
| AC2 - one commit per removal | Two commits, each reverting cleanly on its own. | PASS |
| AC3 - unloaded documents do not count as saved attention | This is what stopped the pass from becoming a purge. A document in `docs/` that no session loads costs nothing; deleting it destroys history to save nothing, and "never named in a review record" is not evidence of uselessness - it is what a generated file looks like by construction. | PASS |
| AC4 - nothing guarding an invariant removed | No gate, no test, no invariant, no evidence packet was touched. The only code-adjacent change is that `AGENTS.md` now points at `ADR-0019` instead of paraphrasing it. | PASS |
| AC5 - "nothing to remove" is a result | The gate set was measured and kept. A subtractive pass that must produce a subtraction is a pass that will invent one. | PASS |

## The duplication had already drifted, which is the point

`docs/ARCHITECTURE.md` was not removed because a second index *might* drift. It was removed because
it had: `docs/INDEX.md` lists `architecture/JSON_CONTRACTS.md` and `ARCHITECTURE.md` did not. Two
indexes of one directory, and the less-read one was already wrong. Its three unique contributions -
the transition warning, the reading order, and the migration-contract pointer - are now in
`INDEX.md`, which is the index everything else already links to.

## Verification Commands

```text
git show v1.11.0:AGENTS.md | wc -l     -> 234;  wc -l AGENTS.md -> 395 before, 374 after
grep -c "second path to a safety decision" docs/adrs/0019-...md  -> 1 (present before the cut)
grep -c "architecture/" docs/INDEX.md  -> 9;   docs/ARCHITECTURE.md listed 8
python3 scripts/check_docs.py check    -> 316 files, links and safety IDs consistent
python3 -m pytest tests -q             -> 521 passed
```

## Compatibility

- Documentation only. No gate, no test, no product surface, no generated artifact.

## Residual risks

- **21 lines is a small saving against a 374-line file.** The honest reading is that AGENTS.md is
  large because it is doing a large job, not because it is padded, and most of its remaining bulk
  is two command lists that a gate requires to be exactly what they are.
- **The pass was run by the author of most of the growth**, which is the worst possible reviewer of
  whether that growth was justified. The measurements are stated so someone else can disagree with
  the conclusion rather than with the taste.
- **"Never named in a review record" turned out to be a weak signal** - it flags generated files
  and heavily-linked references alike. The metric is still worth having and should not be read as a
  deletion list, which is what `process_metrics.py`'s own text already says.
- **Nothing was done about the two command lists**, which are the largest remaining blocks in the
  always-loaded file. They are a second source of truth by design, kept in lockstep by
  `check_workflows.py` because the alternative failed once (CR-TE-06). Removing them means changing
  that gate, which is a different story with a different risk.

## Verifier verdict

pending - branch work
