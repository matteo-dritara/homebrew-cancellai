# Release Evidence - v1.10.0

## Source

- Tag: `v1.10.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-08

## Included work

- Epic: E20 - Windows and WSL Native Support
- Stories: E20-S01, E20-S02, E20-S03, E20-S04, E20-S05
- CR4 Safety Verdicts: `project/evidence/E20-S01/SAFETY_VERDICT-ROUND2.md`, `project/evidence/E20-S01/SAFETY_VERDICT.md`, `project/evidence/E20-S05/SAFETY_VERDICT.md`

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

### Added

- `cancellai-cli`'s target-engine Rust core now observes real Windows file/volume identity
  (`GetFileInformationByHandle`, via a new `windows-sys`-backed capability in
  `cancellai-sealedfs`) instead of reporting every Windows path as identity-`Unsupported`
  (E20-S01, [ADR-0020](../../docs/adrs/0020-windows-native-identity-via-windows-sys.md)). A
  Windows reparse point (symlink, junction, or any other reparse tag) is classified from its
  own attributes and never treated as, or compared using, Unix symlink semantics; the
  `cancellai-safety` filesystem/volume-boundary check (SI-018) now enforces a genuine Windows
  volume boundary instead of refusing unconditionally. As a direct consequence,
  `cancellai-inventory`'s scanner can now descend below a scope root on Windows (previously an
  accepted limitation, E20-S04). Allocated-size reporting, `cancellai-sealedfs::SealedRoot`'s
  no-follow root-establishment walk, Windows process observation, and real Windows file
  deletion were deliberately out of this story's scope and remained unimplemented at the time
  (see the E20-S05 entry below, which closes them).
- `cancellai-platform` now detects a WSL2 runtime environment explicitly (rather than treating
  it as generic Linux) and classifies a path's filesystem context as native Linux, a
  Windows-drive mount (`drvfs`, e.g. `/mnt/c`), or an unrecognized `Other` mount (E20-S02).
  Library-level only for now - no CLI surface yet, and no change to mutation/quarantine
  authority (the existing Unix device-identity boundary check already refuses crossing into a
  `/mnt/c`-style mount, since it is genuinely a different device).
- `docs/PLATFORMS.md` is now generated (`scripts/check_platforms.py`, from
  `project/platforms.json`) rather than hand-authored aspiration: each platform's tier is
  cross-validated against `.github/workflows/rust.yml`'s real CI matrix and against named test
  functions this script confirms actually exist, so a platform cannot be declared tier 1
  without CI and verified destructive-mutation evidence (E20-S03). Per that real bar, macOS,
  Linux, and (since E20-S05) Windows are tier 1; WSL2 (no dedicated CI runner) is tier 2.
- `cancellai-cli`'s target-engine Rust core now implements the remaining Windows capabilities
  E20-S01 scoped out (E20-S05): real running-process observation
  (`CreateToolhelp32Snapshot`, replacing an unconditional "incomplete" result), real
  allocated-size reporting (`GetFileInformationByHandleEx(FileStandardInfo)`, distinct from
  logical size), a verified no-follow, handle-relative root-establishment walk in
  `cancellai-sealedfs` for Windows (`NtCreateFile` via `OBJECT_ATTRIBUTES.RootDirectory`, the
  Windows analogue of `openat`, used by `configure` and by `clean`'s default-root
  re-verification), and real, identity-confirmed Windows file deletion
  (`FILE_DISPOSITION_INFO`/`SetFileInformationByHandle`, following the same open/re-check/
  handle-relative-delete/post-delete-corroboration shape ADR-0017/E21-S07 established on Unix,
  with the atomic-rename step using the native `NtSetInformationFile` rather than the Win32
  `SetFileInformationByHandle` wrapper - the latter does not reliably honor a non-null
  `RootDirectory` for handle-relative rename). Confirmed on real Windows CI (ADR-0020's own
  dated investigation record); `docs/PLATFORMS.md` now records Windows mutation as `verified`
  and the platform as tier 1, alongside macOS and Linux.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
