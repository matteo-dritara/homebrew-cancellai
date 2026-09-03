# Evidence Packet - E21-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR2
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`, findings `CR-TE-01`, `CR-TE-03`

## Outcome

PASS

## Scope

Adds the two partial-scan fixtures the corpus never had, and the rule that stops the same hole
from re-forming. Ran deliberately **before** E21-S03: a fixture written after the repair proves
only that the repair is self-consistent.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a `codex-partial-tree` fixture exists, is NORMATIVE, and holds a rollout inside a directory the scan cannot list | `tests/fixtures/recipes.py::build_codex_partial_tree`: two readable rollouts plus a third under `sessions/2026/05/02`, chmod `0o000`. Classified NORMATIVE in `scripts/characterize.py`. The committed record pins the reference's own verdict: `scan.complete: false`, `withheld_tools: ["codex"]`, `actions: 0`. | PASS |
| AC2 - a `claude-partial-project` fixture exists, is NORMATIVE, and holds a session inside an unreadable *project* directory | `build_claude_partial_project`: two readable sessions in one project plus a third under a chmod `0o000` project directory. This is the branch E06-S02's companion-payload repair never covered - `discover_claude_sessions` reaches it through a separate `project_dir.iterdir()`. Committed record: `scan.complete: false`, `withheld_tools: ["claude"]`. | PASS |
| AC3 - `check_fixtures.py` fails on an undeclared category asymmetry | `_check_category_symmetry`. Proven in three directions: removing the `subagent_tree` declaration fails; removing the `codex-partial-tree` fixture (the corpus state before this story) fails; a declaration the corpus has outgrown fails as stale. Declarations carry `kind` (`structural` vs `tracked_gap`) and a reason of real length, so an unexamined hole cannot read like a decision. | PASS |
| AC4 - both fixtures run through the parity gate in both root-origin scenarios | `scripts/rust_python_parity.py check` reports 12 NORMATIVE fixtures across both scenarios, up from 10. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | A Codex rollout inside a directory the scan cannot list | `codex-partial-tree`'s committed characterization records the reference withholding the whole tool | PASS |
| SI-009 | A Claude session inside a project directory the scan cannot list | `claude-partial-project`'s committed characterization records the same | PASS |
| SI-010 | Errors visible rather than collapsed | Both records carry the concrete unreadable path under `scan.unreadable` | PASS |

## Verification Commands

The story's contract requires the fixtures to **fail** against the unrepaired engine. Captured
before E21-S03 existed:

```text
$ python3 scripts/rust_python_parity.py check
rust/python parity gate FAILED: 4 unexplained divergence(s):
  - codex-partial-tree [default]: candidates: python=frozenset() vs
      rust=frozenset({'8888...881', '8888...882'}); withheld: python=True vs rust=False;
      scan_complete: python=False vs rust=True
  - codex-partial-tree [custom]:  scan_complete: python=False vs rust=True
  - claude-partial-project [default]: candidates: python=frozenset() vs
      rust=frozenset({'9999...991', '9999...992'}); withheld: python=True vs rust=False;
      scan_complete: python=False vs rust=True
  - claude-partial-project [custom]: scan_complete: python=False vs rust=True
```

That is the audit's finding restated as a test the repository owns: the Rust engine proposing
deletion of the readable artifacts while the frozen reference withholds the tool entirely.

After E21-S03:

```text
$ python3 scripts/rust_python_parity.py check
rust/python parity OK: 12 NORMATIVE fixture(s) match across engines, in both root-origin scenarios
$ python3 scripts/check_fixtures.py check      fixtures OK: 12 fixtures cover all required categories
$ python3 scripts/characterize.py check        characterization OK: 12 fixtures match
```

## Compatibility

- The `chmod(0o000)` mechanism follows the existing `claude-partial-tree` convention, including
  the caller's obligation to restore permissions before removing the tree.
- `chmod(0o000)` denies a non-root reader only. Every consumer of these fixtures restores
  permissions in a `finally`; the Rust-side unit tests added by E21-S03 additionally *skip
  loudly* when the process can still read a `0o000` directory, so a root run cannot pass them
  for the wrong reason.

## Performance / operability

- Two more fixtures cost two more comparisons per scenario in the parity gate; runtime is
  unchanged in practice.

## Documentation updated

- `tests/fixtures/README.md` - the category-symmetry rule and the `structural`/`tracked_gap`
  distinction.
- `docs/development/VERIFICATION_STRATEGY.md` - corpus coverage as part of the gate, and the
  rule that a defect fixture must fail before the repair.

## Residual risks

- The corpus still carries three declared asymmetries. `subagent_tree` is structural; `active_data`
  (no Codex fixture) and `layout_drift` (no Claude fixture) are declared `tracked_gap` rather than
  closed, because closing them is not this story's scope. They are now visible in the manifest
  instead of invisible in the corpus.
- Category symmetry is a coverage heuristic, not a proof that every safety invariant is
  exercised on every provider. It is the cheapest rule that would have caught `CR-TE-03`, not a
  complete answer to it.


## Round-1 independent review: PASS_WITH_RESIDUALS

The verifier confirmed both fixtures are `NORMATIVE`, cover both root origins, and reproduce all
four recorded divergences against a baseline engine. It was blocked only by its failed dependency
E21-S01, now repaired.

One methodological note the verifier recorded and this packet adopts: reverting only the two
`session.rs` files no longer compiles, because E21-S04 changed their public result types. The
reproducible counterfactual is therefore "build `c00f16f` with the current fixture corpus and
characterization map", which is what the verifier ran and what any future re-check should run.

## Verifier verdict

`PASS_WITH_RESIDUALS` (round 1) - see project/evidence/E21-VERIFIER-REVIEW.md
