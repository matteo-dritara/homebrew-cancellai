# Contract Fixtures

This directory becomes the normative cross-engine behavior corpus during P0/P1.

Fixtures are **synthetic**. Never commit real Claude/Codex transcripts, prompts, source code, credentials, absolute personal paths, or copied provider state.

## Fixture corpus (E01-S02)

Fixtures are generated from small recipes rather than committed as raw filesystem data:

- `manifest.json` - one entry per fixture: `id`, `tool` (`claude`/`codex`), `category`, `layout`, `description`.
- `recipes.py` - one `build_*` function per fixture, keyed by id in `recipes.FIXTURES`. Each materializes a synthetic provider-root tree at a path the caller supplies.
- `../../scripts/check_fixtures.py check` - validates the manifest against the recipes: every entry resolves to a real recipe, every recipe has a manifest entry, the required categories are all covered, and every generated tree is scanned for content that looks like a real path, email address, or credential.

Required categories (each must have at least one fixture): `normal_session`, `subagent_tree`, `active_data`, `protected_state`, `partial_tree`, `symlink`, `layout_drift`.

`tests/test_fixtures.py` additionally verifies each fixture against the real reference implementation - `fingerprint_root` recognizes every fixture as a credible provider root, the subagent tree's children resolve to their root by `parent_thread_id`, protected names are never selected even under `--aggressive` at a 1-day cutoff, the symlink fixtures actually resolve outside their root, and the partial-tree fixture produces a genuinely incomplete scan (a locked *directory*, not merely an unreadable file - `lstat` on a single unreadable file still succeeds, so only a directory that cannot be listed reproduces the real "we could not look" case).

The content scan in `check_fixtures.py` is a best-effort guard, not a guarantee: it catches the obvious cases (home-directory-shaped paths, emails, common API-key shapes) but is not a substitute for reviewing a diff before it lands.

## Python behavior characterization (E01-S04)

`characterization/<fixture-id>.characterization.json` records what `cancellai.py` actually does on each fixture - the normalized `plan_summary_dict`/`coverage_payload` output, run with the fixture patched in as the *default* provider root (see `scripts/characterize.py` for why: a non-default root is always inspection-only under ADR-0013, which would otherwise mask everything else this corpus exists to show) - plus a reviewed classification: `NORMATIVE`, `INTENTIONAL_DIVERGENCE`, `LEGACY_ONLY`, or `KNOWN_DEFECT` (see [`../../docs/development/VERIFICATION_STRATEGY.md`](../../docs/development/VERIFICATION_STRATEGY.md#python-reference-contract)).

```sh
python3 scripts/characterize.py check      # verify the committed records still match a fresh run
python3 scripts/characterize.py generate   # regenerate them after a reviewed, intentional change
```

Only `NORMATIVE` records bind the Rust candidate at the differential gate (M6 in
[`../../docs/development/MIGRATION_PYTHON_RUST.md`](../../docs/development/MIGRATION_PYTHON_RUST.md)). A classification is a human judgment recorded in `scripts/characterize.py`'s `CLASSIFICATIONS` table, not something inferred from the output automatically.

## Contract

Each fixture added by E01 should contain enough declarative metadata to describe:

- provider and provider-layout/version assumption;
- filesystem tree and platform semantics;
- observation/scan completeness;
- expected discovered artifacts and relationships;
- expected classifications/confidence;
- expected plan/actions or explicit safety block;
- relevant Safety Invariant IDs;
- expected diagnostics/exit semantics.

The same fixture corpus will be consumed by the frozen Python reference and the Rust engine. Differences require either a defect fix or an explicit contract-change record; they must not be normalized away in the comparator.

Large synthetic fixtures should be generated deterministically from small recipes rather than committed as multi-gigabyte blobs.
