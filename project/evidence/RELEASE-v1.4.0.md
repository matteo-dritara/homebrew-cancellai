# Release Evidence - v1.4.0

## Source

- Tag: `v1.4.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-08-29

## Included work

- Epic: E03 - Formal Safety Kernel
- Stories: E03-S01, E03-S02, E03-S03, E03-S04, E03-S05
- CR4 Safety Verdicts: `project/evidence/E03-S01/SAFETY_VERDICT.md`, `project/evidence/E03-S02/SAFETY_VERDICT.md`, `project/evidence/E03-S03/SAFETY_VERDICT.md`, `project/evidence/E03-S04/SAFETY_VERDICT.md`, `project/evidence/E03-S05/SAFETY_VERDICT.md`

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

- Epic E03 implemented the Formal Safety Kernel: cross-platform artifact identity tokens with fail-closed Windows refusal (`rust/crates/cancellai-platform`, SI-013/SI-017); an immutable `SealedPlan` sealed only from a verified root/target capability pair, with fail-closed identity/root revalidation (SI-013/SI-016); typed `ApprovedRoot`/`BoundedPath` root and filesystem-boundary capabilities rejecting root-self deletion, escapes, symlink-escape tricks, and cross-device mounts (SI-002/SI-003/SI-018); a monotonic-minimum Effective Authority lattice with a deterministic explanation trace, collapsing unknown/active/protected/partial state to non-destructive authority (SI-001/SI-007/SI-008/SI-009); and the mutation executor itself - the sole, statically-enforced path to real filesystem deletion, checking root binding, authority, and reversibility before mutating, and confirming a plain file's identity via an open file descriptor immediately around the delete syscall (SI-013/SI-019/SI-020). An independent review round found and this epic's own repair cycle closed three CR4 defects before close: a raw mutation capability bypassing every one of the above checks, a plan executable against a target from a different root, and an executor that never consulted its own recorded authority/reversibility. `cancellai.py`'s own runtime behavior is unchanged; nothing in this epic is wired into a shipping CLI surface yet.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
