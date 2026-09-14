# Release Evidence - v1.14.0

## Source

- Tag: `v1.14.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-14
- Published: pending

## Included work

- Epic: E27 - Codebase Health
- Stories: E27-S01, E27-S02, E27-S03, E27-S04, E27-S05, E27-S06, E27-S07
- CR4 Safety Verdicts: `project/evidence/E27-S01/SAFETY_VERDICT.md`, `project/evidence/E27-S06/SAFETY_VERDICT.md`

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

- **Three `unsafe` blocks deleted rather than documented, and Miri run for the first time**
  (E27-S06, CR4). The independent review of E27-S01 found two of the executor's `SAFETY` arguments
  false - `FILE_STANDARD_INFO` was described as having no validity invariant, and in `windows-sys`
  0.61 its `DeletePending`/`Directory` fields are Rust `bool`, which has one. The conclusion held;
  the reason did not, and a reason is what the next reader extends. Underneath sat the better
  answer: `windows-sys` derives `Default` on that struct, so **41 `unsafe` blocks became 38** and a
  soundness argument that depended on a dependency's field types - and would have changed silently
  under `cargo update` - went with them.
  - **Miri: 197 tests across four crates execute with no undefined behaviour**, including the
    parsing code that reads third-party provider manifests. It runs weekly, not per-PR:
    `cancellai-tui` alone takes 333 seconds under the interpreter.
  - **And it helps least exactly where the risk is highest.** Miri cannot call foreign functions it
    has no shim for, so it cannot execute a single one of the 38 remaining `unsafe` blocks -
    `cancellai-sealedfs` and `cancellai-platform` stop at `statfs`, `cancellai-policy` at
    `fsetattrlist`, and `cancellai-safety` inside `sha2`'s aarch64 SHA-512 intrinsics. Recorded as
    a measured reach rather than left as "Miri never ran".

### Security

- **Recursive deletion now refuses where the platform cannot resist a symlink race** (E27-S04,
  CR3). The shipped Python reference deletes directories with `shutil.rmtree`, and every
  containment check before that call is path-based - a path re-checked immediately before use
  cannot close a race, because once the walk starts a subdirectory can be swapped for a symlink
  pointing anywhere. Python closes that only where the platform supports fd-relative removal and
  reports it as `shutil.rmtree.avoids_symlink_attacks`; **nothing here had ever read that flag**.
  On macOS it is `True`, so the shipped tool was protected by a property of the platform rather
  than by a decision. It is now a stated precondition: where the platform cannot remove safely,
  `clean` refuses the directory and says why, rather than removing it anyway. Behaviour on macOS
  and Linux is unchanged, which the committed characterization and the Python/Rust differential
  parity both confirm. This is the same gap
  [ADR-0017](../../docs/adrs/0017-sealed-root-handle-for-configuration-writes.md) built
  `cancellai-sealedfs` to close in the Rust engine.

### Added

- **Coverage is now a ratchet in the crates where falling matters** (E27-S02, CR1). Per-crate
  region coverage is recorded in `project/coverage_baseline.json` and
  `scripts/check_coverage.py check` fails when a kernel-ring crate covers less than it already did.
  Read the distribution, not the average: the workspace sits at 94.54%, `cancellai-safety` at
  98.23%, and **`cancellai-sealedfs` - the crate holding every `unsafe` block and the mutation
  boundary - at 92.54%**, the lowest of the gated set. `cancellai-guardian`'s 0% is correct and
  deliberately not gated: it is a sixteen-line skeleton that prints "not yet implemented".

### Changed

- **The Rust lint policy now states this project's thesis in a form the compiler checks**
  (E27-S01, CR4; [ADR-0028](../../docs/adrs/0028-lint-policy-states-the-safety-thesis-and-differs-by-ring.md)).
  `[workspace.lints]` held one line - `unsafe_code = "forbid"` - so `cargo clippy -D warnings` ran
  the default set and nothing else, in a workspace whose whole argument is that a tool deleting
  files must never act on an unproven assumption. Measured before changing anything: **41 `unsafe`
  blocks against 30 `SAFETY:` comments**, and **13 panic sites in production code**, six of them
  indexing strings that arrive from third-party provider manifests.
  - All 41 unsafe blocks now carry a written safety argument, enforced by the compiler and
    asserted by a test that does not depend on clippy. **Seven of the eleven undocumented ones
    exist only behind `cfg(windows)`** and no single-platform run could have found them.
  - Two of the thirteen panics were real rather than theoretical: a plan action naming no target
    artifact aborted the CLI part-way through a run, and the Windows rename path inside the
    mutation boundary sliced a buffer whose size was computed a few lines earlier.
  - The glob matcher that reads provider manifest patterns is panic-free by construction and is
    checked against a naive reference implementation over every pattern and input up to length
    four across an alphabet containing multi-byte characters - the differential method this
    repository already uses between its Python reference and its Rust port, applied to a parser
    that had never been tested that way.
  - **A structural defect the measurement exposed:** Cargo's `[lints]` table is all-or-nothing, so
    `cancellai-sealedfs` - which must declare its own to lift `forbid` - was the one crate a
    workspace policy could never reach. Every `unsafe` block in the repository sat in the only
    crate exempt from the rule about `unsafe` blocks. A gate now refuses any workspace lint a
    crate drops.
  - First coverage measurement in the project's history: **94.54% of regions**, with the worst
    kernel-ring file being `cancellai-sealedfs/src/lib.rs` at 92.57%.

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
