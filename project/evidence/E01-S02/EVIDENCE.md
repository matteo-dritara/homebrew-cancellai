# Evidence Packet - E01-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E01)
- Change Risk: CR1
- Spec version/commit: `tests/fixtures/manifest.json` + `tests/fixtures/recipes.py` as added in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Fixtures cover normal sessions, subagents, active data, protected state, partial trees, symlinks, and layout drift | `tests/fixtures/manifest.json` declares 10 fixtures across the 7 required categories (`normal_session`, `subagent_tree`, `active_data`, `protected_state`, `partial_tree`, `symlink`, `layout_drift`); `scripts/check_fixtures.py check` fails if any required category has zero fixtures (`missing_categories` check) and passes today. `tests/test_fixtures.py` additionally verifies each fixture is a *credible* layout, not just a directory that happens to exist: `fingerprint_root` recognizes every fixture as a custom-but-credible provider root; the subagent tree's two children resolve to the root by `parent_thread_id` (`read_codex_parent_session_id`); the protected-state fixtures are never selected by `build_plan` even with `aggressive=True` and a 1-day cutoff; both symlink fixtures resolve outside their own root (`is_within` is False); the partial-tree fixture produces a genuinely incomplete `Scan` from a locked directory (a locked plain file was tried first and shown not to reproduce the real "we could not look" case - see Residual risks). | PASS |
| AC2 - Fixtures contain no real user content or paths | Every recipe in `tests/fixtures/recipes.py` writes only synthetic content (literal `"synthetic-project-*"` project names, fixed UUID-shaped session ids following the existing `tests/test_cancellai.py` convention, placeholder JSON/TOML). `scripts/check_fixtures.py` materializes every fixture into a temp directory and scans every path and readable file's content against a forbidden-pattern list (home-directory-shaped absolute paths, email addresses, `sk-`/`gh*_`/`AKIA` credential shapes, PEM private-key headers) - `test_checker_flags_forbidden_content` proves the scan actually fires on a planted violation, not only that the real corpus happens to pass it. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story (CR1, no runtime code path changed - discovery/build_plan/safe_remove are exercised read-only by the new tests, never mutated).

## Verification Commands

```text
python3 -m pytest tests -v
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All passed (132 tests, 12 subtests; `fixtures OK: 10 fixtures cover all required categories`). `scripts/check_fixtures.py check` is the "fixture manifest validation" named in the story's verification contract, and it is now wired into `.pre-commit-config.yaml` (new `fixtures-check` hook) and `.github/workflows/tests.yml` (new lint-job step) so it runs the same way in CI as locally.

`pytest` was run inside a local virtualenv (`python3 -m venv` + `pip install -r requirements-dev.txt`) because the system Python 3.13 is externally managed (PEP 668) and rejects a bare `pip install`; this is a local environment detail, not a project change, and CI installs into its own ephemeral runner as `tests.yml` already did.

## Compatibility

- Fixtures are pure-stdlib synthetic filesystem trees; nothing platform-specific was added beyond what `tests/test_cancellai.py` already relies on (POSIX chmod semantics for the partial-tree fixture, matching that file's existing precedent and comment about non-root CI).

## Performance / operability

- `scripts/check_fixtures.py check` materializes and deletes 10 small synthetic trees (each a handful of KB) into a temp directory per run; negligible cost, well inside the "self-budget" spirit of C-11 (this tool does not itself become an unbounded producer).

## Documentation updated

- `tests/fixtures/README.md` - documents the manifest/recipes/checker convention and the required-category list (the story's declared documentation impact).
- `AGENTS.md` - added `scripts/check_fixtures.py` to the Python checks command list.
- `.pre-commit-config.yaml` - new `fixtures-check` local hook; added `check_fixtures.py` to the mypy hook's `files` pattern.
- `.github/workflows/tests.yml` - added `check_fixtures.py` to the lint job's mypy invocation and a new `check_fixtures.py check` step.
- `pyproject.toml` - added `scripts/check_fixtures.py` to `[tool.mypy] files`.

## Residual risks

- The content-pattern scan in `check_fixtures.py` is best-effort (documented as such in `tests/fixtures/README.md`): it catches obvious shapes (home paths, emails, common credential prefixes) but is not a guarantee against every way real content could leak into a future fixture. Human review of any fixture diff remains necessary.
- `manifest.json`'s `layout` field (`default` / `unknown_version`) is currently only descriptive metadata; nothing yet consumes it programmatically. It becomes load-bearing once E01-S03 defines the versioned plan/result schema and E01-S04 characterizes Python behavior per fixture - both explicitly out of this story's scope.
- Coverage is one fixture per category per tool where it made sense (ten fixtures total), not an exhaustive matrix of every provider/version/layout combination; broader corpus growth is expected incrementally as later E01/E02 stories need specific cases, not as a one-time exhaustive catalog here.
- The first implementation of the partial-tree fixture chmod'd a single *file* to `0o000`, which does not actually deny `lstat`/discovery in POSIX (stat requires no read permission on its target) and so produced a scan that was silently complete. `tests/test_fixtures.py::test_partial_tree_fixture_has_exactly_one_unlistable_subtree` failing during development caught this before it landed; the fixture now locks a *directory*, matching the real mechanism `tests/test_cancellai.py` already relies on. Recorded here because it is exactly the kind of "looks right, isn't" gap a reviewer should re-check independently rather than trust from this description.

## Verifier verdict

PENDING - epic E01 review runs once every story in E01 is `ready_for_review` (at most twice per epic, per ADR-0014).
