# Release Evidence - v2.0.0

## Source

- Tag: `v2.0.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-25
- Published: pending

## Included work

- Epic: E06 - Rust CLI Parity and Cutover
- Stories: E06-S01, E06-S02, E06-S03, E06-S06, E06-S07, E06-S08, E06-S09, E06-S10, E06-S11, E06-S12, E06-S13, E06-S04, E06-S05, E06-S14, E06-S15, E06-S16
- CR4 Safety Verdicts: `project/evidence/E06-S06/SAFETY_VERDICT.md`, `project/evidence/E06-S07/SAFETY_VERDICT.md`, `project/evidence/E06-S12/SAFETY_VERDICT.md`, `project/evidence/E06-S13/SAFETY_VERDICT.md`, `project/evidence/E06-S04/SAFETY_VERDICT.md`, `project/evidence/E06-S14/SAFETY_VERDICT.md`, `project/evidence/E06-S15/SAFETY_VERDICT.md`

## Gates

Re-run at the tag by `.github/workflows/release.yml`; run locally before tagging:

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py \
  scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py scripts/release.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

- G1 Functional: PASS
- G2 Safety: PASS
- G3 Compatibility: PASS
- G4 Operability: PASS

## Compatibility

- Platforms: macOS. Python 3.10 and 3.14 exercised in CI.
- Providers/capabilities: Codex CLI and Claude Code, layouts observed at release time.
  Unclassified entries are reported by `status --coverage` and never cleaned.
- State/schema migrations: none. The tool keeps no persistent state.

## Supply chain

- Checksums: the Homebrew formula records the SHA-256 of the tag archive, written by `scripts/release.py finalize`.
- SBOM: not produced at this stage. The shipped tool has no runtime dependencies; development tooling is pinned in `requirements-dev.txt`.
- Provenance/attestation: deferred to E17.
- Signature verification: deferred to E17.
- Release manifest: this file.

## Install smoke tests

- Homebrew: `brew audit --strict` and `brew style` run in CI on every change; `brew install`/`brew test` exercise the tagged archive.
- direct shell / PowerShell / Linux packages: not applicable at this stage.

## Performance

- Scan benchmarks: none formalised; deferred to E10.
- Self-budget: recorded scan errors are bounded, and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

### Changed

- **`cancellai` is now the Rust engine** (E06-S04, the canonical switch). The Homebrew formula
  installs the signed release binary for the platform as `cancellai`; the frozen Python reference
  stays installed as `cancellai-legacy` until and including 2.1.0, as the immediate rollback.
  Intentional differences from the Python CLI, each disclosed in `docs/CLI_RUST.md`:
  - new commands: `inspect`, `plan`, `update --check`, `containment`, `desktop-api`;
  - new flags on shared commands: `status --allow-running`, `version --source`;
  - removed: `--aggressive`, `status --paths`/`--coverage`/`--top`, `--codex-backend` (on `status`
    and `clean`) - refused
    as usage errors; the Codex native delete backend is not used;
  - `--json` prints the documented `JSON_CONTRACTS.md` documents, not the Python shapes;
  - `clean` never rewrites Claude's `history.jsonl` (it says so), never deletes a file with more
    than one hard link, and obeys signed incident containment and the release channel;
  - `configure --claude-retention` is refused on Windows (no verified handle-bound settings
    write there yet); it works as before on macOS and Linux;
  - `status --json` and `clean --json` print the documented documents (see above);
  - `cancellai version` prints `cancellai-cli <version>` and the help names the program
    `cancellai-cli`, the engine's own name; `cancellai-legacy --version` prints the Python one;
  - Windows is supported natively.
  The full, checked inventory of every shared flag is `project/cli_inventory.json`.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
