Review-Scope: epic
Round: 11
Verifier: Codex
Date: 2026-09-24
Review target: `0afe3ad..2414975a40538cbda0d4a60f59945906d2eba062`

# E06 formal verifier review, round 11

The round-11 harness names E06-S14, E06-S15, and E33-S01. At entry,
E06-S14 and E33-S01 were `ready_for_review`; E06-S15 was already `done`
and is re-judged here as the harness requires. E06-S04 remains blocked on
E06-S14. No production code was changed and no commit or release was made.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S14 | FAIL | `tests/test_round11_adversarial.py` supplies a complete, schema-valid published manifest with two differently named artifacts carrying `aarch64-apple-darwin`. The three expected archives, digests, formula bytes, source commit, and mocked provenance all agree. `adoptable_formula` returns the formula instead of refusing; the independent regression fails at `ReleaseError not raised`. `manifest_digests` counts expected artifact *names* but does not count target triples across all entries. Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda. |
| E06-S15 | PASS | Independent temporary-file probe: no authorization refused; a valid owner/version/verdict-digest authorization returned no problems; a changed final verdict with a later owner-note `PASS` refused; a stale digest and wrong version refused. `load_epic` was patched to raise if story status was consulted and was never called. `finalize` calls this check before writing the formula. Brief-Checksum: 13575c2d3f9fdea6c85190d0e7daae57f1299d780b4857a7236d69da5e915ab3. |
| E33-S01 | PASS | Independent localhost HTTPS feed probe using the system `/usr/bin/curl` and a synthetic Ed25519 publisher: a valid signed notice returned 0 and inserted one raw event row; HTTP 500 carrying the same valid notice returned 4 with rows identical; an over-256-KiB response returned 4 with rows identical. The stable-channel `test-curl,kill-points` integration suite passed 16 tests, including concurrent refresh, rollback, corrupt history and killed pre-commit transaction. Feed text reaches the same `install_text` and kernel replay path as local install; only the explicit local lift path writes a lift event. Brief-Checksum: d241ce1bedaf2c4e61db3ee33384640f4c5e3a46d03d134b55bf118345bcfced. |

## E06-S14 failure and required repair

**Reproduction.** The regression constructs a manifest that
`release_manifest.validate_document` accepts. Its four artifacts include
the three expected Homebrew engine names plus `second-macos-arm-archive`
with the same `target_triple` as the arm64 macOS entry. All archived bytes
hash to their declared digest and the published formula equals the renderer's
output. `manifest_digests` selects only entries whose names exactly match
the expected names; the fourth artifact is ignored. Thus `adoptable_formula`
returns the formula. `finalize` writes that return value after its
authorization check, so the duplicate-target manifest can change the live
formula. The executor's duplicate-target test duplicates an expected *name*,
which this path correctly refuses; it never duplicates a target under another
valid name.

**Required repair.** Before render or adoption, count all manifest artifact
entries by `target_triple` and refuse unless each required Homebrew target
occurs exactly once, independent of artifact name. Keep the expected-name
check, and test a distinct-name duplicate through simulated `finalize`,
asserting the live formula remains byte-identical. This violates E06-S14
AC3's explicit target-cardinality refusal and the evidence-gated release
boundary under SI-019/C-16.

The current live formula remains Python-only; no cutover or local provider
mutation occurred in this review. E06-S04 must remain blocked until this
finding is repaired and independently checked.

## Safety and falsification coverage

- **E06-S14:** inspected the publish job's checksum-verification/render/order,
  `render_release_formula`, `manifest_digests`, `engine_sha256s`,
  `provenance_command`, `adoptable_formula`, and `finalize`. Existing
  simulated altered archive, formula-byte, missing asset, wrong version,
  source-commit, and provenance cases passed (71 release/round-10 tests).
  The new valid-manifest duplicate-target counterexample fails as intended.
- **E06-S15:** inspected file identity/digest binding and the final-round
  verdict reader after E35's parser repairs. The same-user identity-claim
  limit is explicitly accepted by ADR-0040; no actual migration
  authorization exists yet. The test was on temporary files only.
- **E33-S01:** exercised real HTTPS transport, valid signing, server failure,
  and byte cap against a synthetic state root. Inspected URL override bounds,
  fixed system curl, signed replay, SQLite `BEGIN IMMEDIATE`, and the
  separation of feed install from local lift. The native host was macOS;
  Linux/Windows runtime behavior was not observed in this round. Windows
  cross-target Clippy passed, while Linux cross-target Clippy could not start
  without `x86_64-linux-gnu-gcc`.

