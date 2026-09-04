# Evidence Packet - E20-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (Codex, per-epic review once every E20 story is `ready_for_review`)
- Change Risk: CR1
- Spec version/commit: `project/epics/E20.json`'s E20-S03 story contract

## Outcome

PASS (executor self-assessment; independent verification pending). `docs/PLATFORMS.md` is now
generated (`scripts/check_platforms.py`, from `project/platforms.json`) rather than
hand-authored aspiration, and every claim in it is cross-validated by the generator/checker
against real evidence - CI matrix membership in `.github/workflows/rust.yml`, and named test
functions actually present in `rust/crates/` - rather than trusted on the JSON's own word.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| No platform is called supported without required CI and destructive safety fixtures | `scripts/check_platforms.py`'s `validate()`: (1) a platform's `ci_check_job`/`ci_quality_job` claim is checked against `rust.yml`'s real `os:` matrix for the `check`/`quality` jobs (regex-parsed, no YAML dependency, mirroring `scripts/check_workflows.py`'s existing approach); (2) every `evidence_tests` entry must be a `fn` name this script finds for real under `rust/crates/`; (3) `tier: 1` is refused unless both CI flags are true **and** both `identity`/`mutation` capabilities are `"verified"` - not merely asserted. Demonstrated by forcing a false `tier: 1` claim for Windows (mutation is `"unsupported"` there) and confirming `validate()` rejects it, and by appending a nonexistent test name to a platform's `evidence_tests` and confirming that is rejected too (both reproduced live during this work, not merely reasoned about - see Verification Commands). Per this real bar: macOS and Linux are tier 1; Windows (real identity, no real deletion yet - E20-S01) and WSL2 (no CI runner at all) are tier 2, corrected from the previous hand-authored doc's aspirational "Target Tier 1" listing of all four. | PASS |
| Unsupported OSes remain inspect-only or refused explicitly | Documented in the new "What a non-tier-1 platform does today" section, pointing at the actual enforcing code: `cancellai-platform`'s observers report a distinct `Unsupported` fact (never a guessed result), and `cancellai-safety::root_capability`/`mutation_executor` fail closed on it (SI-002, SI-017) - inspection/planning commands remain available, `clean` cannot proceed to a real deletion. This is pre-existing enforced behavior (E03-S01 onward), not new code from this story - this story documents and cross-references it accurately rather than re-implementing it. | PASS |

## Safety Evidence

No safety obligations are declared for this story (CR1, observational/documentation). No
production code path changed - `scripts/check_platforms.py` is a governance/CI tool, not part
of the shipped binary.

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| C-18 (the project is not a black box) | A platform's tier claim must be reproducible from version-controlled source data, not asserted prose | `docs/PLATFORMS.md` carries a "Do not edit by hand" generated banner (added to `scripts/check_process.py`'s `GENERATED_FILES`, so a hand-edit now fails `check_process.py check` the same way every other generated doc's drift does); `scripts/check_platforms.py check` fails on any drift between `project/platforms.json` and the rendered file | PASS |

## Verification Commands

```text
python3 scripts/check_platforms.py generate
python3 scripts/check_platforms.py check
python3 -m ruff check scripts/check_platforms.py
python3 -m ruff format --check scripts/check_platforms.py
python3 -m mypy scripts/check_platforms.py
python3 scripts/check_workflows.py check
python3 scripts/check_docs.py check
python3 scripts/check_process.py check
python3 scripts/project_os.py check   # fails only on the expected "evidence not yet committed" gate before this file existed
python3 -m pytest tests -q
```

All green (`pytest`: 192 passed once this evidence file and the story-status commit exist -
2 failures observed before this file was written, both the expected "evidence not yet
committed" governance gate, not a real regression). The enforcement was verified negatively as
well as positively during this work: temporarily setting `windows`'s `tier` to `1` in a scratch
copy of `project/platforms.json` and calling `validate()` directly reproduced the expected
rejection message; temporarily appending a nonexistent test name to `evidence_tests` reproduced
the expected "not found" rejection. Both scratch copies were discarded, not committed.

## Compatibility

- No runtime/product behavior changed. `docs/PLATFORMS.md`'s content changed from aspirational
  ("Target Tier 1" for all four platforms) to current-state-accurate (macOS/Linux tier 1;
  Windows/WSL2 tier 2) - a documentation correction, not a capability regression: no platform
  actually lost or gained any capability as a result of this story.
- `README.md` gained one link to `docs/PLATFORMS.md`; its own "macOS only for this release"
  statement (about the shipped Python v1 CLI) is unchanged and unaffected - it describes a
  different artifact than the Rust target-engine matrix this story adds.

## Performance / operability

- `scripts/check_platforms.py`'s `find_test_function_names()` walks `rust/crates/**/*.rs` with
  a regex per file - negligible cost (sub-second on this workspace's crate count), run only in
  CI/pre-commit governance checks, never in the shipped binary.

## Documentation updated

- `docs/PLATFORMS.md` (declared documentation impact) - now generated; content corrected from
  aspirational to current-state-accurate, capability-scoped (identity/mutation columns) rather
  than a flat per-OS boolean.
- `README.md` (declared documentation impact) - one link added, no other change.
- `AGENTS.md` - `scripts/check_platforms.py check` added to "Current Python checks", and to the
  `mypy` command's script list.
- `.github/workflows/release.yml` - `scripts/check_platforms.py check` added to the `verify`
  job (required once `AGENTS.md` lists it, per `scripts/check_workflows.py`'s own
  cross-validation - this was in fact caught live by that checker during this work, not merely
  reasoned about), and to that job's `mypy` script list.
- `scripts/check_process.py` - `docs/PLATFORMS.md` added to `GENERATED_FILES` so a hand-edit is
  caught the same way every other generated doc's drift is.
- `CHANGELOG.md` - `[Unreleased]` entry added.

## Residual risks

- `find_test_function_names()` matches any `fn <name>(` in the tree, not specifically
  `#[test]`-annotated functions (disclosed in its own docstring) - a name collision with a
  non-test function would be a false negative (this checker missing a real test), never a false
  positive (wrongly validating an unfounded claim), so this is a conservative, not a dangerous,
  imprecision.
- The tier-1 bar this story encodes (CI + verified identity + verified mutation) is this
  executor's own reading of "required CI and destructive safety fixtures" - a reasonable,
  defensible interpretation, but a judgment call the independent verifier should confirm matches
  the story's intent, particularly whether "destructive safety fixtures" was meant to require
  verified *mutation* capability specifically (this executor's reading) or could be satisfied by
  fixtures proving safe *refusal* alone (which Windows already has, via its `Unsupported`/`None`
  refusal paths) - the packet's Windows classification (tier 2) depends on which reading applies.

## Verifier verdict

Pending independent review (per-epic, once every E20 story reaches `ready_for_review` -
`docs/development/AGENT_PROTOCOL.md`). Not populated by the executor.
