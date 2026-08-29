# Release Evidence - v1.5.0

## Source

- Tag: `v1.5.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-08-29

## Included work

- Epic: E04 - Single-Pass Inventory Engine
- Stories: E04-S01, E04-S02, E04-S03, E04-S04
- CR4 Safety Verdicts: none

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

- Epic E04 implemented the Single-Pass Inventory Engine: `FileFacts`, a per-path evidence record (`rust/crates/cancellai-inventory`) composed from independently-observed logical size, allocated/physical size (a new `AllocationObserver` platform seam distinguishing sparse/cloned/compressed allocation from logical length), identity, and filesystem-boundary facts, with every unsupported metric an explicit typed value rather than a fabricated zero or borrowed metric; `scan_scope`, a single recursive walk per scope whose status/top-consumers/planning report views are pure reads over one snapshot, never a re-walk, and which never follows a symlink or descends across a device/filesystem boundary (SI-018); scope-level completeness classification (`Complete`/`Partial`/`Unknown` with named permission/I/O/disappearance/unsupported-feature reasons, SI-008/SI-009) that a planning-facing view cannot be obtained without, enforced by construction and by a `compile_fail` regression proving the bare-candidates accessor is unreachable outside the crate; and a performance baseline (a CI microbenchmark plus scheduled 10k/100k/1M-entry benchmarks with a machine-readable trend artifact). An independent review round found and this epic's own repair cycle closed a CR3 defect before close: a `read_dir`-listed entry's unreadable/vanished observation was silently dropped instead of degrading scope completeness, and the bare planning-candidates accessor was reachable without the completeness it should always carry. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