## Gates actually run

| Command or group | Result |
| --- | --- |
| `project_os.py check/status/next/review`; three `brief <ID> --role verifier` commands; `check_agent_toolchain.py report` | PASS at entry. The review queue listed E06-S14 and E33-S01; S15 was already done. No toolchain decision was overdue. |
| `gh run list --branch main --limit 5` | UNKNOWN: `gh` required authentication, so no workflow was assumed green. |
| `python3 -m pytest tests/test_release.py tests/test_round10_adversarial.py -q` and `python3 -m pytest tests/test_release.py tests/test_project_os.py -q` | PASS: 71 and 102 tests, respectively. |
| `python3 -m pytest tests/test_round11_adversarial.py -q` | FAIL as intended: schema-valid duplicate-target manifest was adopted. |
| `python3 -m pytest tests -q` | 804 passed, 6 skipped, 2 failed: the intended duplicate-target regression and the pre-existing untracked `.opencode-run/prompt.md` broken links. |
| `python3 -m ruff check .`, `python3 -m ruff format --check .`, `python3 -m mypy ...` | UNAVAILABLE: `ruff` and `mypy` modules are absent from the system Python. `pre-commit run ruff-check/ruff-format --files tests/test_round11_adversarial.py` passed from their pinned hook environments. |
| Python project script gates in AGENTS.md | PASS except `check_docs.py check` (the same untracked `.opencode-run/prompt.md` links) and `gate_sensitivity.py check` (docs/pytest fail on the unmutated tree). Includes parity self-test/check, process, release, evidence, workflow, mutation boundary, and manifest gates. |
| `cargo fmt --check`; host workspace Clippy with all targets/features and `-D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace` | PASS. |
| `cargo deny check`; `CARGO_HOME=/private/tmp/e06-round11-cargo cargo deny --offline check` | First was blocked by a read-only advisory DB lock; offline run with a writable copy of that DB passed advisories, bans, licenses and sources. |
| Windows and Linux cross-target workspace Clippy | Windows PASS; Linux unable to build bundled SQLite because `x86_64-linux-gnu-gcc` is missing. |
| `CANCELLAI_CHANNEL=stable cargo test -p cancellai-cli --test containment --features test-curl,kill-points` | PASS: 16 tests. |
| Independent localhost HTTPS probe; independent temporary authorization probe; `git diff --check` | PASS, with individual outcomes above. |

## Documents actually opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`;
`docs/BACKLOG.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`;
`docs/development/AGENT_TOOLCHAIN.md`;
`docs/development/WORK_ITEM_MODEL.md`;
`docs/development/RELEASE_GATES.md`;
`docs/development/VERIFICATION_STRATEGY.md`;
`docs/development/MIGRATION_PYTHON_RUST.md`;
`docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`;
`docs/architecture/PERSISTENCE_MODEL.md`;
`docs/security/THREAT_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/security/SUPPLY_CHAIN.md`; `docs/security/INCIDENT_RESPONSE.md`;
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`;
`docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md`;
`docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`;
`docs/RELEASING.md`; `project/epics/E06.json`; `project/epics/E33.json`;
`project/templates/SAFETY_VERDICT.md`;
`project/evidence/E06-S04/SAFETY_VERDICT.md`;
`project/evidence/E06-S14/SAFETY_VERDICT.md`;
`project/evidence/E06-S15/SAFETY_VERDICT.md`;
`project/evidence/E33-S01/SAFETY_VERDICT.md`;
`project/evidence/E06-VERIFIER-REVIEW-ROUND9.md`;
`project/evidence/E06-VERIFIER-REVIEW-ROUND10.md`;
`project/evidence/E06-S14/EVIDENCE.md`;
`project/evidence/E06-S15/EVIDENCE.md`;
`project/evidence/E33-S01/EVIDENCE.md`.

## Overall verdict

FAIL for the round: one of three judged stories failed (33% finding yield).
E06-S14 returns to `in_progress`; E06-S04 remains blocked. E06-S15 passes
its re-review and stays `done`. E33-S01 passes and moves to `done`;
E33-S02's obsolete dependency blocker is cleared to `planned` without
reviewing its Guardian implementation; `project_os.py generate` refused
the stale blocker once E33-S01 closed. ADR-0025's yield rule calls for
another review after repair. The owner's recorded round-11 cost-ceiling
exception calls for a redesign consultation if this round fails; it does
not close E06-S14 on this failure. Neither epic closes or cuts a release
here. No new residual risk is accepted for E06-S14.
