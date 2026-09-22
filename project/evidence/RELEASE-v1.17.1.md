# Release Evidence - v1.17.1

## Source

- Tag: `v1.17.1`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-22
- Published: no - verify-rust failed on cargo test --workspace on both windows-latest and ubuntu-latest (macos-latest passed); clippy itself now passed on all three platforms, confirming the service_windows.rs unused-import fix worked. Three real, platform-specific defects found: (1) KillSwitch::is_engaged trusted a bare ErrorKind::NotFound as confirmed-absent, but real Windows CI showed a path through a non-directory component also reports NotFound there, misreading a failed engage() as disengaged - repaired by requiring the marker's parent to be confirmed a real directory before trusting NotFound; (2) schtasks /Query .../XML output did not decode as UTF-8 - its own XML declares encoding="UTF-16" and schtasks.exe writes genuine UTF-16LE bytes with a BOM, which String::from_utf8_lossy silently mangled into unmatched replacement characters - repaired by detecting a UTF-16 BOM and decoding accordingly in the shared command-output decoder; (3) real Linux CI showed a reachable systemd --user session bus does not guarantee `enable --now` sees a unit file this same process just wrote ("Unit file ... does not exist" even after an explicit daemon-reload) - repaired with a bounded retry (reload, then enable, up to 3 attempts) in enable() itself, and the real smoke test's fallback recognition extended to this specific, distinguishable outcome too (run 35743679810)

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

v1.17.0's release workflow failed at verify-rust (windows-latest) on a Windows-only unused-import clippy denial in cancellai-guardian/src/service_windows.rs, and at verify on a missing recorded outcome for the already-published v1.16.0; both repaired, the tag stands as immutable history

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

### Fixed

- The Rust engine's `cancellai-guardian` crate failed real Windows CI on the v1.17.0 tag: `cargo
  clippy --workspace --all-targets --all-features -- -D warnings` denied an unused `use
  std::path::PathBuf` in `service_windows.rs`, used only by that module's own `#[cfg(test)]`
  fixtures - on a real Windows build the module is included via `target_os = "windows"` alone
  (`lib.rs`'s `cfg(any(test, target_os = "windows"))`), so the plain library target compiles it
  with those fixtures absent and the import genuinely unused there, a platform-specific lint gap
  this workspace's own tooling documentation already names and no local host could reproduce
  without a Windows cross-compiler. Gated the import to `#[cfg(test)]` to match its real usage.
  v1.17.0's tag stands as immutable history and is recorded as unpublished; this fix ships as the
  next version.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
