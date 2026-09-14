# Evidence Packet - E27-S03

- Commit/PR: the documentation alignment on `rework/codebase-health`
- Executor: Claude
- Independent verifier: none - branch work, CR0
- Change Risk: CR0
- Spec version/commit: `project/epics/E27.json` at this commit

## Outcome

PASS

## Drift found, by measurement rather than by reading

| Claim in a document | Reality |
| --- | --- |
| `RELEASE_GATES.md`: "Nine of thirty-one are structural" | The table already listed **16 structural and 15 behavioural**, and `check_coverage.py` made it 32 |
| `docs/INDEX.md` lists the generated measurements | `project/coverage_baseline.json` was absent from the list |
| `README.md`: macOS-only, "a single, readable, stdlib-only Python file" | True of the installed tool, and every release since v1.13.2 also attaches **four prebuilt Rust archives with SBOMs and signed provenance**, which the README never mentions |

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every gate classified, counts matching the table | `check_coverage.py check` added to the table as behavioural; the prose now says sixteen of thirty-two, counted from the table (`grep -cE '^\\| \`[^\|]+\` \\| structural'`) rather than retyped. The old "nine" was wrong about a table anyone could count. | PASS |
| AC2 - the index names the measurements | `project/coverage_baseline.json` now sits beside `PROCESS_METRICS.md` and `GATE_SENSITIVITY.md`, with the one-line reading instruction that matters: the distribution, not the average. | PASS |
| AC3 - the README describes what a release attaches | One paragraph in Platform support, saying plainly that the Rust archives are the migration target and not the product, that the cutover (E06-S04) is still open, and that `brew install` remains the supported route. They are published so the supply chain is exercised before anything depends on it. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The classification drift becoming a build failure | It was one commit away. `gate_sensitivity.py`'s `unclassified_gates()` fails on a gate the table does not classify; adding `check_coverage.py` without this story would have produced exactly that. The gate is green with the table corrected. | PASS |
| n/a | A user acting on an unsupported artifact | The README now says what the Rust archives are before a reader downloads one. Nothing about their publication changed - only whether anyone is told. | PASS |

## Verification Commands

```text
python3 scripts/check_docs.py check        -> 315 files, links and safety IDs consistent
python3 scripts/gate_sensitivity.py check  -> 11 mutants killed, no unclassified gate
python3 scripts/check_repository_topology.py check -> canonical repository unambiguous
```

## Compatibility

- Documentation only. No code, no gate behaviour, no product surface.

## Residual risks

- **This was a sweep, not a system.** Nothing compares a prose count against the table it describes;
  the next such number will drift the same way. A checker could read both, and would be one more
  gate in a repository that has thirty-two.
- **26 documents have never been named in a review record**, and this story did not touch that. It
  is the input to the subtractive pass, not something to fix by editing them.
- **The README's accuracy rests on a reading**, not a check. `check_docs.py` validates links and
  invariant IDs; nothing validates that a sentence is still true.

## Verifier verdict

pending - branch work
