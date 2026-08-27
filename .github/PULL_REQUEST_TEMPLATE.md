## What this changes and why

## Checklist

- [ ] `python3 -m pytest test_cancellai.py -v` passes
- [ ] `ruff check .` and `ruff format --check .` pass
- [ ] `mypy cancellai.py scripts/gen_docs.py` passes
- [ ] `python3 scripts/gen_docs.py --check` passes (only relevant if `build_parser()` changed)
- [ ] Added/updated tests if this touches `safe_remove`, `validate_config_root`,
      `choose_old_sessions`/`choose_codex_old_sessions`, or a protected-name set
- [ ] Added a line under `## [Unreleased]` in `CHANGELOG.md` (if user-visible)
