# Release Evidence - v1.11.0

## Source

- Tag: `v1.11.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-08

## Included work

- Epic: E23 - Release gate history availability
- Stories: E23-S01
- CR4 Safety Verdicts: `project/evidence/E23-S01/SAFETY_VERDICT-ROUND2.md`, `project/evidence/E23-S01/SAFETY_VERDICT.md`

## Note on why this release exists

The `v1.10.0` tag (E20 - Windows and WSL Native Support) was committed, tagged, and pushed, but
its `verify` job failed (`gh run 34252829459`): a shallow checkout left every platform's
`verified_commit` unresolvable to `check_platforms.py`'s ancestry check, so the GitHub release
for `v1.10.0` was never published. `v1.10.0` remains as a real, immutable git tag (never
deleted, per this document's own Rollback section) but has no corresponding GitHub Release;
`v1.11.0` is the first tag to actually publish, and includes every commit `v1.10.0` did plus
this release's own E23-S01 fix.

## Note on how E23-S01 was verified

Independent verifier review round 1 (Codex, `project/evidence/E23-VERIFIER-REVIEW.md`) returned
`FAIL`: `release_history_gate_errors()` inspected only the `verify` job's first
`actions/checkout` step, so a workflow with a full-history checkout at an isolated `path:`
followed by a second, default shallow checkout into the actual job workspace bypassed it while
the ancestry gate still ran shallow. The repair (`project/evidence/E23-S01/ROUND2-REPAIR.md`)
tracks every checkout step's target directory and depth in file order and resolves the depth of
whichever directory the gate's own step actually runs in, with 7 new adversarial multi-checkout
tests including round 1's exact bypass. This was then closed by owner decision without spending
the second independent review round ADR-0014 permits - the same pattern v1.8.0's E21 and
v1.9.0's E22 closures used - so this repair carries no independent re-confirmation beyond the
executor's own re-run of every gate round 1 specified, plus adversarial self-testing against
round 1's exact finding; see `project/evidence/E23-S01/SAFETY_VERDICT-ROUND2.md`'s "Known
residual risks" for what that does and does not cover.

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

- `.github/workflows/release.yml`'s `verify` job now checks out full git history
  (`fetch-depth: 0`) instead of GitHub Actions' default shallow checkout, so
  `scripts/check_platforms.py check`'s ancestor validation
  (`git merge-base --is-ancestor <verified_commit> HEAD`) can find each platform's
  `verified_commit` object at all. A shallow checkout made an older `verified_commit`
  entirely absent from the tagged commit's local history (not merely unreachable), which
  failed the `v1.10.0` tag's release workflow after the tag had already been pushed - the
  GitHub release was never published (`gh run 34252829459`). `scripts/check_workflows.py`
  now fails statically if the checkout step in the job running that provenance gate reverts
  to anything but `fetch-depth: 0` (E23-S01).

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
