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

## Round-6 repairs (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND6.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| `release.py check` accepted wrong-version engine URLs, corrupted digests and an extra malformed resource | `formula_engine_problems` requires exactly one well-formed resource per target, no other release-download URL, and every URL at the formula's version; `check` and `finalize` both apply it | `CutoverFormulaValidationTests` (wrong version, malformed digest, extra resource, pre-cutover formula) |
| Re-running `finalize` for v1.21.0 adopted the Rust template, and a failed post-write check left the formula changed | Adoption needs `--adopt-cutover` and is refused for any version before 2.0.0 or in `UNVERSIONED_ENGINE_RELEASES`; the finalized text is validated before an atomic write, and a failing post-write `check` restores the original | `test_finalize_never_adopts_the_cutover_without_being_told_or_for_an_unversioned_engine`, `test_a_failed_post_write_check_restores_the_formula` |
| The cutover CI job ran `version` without asserting it | `release.py verify-installed` asserts `cancellai version` = `cancellai-cli <version>` (`0.1.0` only for the one release listed as unversioned) and `cancellai-legacy --version`; the formula's own `brew test` runs for every versioned release | `tests.yml` homebrew job |
| Undisclosed: `configure` refused on Windows; help/version still presented `cancellai-cli` as a beta | Disclosed in the Unreleased notes and `docs/CLI_RUST.md`; the "target-engine beta" wording is gone from the help; a checked inventory of every shared flag, `project/cli_inventory.json`, is tested against the Python parser, the Rust help goldens and the release notes | `tests/test_cli_inventory.py` |

## Round-7 repairs (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND7.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| Fabricated digests and platform-swapped archives passed the check | Each engine resource must sit in its own platform block (`enclosing_platform`); `published_digest_problems` compares every digest with the one the release published beside its archive - in `finalize` and in CI (`release.py verify-formula`), since `check` runs offline | `test_archives_swapped_between_platform_blocks_are_reported`; `verify-formula` in `tests.yml` |
| A 2.0.0 finalize could skip the cutover; adoption replaced a partly edited formula; the runbook described automatic adoption | From 2.0.0 every release must carry the engine; `--adopt-cutover` requires E06-S04 `done` (the owner-accepted verdict) and an untouched pre-cutover formula; `docs/RELEASING.md` rewritten | `test_a_release_at_or_after_the_cutover_cannot_skip_the_engine`, `test_adoption_needs_the_owner_accepted_cutover_story`, `test_a_partly_converted_formula_is_refused` |
| The smoke test exempted the unversioned engine, skipped `brew test`, and accepted a wrong legacy version | `tests.yml` builds this commit's engine (stable channel) and source archive, installs the rendered formula, requires `cancellai-cli <v>` and `cancellai <v>` exactly, and runs `brew test`; no exemption | `test_verify_installed_demands_exact_versions`; the `homebrew` CI job |
| The CLI inventory omitted Rust-only flags | The inventory is the union of both CLIs' flags on the shared commands; `status --allow-running` and `version --source` are "added" and disclosed | `test_every_python_and_every_rust_flag_is_inventoried` |

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
