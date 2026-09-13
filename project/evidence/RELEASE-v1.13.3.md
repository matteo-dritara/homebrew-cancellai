# Release Evidence - v1.13.3

## Source

- Tag: `v1.13.3`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-13
- Published: pending

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

The knowledge-bundle verifier used ed25519-dalek's permissive Verifier::verify where ADR-0024 specifies verify_strict, so a weak key already present in local trust policy could authenticate forged input. Found and repaired by the independent review of E17-S08. Shipping it is the reason this version exists; the rest of the section rides along. Semver note: the MSRV raise in this section would normally argue for a minor, and this repository's own rule (E22-S07) says a release that closes no epic takes the next patch number. The rule wins: the Homebrew-installed artifact is unchanged, the Rust CLI is not yet the product, and the raised minimum reaches people building from source rather than people installing.

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

- **The workspace minimum Rust version is 1.88.0** (E17-S08, CR4;
  [ADR-0026](../../docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md), accepted). 1.85.0 was inherited
  from edition 2024 and never chosen on its own merits, and it had come to cost three things: an
  open advisory in `lru` that only `ratatui 0.30` clears, an accepted `cargo deny` waiver for an
  unmaintained `paste`, and a ban on let-chains in this workspace's own source that the safety
  kernel had already broken without anyone noticing. Building from source now needs rustc 1.88
  (released mid-2025). Nothing about the shipped binaries or the Homebrew formula
  changes.
  - **GHSA-rhfx-m35p-ff5j is closed.** `lru` 0.12.5 -> 0.18.4 through `ratatui` 0.29 -> 0.30.2,
    and `paste` leaves the graph, so `rust/deny.toml`'s ignore list is now **empty** - a
    supply-chain gate with no waivers.
  - **An intermediate upgrade resolution compiled two copies of `crossterm`** - 0.28 declared by `cancellai-tui`, 0.29 by
    `ratatui 0.30` - which means two copies of the crate owning raw mode and the event stream in
    one process. `cargo deny` only warns on duplicates. Now one.
  - Filed as CR2 and executed as CR4: clippy reads `rust-version`, so raising it enabled lints
    that had been silently skipped - five `collapsible_if` and one `manual_is_multiple_of`, three of
    the six sites in floored crates. The floor caught the story mid-flight and raised its own level. An
    MSRV bump is a code change here, not a configuration change.

### Added

- **A cut version whose release never published is now a state the repository can express**
  (E17-S10, CR1). Each release packet records `Published: yes | no | pending` with the reason when
  it failed, `release.py check` names every unpublished version, and the formula-lag rule skips one
  rather than demanding the impossible - v1.13.1 had to be finalized by hand before v1.13.2 could
  be prepared, because the tooling had no word for what had happened. Backfilling the record found
  a fourth: **v1.10.0** was cut and never published a month ago, and nobody had noticed.
- The toolchain report counts approved pack members as managed (E17-S11, CR1). It listed all eight
  approved skills as "decide or remove" in the one report the session-start ritual puts in front of
  the owner; `check` had always counted them correctly. Found by the E17-S08 independent review.

### Fixed

- Knowledge bundles now use the strict Ed25519 verification required by ADR-0024, rejecting
  weak keys/signatures without replacing the last accepted bundle (E17-S08 independent review).
- Cargo-deny now checks transitive unsoundness advisories explicitly; its default scope had
  missed `lru` even though RustSec already carried the advisory (E17-S08 independent review).

- The branch check added in 1.13.2 prints a status instead of a blank column (E26-S04, CR0).
  `gh run list --json conclusion` returns `""` rather than `null` while a run is still going, and
  jq's `//` falls back only on `null` - so every in-progress workflow reported as an empty field,
  which reads exactly like a clean sheet. A check whose failure mode is "looks fine" is worse than
  no check, which is the argument the step was added on.

## Known residual risks

No epic closes here. The story-level records are in `project/evidence/`; these are what a reader
of this tag should know.

- **E16 and E17 cannot close, and not because work is missing.** Every E16 story is `done`, but
  the epic depends on E15, which depends on E14 and E12 - twelve unimplemented stories across
  three epics of product scope. E17 depends on E06, whose `Canonical engine switch` is the
  Python-to-Rust cutover. `project_os.py check` refuses `done` in both cases, correctly: the
  dependency graph encodes real product sequencing, not paperwork.
- **E17-S07 is still `blocked`** - incident containment and signed capability downgrade, CR4, a
  genuine product feature whose own contract demands an independent Safety Verdict. It was not
  attempted at the end of a long session to reach a closure that is blocked anyway.
- **The strict-verification defect predates this session.** It shipped in E16-S02 on 9 September
  and was live in v1.11.0 - the last release that published before this week - through every
  release that followed. Nothing in this repository caught it; an independent reviewer reading
  ADR-0024 against the code did.
- **Four earlier versions were cut and never published**, now recorded as such in their own
  packets (v1.10.0, v1.12.0, v1.13.0, v1.13.1). v1.10.0 had been forgotten for five days.
- **GHSA-rhfx-m35p-ff5j is closed** and `cargo deny`'s unsound scope is explicit. The published
  explanation of why the gate first missed it was wrong and is corrected, with the original kept.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
