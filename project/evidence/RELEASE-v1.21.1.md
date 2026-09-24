# Release Evidence - v1.21.1

## Source

- Tag: `v1.21.1`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-25
- Published: yes

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

Rehearsal of the release pipeline E06-S14 rebuilt - the formula rendered from the release manifest and published as cancellai.rb, checked by verify-release on the real assets - run before the 2.0.0 cutover depends on it (E06-S16, owner decision 2026-09-25). Homebrew keeps installing the Python reference; the engine archives carry every engine and containment change since 1.21.0.

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

- The Rust engine now reports the release version it ships in (it reported `0.1.0` through
  1.21.0).

### Added

- Every release publishes `cancellai.rb`, the Homebrew formula rendered from its release manifest,
  and `release.py verify-release` checks a published release - manifest, tag, archives, provenance
  and formula - exactly as `finalize` does, without writing anything (E06-S14, E06-S16).
- `cancellai-cli containment install|list|lift` (E06-S07, ADR-0039): signed incident containment
  now reaches the live mutation path. A notice signed with the cancellAI incident-response key (or
  a publisher the owner trusts) caps a provider at observe or recommend, and `plan`/`clean` obey
  it, re-checking immediately before every deletion. Only `containment lift --confirm` removes one.
- `cancellai-cli containment refresh` (E33-S01): fetches the published cancellAI containment
  notice with the system `curl` (https only, 256 KiB cap) and installs it through the same
  verified path as `containment install`; an unavailable or unusable feed changes nothing.

- `cancellai-cli clean` accepts `--verbose`, which lists each performed action, and
  `--keep-claude-history`, for scripts written against the reference CLI (E06-S10). The Rust
  engine never rewrites Claude's `history.jsonl`, with or without the flag; without it, a
  run that deleted a Claude artifact now says so (on stderr for `--json`).

### Changed

- The build's release channel now bounds what `cancellai-cli` may do (SI-030): only a
  stable-channel (or beta) build deletes. A build compiled without `CANCELLAI_CHANNEL=stable` -
  any local `cargo build` - plans every deletion as an observation and exits 4. Release archives
  are built as stable and are unaffected.

### Fixed

- `cancellai-cli` refuses `--days 0` as a usage error, as the reference does; it used to accept it
  and plan every session for deletion.
- `cancellai-cli clean` now deletes on Windows (E06-S13). The Windows identity-confirmed delete
  primitive existed since E20-S05, but the safety executor still refused every Windows target, so
  every planned deletion there was safely skipped. Only plain files are admitted, as on Unix;
  directories and reparse points stay refused.
- `cancellai-cli clean --json` always prints a JSON document (E06-S11). It used to print a plain
  sentence when no deletion was planned, and a human summary on `--dry-run`, which broke any
  script parsing its output on exactly the runs that did nothing - including the ones where
  safety withheld the work.
- `cancellai-cli` no longer treats a Codex rollout it cannot read as a session with no parent:
  the failure now makes the Codex scan incomplete, which withholds every deletion for the tool,
  as the reference does (E06-S12). Before, the scan reported complete and the rollout could be
  planned for deletion.
- A Claude session companion directory with many unreadable entries no longer buffers every
  failure in memory before the scan's bounded reason log applies (E06-S12).

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.

## Rehearsal verification (E06-S16)

Release workflow run `36069385676` on tag `v1.21.1` (commit `db58705`): all eleven jobs succeeded -
`verify`, `verify-rust` on three platforms, `build-artifacts` on four targets,
`attestation-verify`, `release-manifest-generate`, `publish` - and published `cancellai.rb` with the
four archives, their sidecars and SBOMs, and `release-manifest.json`.

`python3 scripts/release.py verify-release --version 1.21.1`, run on 2026-09-25 against the
published assets, on the first attempt:

```text
verified release-manifest.json: exactly what release.yml writes for v1.21.1
verified archives: 4 downloaded, hashed and provenance-verified, one run, the tag's successful release run
verified cancellai.rb: byte-identical to the Python-only formula rendered from them
```

`finalize --version 1.21.1` then wrote `Formula/cancellai.rb`, byte-identical to the published
`cancellai.rb` asset (`cmp`).
