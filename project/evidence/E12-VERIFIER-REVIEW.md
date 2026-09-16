# E12 Independent Verifier Review — Round 1

Review-Scope: epic
Round: 1
Review target: `82d30c1..f8dc4cc` (main)
Verifier: Codex
Date: 2026-09-16

## Per-story verdicts

| Story | Verdict | Concrete evidence | Handoff |
| --- | --- | --- | --- |
| E12-S01 | FAIL | Forced `<destination>.quarantine-record.json.tmp` collision: the implementation returned an error after rename with `source_exists=false; moved_exists=true; record_exists=false`. This violates AC3 (restore metadata recorded contentlessly) and SI-020’s explicit/recoverable mutation requirement. Required repair: durable journal/metadata plus rollback or idempotent recovery across rename and sidecar failures/crashes. | Brief-Checksum: eae5c25c2224707972f6731bae2035dcd9f16e291ce9bd7e5f41c76e3ecce734 |
| E12-S02 | FAIL | `fstatat` destination-absence check followed by `renameat` is not atomic. Independent reproduction created provider bytes after the check; rename replaced them with the quarantined artifact. This violates AC1 (never silently overwrite) and SI-013. Required repair: atomic no-replace move or fail closed where unavailable, with an injected interleaving test. | Brief-Checksum: 012e3eba7ba248700a0e05059af0773ce45b82f2a910312ece7ce419f62b0ea0 |
| E12-S03 | FAIL | A 10-byte archived file changed completely while retaining length returned `equal_length_corruption=Verified`; only length is checked. This violates AC3 (integrity checked before source purge) and SI-020. Required repair: versioned cryptographic digest verified before purge, plus durable recovery for the after-rename sidecar window. | Brief-Checksum: a24694312951905e7ce225d12fd62fbde2adc13ab194148acdc4451d700d726b |

## Reproductions and test-quality findings

1. E12-S01: ran a standalone program against `SystemMutationExecutor`, blocking the quarantine sidecar temporary name. It printed: `sidecar_failure=Some("quarantine move succeeded but its quarantine-record sidecar could not be written: File exists (os error 17)"); source_exists=false; moved_exists=true; record_exists=false`.
2. E12-S02: independently executed the state sequence represented by the implementation’s separate absence check and rename: destination absent → concurrent `new provider state` → rename. It printed `destination_after_check_then_rename=quarantined artifact`. `rename_child_matching_unix_identity` contains precisely that `fstatat`/`renameat` gap.
3. E12-S03: archived `0123456789`, replaced it with equal-length `abcdefghij`, and observed `equal_length_corruption=Verified` from public `verify_archive_integrity`.
4. The executor tests do pass, but the E12-S01 test explicitly accepts a moved artifact without its required record, E12-S02 only tests a destination present before the check, and E12-S03 calls a changed-length payload “corruption.”

## Gates actually run

| Command(s) | Result |
| --- | --- |
| `python3 -m pytest tests -v` | PASS, exit 0 — 665 passed, 542 subtests passed. |
| `python3 -m ruff check .` | NOT RUNNABLE, exit 1 — `No module named ruff`. |
| `python3 -m ruff format --check .` | NOT RUNNABLE, exit 1 — `No module named ruff`. |
| AGENTS.md’s full `python3 -m mypy cancellai.py ... scripts/gate_sensitivity.py` target list | NOT RUNNABLE, exit 1 — `No module named mypy`. |
| `python3 scripts/gen_docs.py --check`; all listed `scripts/*.py check` commands; `rust_python_parity.py self-test` | PASS, exit 0 for each on the review-target state. |
| Post-record regeneration: `python3 scripts/project_os.py generate`; `python3 scripts/project_os.py check`; `python3 scripts/verifier_handoff.py check`; `python3 scripts/check_evidence.py check`; `python3 scripts/check_process.py check`; `python3 scripts/check_docs.py check` | PASS, exit 0 for each. |
| `python3 scripts/process_metrics.py check` after recording this round | FAIL, exit 1 — generated metrics were stale; then `python3 scripts/process_metrics.py generate` and a repeat `check` both passed, exit 0. |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace`; `cargo deny check` | PASS, exit 0 for each (deny emitted pre-existing warnings only). |
| Cross-target clippy: `--target x86_64-pc-windows-gnu` and `--target x86_64-unknown-linux-gnu` | PASS, exit 0 for each. |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`; `project/epics/E12.json`; `docs/architecture/PERSISTENCE_MODEL.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; all three E12 verifier briefs; and `project/templates/SAFETY_VERDICT.md`.

## Overall round verdict

`FAIL` — 3 of 3 judged stories have reproduced CR4 defects (100% finding and rejection yield). E12-S01 is returned to `in_progress`; E12-S02 and E12-S03 have their own FAIL verdicts but are `blocked` because they depend on the now-open E12-S01, and E12-S04 is likewise blocked. A second review round is mandatory under ADR-0025 after repair; it must test the repaired failure paths rather than repeat these passing but insufficient tests.
