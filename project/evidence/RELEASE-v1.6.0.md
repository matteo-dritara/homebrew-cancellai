# Release Evidence - v1.6.0

## Source

- Tag: `v1.6.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-08-31

## Included work

- Epic: E05 - Provider API and Reference Adapters
- Stories: E05-S01, E05-S02, E05-S03, E05-S04, E05-S05
- CR4 Safety Verdicts: `project/evidence/E05-S02/SAFETY_VERDICT.md`

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

- Epic E05 implemented the Provider API and Reference Adapters: a nine-capability
  `ProviderCapabilities` contract (`cancellai-provider-api`) where capability absence is
  explicit and never inferred from provider identity, and every response carries evidence and
  confidence by construction; Built-in Verified/Community Verified/Local Custom/Untrusted
  provider trust wired into the Effective Authority lattice as its own constraint, gated by
  `TrustedTier`, an opaque type whose only public constructors are the safe `Untrusted` default
  and a checked, evidence-requiring promotion - closing the `ProviderTrustAuthority` gap
  `docs/architecture/DOMAIN_MODEL.md` had called out since E03-S04; Claude Code and Codex CLI
  reference adapters porting `cancellai.py`'s discovery/classification/session-relationship
  logic to Rust (root fingerprinting, the Unicode-canonical-caseless protected-name barrier,
  session/subagent-graph discovery, native-delete capability detection), each checked against
  the committed Python characterization corpus by reproducing its fixtures directly; and a
  generated, per-capability reference-provider compatibility matrix in `docs/PROVIDERS.md`. An
  independent review round found and this epic's own repair cycle closed a CR4 defect before
  close: the first version of provider trust typed its authority-lattice input as a bare,
  publicly-constructible enum, so an external caller could self-assign the highest trust tier
  with no promotion evidence at all - the exact self-assignment SI-021 prohibits. `cancellai.py`'s
  own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface
  yet.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
