# Release Evidence - v1.8.0

## Source

- Tag: `v1.8.0`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-03

## Included work

- Epic: E21 - Target Engine Trust Remediation
- Stories: E21-S01, E21-S02, E21-S03, E21-S04, E21-S05, E21-S06, E21-S07
- CR4 Safety Verdicts: `project/evidence/E21-S03/SAFETY_VERDICT.md`, `project/evidence/E21-S03/SAFETY_VERDICT_OWNER_ACCEPTANCE.md`, `project/evidence/E21-S07/SAFETY_VERDICT.md`, `project/evidence/E21-S07/SAFETY_VERDICT_OWNER_ACCEPTANCE.md`

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

Actually run locally before tagging, beyond the list above: the full checker set
(`check_fixtures`, `check_schemas`, `characterize`, `diff_harness`, `check_rust_workspace`,
`check_mutation_boundary`, `check_provider_compatibility`, `rust_python_parity` self-test and
check, `release.py check`) and the full Rust set (`cargo fmt --check`, `clippy -D warnings`,
`cargo test --workspace`, `cargo deny check`). Results: 179 Python tests, 327 Rust tests, 12
NORMATIVE fixtures across both root-origin scenarios, all green.

### Correction, recorded after the tag

**The `rust` workflow failed at the tagged commit `b5d83bd`, on one job: `quality
(windows-latest)`.** `use cancellai_inventory::ScopeCompleteness` in both provider test modules
is reached only by the `#[cfg(unix)]` partial-scope assertions, so on Windows it is an unused
import and `clippy -D warnings` refuses it. Every other job in that run passed, on all three
platforms and both toolchains, including `cargo check --workspace --all-targets` on Windows. No
runtime behaviour is affected: it is a lint on test code, not a defect in the shipped tree.

Repaired on `main` in `0ea7984`, where all four workflows pass. The tag is not moved:
`docs/RELEASING.md` treats published tags as immutable history, and rewriting one to hide a
failed check is the opposite of what this evidence exists for.

**Why the release workflow did not catch it, and why that matters more than the lint.**
`.github/workflows/release.yml` states that it re-runs *every* gate at the tagged commit. It
does not: it runs no Rust check at all. It reported `success` for `v1.8.0` while the `rust`
workflow was failing on the same commit. That is `CR-TE-06` from
[the 2026-09-03 review](../../docs/audits/2026-09-03-CODE_REVIEW.md), filed hours earlier as an
argument and demonstrated here as an incident, on the very release that closes the epic which
found it. `E22-S01` carries the repair and now has a concrete reproduction behind it rather than
a hypothesis.

The gate results below describe the local pre-tag run. They were accurate for macOS, and the
Windows lint is the one thing they overstated.

- G1 Functional: PASS locally on macOS; `quality (windows-latest)` failed at the tag (see
  correction above), repaired in `0ea7984`
- G2 Safety: PASS **with residuals**, owner-accepted. One independent adversarial round was run
  and its findings repaired; the repairs carry no independent confirmation. See
  `project/evidence/E21-CLOSURE.md` and the two owner-acceptance verdicts.
- G3 Compatibility: PASS on macOS and Linux. Windows is unchanged and still cannot perform a
  real deletion (E03-S01 residual, E20-S01 scope).
- G4 Operability: PASS. This release is the first to measure the shipped discovery path rather
  than an unreachable traversal (E21-S05).

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

- Scan benchmarks: the shipped discovery path is now measured, per PR
  (`cancellai-cli/tests/performance_shipped_path.rs`) and on the scheduled 10k/100k datasets
  (`performance_scheduled_shipped.rs`, primary trend artifact). Smoke run at 10k: 20,000
  artifacts in 1.74s (11,506/sec) against a 60s regression threshold. The
  `cancellai-inventory` benchmarks are retained as a guard on that crate's own contract.
- Memory: rollout metadata reading is bounded and proven so by bytes actually consumed, not by a
  proxy. Measured end to end on a single 287 MB rollout: peak RSS 2.9 MB, against 303 MB before
  this release and 27.7 MB for the Python reference on the same file.
- Self-budget: recorded scan errors are bounded (`MAX_RETAINED_REASONS`, with a truthful total
  kept alongside), and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

### Fixed

- **A directory the scan could not read no longer authorizes deletion** (E21-S03, `CR-TE-01`).
  On an ordinary tree - one directory without read permission - `cancellai-cli` deleted an
  eligible artifact and exited `0` while reporting the scope complete and its knowledge
  verified; `cancellai.py` withheld every destructive action for that tool and exited `4`. Both
  provider adapters discarded every failure to observe part of the tree with a bare
  `else { continue }`. They now record each one with a cause and a path, withhold the whole
  tool, and degrade `knowledge_confidence` for every artifact in the scope (SI-008, SI-009,
  SI-010, C-02). The Claude side was broader than previously recorded: E06-S02 had repaired only
  the companion-payload branch, and an unreadable **project** directory still passed silently,
  disclosed nowhere.
- `scan_completeness[].error_count` reports the real number of unobservable paths instead of
  `u32::from(!complete)`, which was only ever `0` or `1` (`CR-TE-10`).
- **The delete path prevents the path-swap race instead of detecting it** (E21-S07, `CR-TE-05`).
  The unlink is issued through `cancellai-sealedfs`'s handle-relative `unlinkat` against a
  directory descriptor opened once with `O_NOFOLLOW` at every component, so a rename or
  symlink-swap after validation cannot redirect it. Consequence, intended and user-visible: a
  provider root reached through a symlinked path component can no longer be cleaned - the rule
  E07-S09 already set for root establishment, now holding at the moment of mutation.
  `MutationOperation`'s two unconfirmed, unreachable variants (`Quarantine`,
  `DeleteDirectoryTree`) were removed rather than left armed for E12 to inherit (`CR-TE-11`).
