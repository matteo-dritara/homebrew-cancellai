# Evidence Packet - E06-S04

- Commit/PR: the E06-S04 commits on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit; ADR-0039; owner decisions 2026-09-24 (Rust = `cancellai`, Python = `cancellai-legacy` through 2.1.0)

## Outcome

PASS (pending the independent pass and the owner's migration Safety Verdict)

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - owner-visible migration Safety Verdict accepted | To be written by the independent verifier and accepted by the owner in `project/evidence/E06-S04/SAFETY_VERDICT.md`; the executor does not write it. | PENDING |
| AC2 - Python stays tagged/archivable for at least one transition window | The cutover formula installs `cancellai.py` as `cancellai-legacy` (owner decision: through 2.1.0); every release tag carries the Python source. | PASS |
| AC3 - release notes state every intentional contract change | `CHANGELOG.md` Unreleased "Changed" lists the switch and each divergence; `docs/CLI_RUST.md` carries the detail. | PASS |
| AC4 - cutover checklist blockers closed or accepted, or a perimeter decided by ADR | ADR-0039 decided the perimeter; E06-S06..S13 closed every open item; `docs/development/RELEASE_GATES.md` "Update 2026-09-24" records each gate against its evidence. | PASS |

## What the switch changes, and how it is verified

| Change | Evidence |
| --- | --- |
| The Rust engine reported `0.1.0` in every release through 1.21.0 | `release.py` moves `rust/Cargo.toml`, internal path-dependency requirements and lockfile members with the source version (`set_engine_version`); `check` reports drift; `release.yml` refuses a tag that disagrees with the engine version; the workspace is 1.21.0 now. Tests: `EngineCutoverTests`. |
| The formula installs the Rust binary as `cancellai` | `packaging/cancellai.rb.template`: per-platform engine resources plus `cancellai.py` as `cancellai-legacy`. `finalize` adopts it only while the live formula has no engine resources; `point_engine_resources` refuses any other set of targets. `brew style` and `brew audit --strict` pass locally on the rendered formula. |
| A broken install must not reach users | `tests.yml` renders the cutover formula against the latest published release, taps it and runs `brew install`, `cancellai version` and `cancellai-legacy --version` on every change. |
| The switch must not ship before the verdict | The live `Formula/cancellai.rb` has no engine resources (`test_the_live_formula_has_no_engine_yet`); the template is adopted only by `finalize` of the cutover release. |

## Verification (native reproduction per platform, E21-S02 partial-scan fixtures)

`rust_python_parity.py check` runs the partial-scan fixtures (`codex-partial-tree`,
`claude-partial-project`, `claude-partial-tree`, `codex-unreadable-rollout`) in both root-origin
scenarios on every change, on the stable-channel build; Linux and macOS in CI, Windows through the
stable-channel CLI suite (E06-S13).

## Residual risks

- The formula covers Homebrew on macOS (arm64, Intel) and Linux x86_64; Linux arm64 users have no
  engine archive (none is built) and would fail at install.
- The first real adoption of the template happens at the 2.0.0 `finalize`; CI proves the rendered
  formula installs, not the adoption commit itself.

## Verifier verdict

pending
