# Evidence Packet - E22-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E22 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`

## Outcome

PASS

## Scope

The Rust workspace had none of the supply-chain automation the Python reference already has:
no Dependabot ecosystem entry (`serde`, `serde_json`, `unicode-normalization`, and `libc`
inside `cancellai-sealedfs` - the only crate in the workspace containing `unsafe` - never
received update proposals), and no CodeQL coverage (the authority kernel, the `sealedfs` FFI
boundary, and the provider adapters were outside static-analysis scope entirely).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - dependabot covers the cargo ecosystem | `.github/dependabot.yml` gained a `cargo` ecosystem entry with `directory: "/rust"`, the directory containing `rust/Cargo.toml` and its 13-crate workspace (including `cancellai-sealedfs`, which depends on `libc`). | PASS |
| AC2 - CodeQL analyses Rust alongside Python | `.github/workflows/codeql.yml` gained `analyze-rust`, a second job (`languages: rust`) using the same pinned `github/codeql-action/init`/`analyze` SHAs the Python job already uses, with a `cargo build --workspace` step (Rust extraction needs a real build) so the whole workspace - kernel, FFI boundary, adapters - is covered. | PASS |
| AC3 - a CodeQL finding in Rust surfaces in the same security-events channel | `analyze-rust` runs inside the same workflow file, under the same top-level `permissions: security-events: write` the Python job already relies on; no new permission block was added because none was needed. | PASS |

## Safety Evidence

Not safety-bearing (CR1: adds analysis/automation, does not change runtime behaviour).

## Verification Commands

```text
$ python3 -c "import yaml; yaml.safe_load(open('.github/dependabot.yml'))"      # parses
$ python3 -c "import yaml; yaml.safe_load(open('.github/workflows/codeql.yml'))" # parses
$ python3 scripts/check_workflows.py check
workflow policy OK: 6 workflow files use explicit permissions and immutable action SHAs
$ test -f rust/Cargo.toml && echo ok    # dependabot's cargo directory is valid
ok
```

Full local Python gate set (`pytest`, `ruff`, `check_docs`, `check_workflows`,
`check_process`, `release.py check`) re-run and green; this story does not touch the Rust
workspace's source or the differential/parity gates, so the heavier Rust-workspace-affecting
checks were not re-run beyond `cargo build --workspace` already exercised while validating
E22-S01's `verify-rust` job structure.

## Compatibility

- No product behaviour change. CI-only.

## Performance / operability

- `analyze-rust` adds one more `ubuntu-latest` job to every push/PR/weekly-scheduled CodeQL
  run; it does not affect `rust.yml`, `release.yml`, or the CLI's own runtime.

## Documentation updated

- `docs/security/SUPPLY_CHAIN.md` records both additions and what they cover.
- `.github/dependabot.yml`, `.github/workflows/codeql.yml` (the changes themselves).

## Residual risks

Both items in the story's Verification Contract require a real GitHub-hosted run and cannot
be exercised locally, the same limitation E22-S01 recorded for its own dry-run/tag checks:

- **"The CodeQL Rust job completes on a real run and reports a non-empty analysis, not a
  skipped language."** CodeQL's Rust extractor is invoked identically to how the Python job
  is already invoked (same pinned action versions), and `cargo build --workspace` was
  confirmed to succeed locally, but whether GitHub's hosted CodeQL Rust extractor actually
  analyses the built artifacts (rather than silently skipping) can only be confirmed by
  watching the Security tab / job log after this lands on `main`.
- **"A synthetic outdated pin produces a dependabot proposal."** Not exercised - forcing a
  live Dependabot run against a synthetic outdated pin requires either waiting for the
  monthly schedule or triggering Dependabot from the GitHub UI/API on the real repository,
  which is outside local executor scope. The config was validated structurally (parses,
  `directory` points at a real `Cargo.toml`) instead.

An independent verifier or the next scheduled Dependabot/CodeQL run should confirm both
empirically.

## Verifier verdict

pending