- Rollout metadata reading honours the 512 KiB bound it documents instead of loading the whole
  transcript (E21-S06, `CR-TE-04`). Measured on a single 287 MB rollout: peak RSS **2.9 MB**,
  against 303 MB before - and below the Python reference's own 27.7 MB.

### Added

- Two fixtures the corpus never had - `codex-partial-tree` and `claude-partial-project` - both
  `NORMATIVE` and running through the differential gate in both root-origin scenarios (E21-S02).
  They were written to **fail** against the unrepaired engine, and did; the failing run is
  committed in their evidence packet. `scripts/check_fixtures.py` now refuses an undeclared
  category asymmetry between the two reference providers, because the corpus carrying
  `partial_tree` for Claude and not for Codex is what let the gate stay green while the engine
  deleted (`CR-TE-03`).
- Scope completeness is a shared *type* obligation on every provider adapter
  ([ADR-0018](../../docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md),
  E21-S04). `ProviderResolution` hands out planning candidates only through a value carrying the
  scope's completeness, with a `compile_fail` regression proving the bare-candidates route is
  unreachable; `scripts/check_rust_workspace.py` fails if `cancellai-cli` stops being able to
  reach `cancellai-inventory` at all, which is how `CR-TE-02` went unnoticed.
- A performance gate on the discovery path the CLI actually executes (E21-S05). Every timing
  assertion is paired with an assertion on what the resolution produced, so a benchmark
  measuring an empty tree fails instead of reporting an excellent number.

### Note on how E21 was verified

Round 1 of independent adversarial review (Codex) returned `FAIL` for five of the seven stories
and reproduced a real escape the first implementation had left open: an unreadable Claude
`projects/` root was converted into a clean empty scan, so `clean --yes` exited `0` where the
frozen reference exits `4`. The completeness was computed correctly in discovery and discarded
one layer up - the same class of defect this epic exists to close. Every finding is repaired and
pinned by a regression written against the verifier's own reproduction; repairing them surfaced
one more instance of the same pattern (`Path::exists()` collapsing "not installed" into "not
readable"), also closed. The epic was closed by owner decision without spending the second
review round, which means these repairs carry no independent confirmation -
`project/evidence/E21-CLOSURE.md` records that and the residual risk it accepts.

- An independent target-engine review is committed as
  [`docs/audits/2026-09-03-CODE_REVIEW.md`](../../docs/audits/2026-09-03-CODE_REVIEW.md), with
  thirteen findings (`CR-TE-01`..`CR-TE-13`) converted into story contracts under two new
  epics rather than left as prose: **E21 Target Engine Trust Remediation** and **E22
  Engineering System Hardening**. They are new epics, not additions to E06, because E06 has
  already used both independent review rounds ADR-0014 permits.
- [ADR-0018](../../docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md):
  scope completeness becomes a shared *type* obligation on every provider adapter, while the
  adapters keep their layout-specific traversal. `cancellai-inventory`'s completeness model is
  currently unreachable from the shipped binary, which is why the defect its own reviewer
  rejected in E04-S03 reappeared in the adapters that replaced it.
- [ADR-0019](../../docs/adrs/0019-dependency-rings-per-crate.md): the safety kernel stays
  dependency-free except by dedicated ADR; the experience and persistence crates may use
  mature, licence-checked libraries. This is what will give `cancellai-cli` the `--help`/
  `--version` surface it currently lacks entirely.

### Fixed

- `project/roadmap.json` declared `current_phase: "P0"` while both P0 epics were `done` and P1
  stood one epic from closing, so `PROJECT_STATUS.md` generated a phase the project had already
  left. Corrected to `P1` (`CR-TE-12`).

### Note on scope

`cancellai.py` is unchanged by this release and remains the sole canonical, shipping engine: its
version number moves because closing an epic cuts a release (ADR-0014), not because its
behaviour did. Everything above concerns the beta Rust engine, the fixture corpus, and the gates
around them. `E06-S04` now records `E21` and `E22-S01` among its blockers; closing E21 satisfies
one of them and does not make the cutover ready.

## Known residual risks

Recorded in full in `project/evidence/E21-CLOSURE.md`, and summarised here because a release
packet that points elsewhere for its risks is not evidence:

1. **No independent confirmation of the round-1 repairs.** The verifier reproduced the defects;
   the fixes are executor work verified by executor-written tests. The second review round
   ADR-0014 permits was not spent - an owner decision, recorded rather than implied.
2. **The `fstatat`/`unlinkat` window** (E21-S07) is open by construction and documented in
   ADR-0017 rather than claimed closed.
3. **Reason retention is bounded at 64 named paths**; the reported count stays truthful.
4. **The audit that produced this epic was written by the same agent that implemented it**
   (`docs/audits/2026-09-03-CODE_REVIEW.md`). A gap it did not find is still undisclosed.
5. **E22 - Engineering System Hardening is planned and not started.** Its six stories carry the
   remaining audit findings: the release workflow still re-runs fewer gates than it claims, the
   Rust supply chain has neither dependency updates nor static analysis, the CLI still has no
   `--help`/`--version`, and `cancellai-policy`'s direct test coverage is still thin.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
