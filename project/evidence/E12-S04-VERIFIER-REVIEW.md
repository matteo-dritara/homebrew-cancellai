# E12-S04 Verifier Review

Review-Scope: story
Round: 1
Review target: `f215d33..8afcebc` (also inspected current `HEAD` `f189193` for downstream overlap)
Verifier: Codex
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Date: 2026-09-20

## Verdict

`FAIL`

E12-S04 does not meet AC1, "Tombstones contain no prompts/source/file contents." Its public
`Tombstone` type exposes arbitrary `String` values for `provider_id`, `category`,
`reason_code`, `policy_id`, and `plan_id`, and arbitrary `EvidenceId` strings. The implementation
passes those values unchanged to `EventMetadata`/`MutationReference`, then to SQLite. This is
not an abstract typing concern: `ledger.rs` explicitly records the inherited residual that its
`reason_code` is a `String` into which a caller can put content.

## Independent reproduction

I built and ran a standalone temporary Rust consumer against the public `cancellai-store` API:

```text
$ cargo run --offline --manifest-path /private/tmp/e12s04-tombstone-repro.fCXqqu/Cargo.toml
FAIL: arbitrary prompt/source/path sentinel was persisted in a Purged tombstone
```

The consumer supplied `ActionClass::Delete` and `Reversibility::Irreversible`, with
`PROMPT_SENTINEL_do_not_store_source_contents` in `provider_id`, `reason_code`, `policy_id`,
`plan_id`, and an `EvidenceId`, plus `/private/provider/source.rs` in `category`. The call
succeeded; `EventLedger::read_all()` returned the same sentinel and path in its stored `PURGED`
event. Thus the primary acceptance test only proves that benign sample strings round-trip; it is
tautological with respect to content exclusion and omits malformed/adversarial metadata.

## Required repair

Replace every tombstone field that can carry caller-selected bytes with content-safe, validated
identifier/value types (or a strict, documented allowlist validator at this public boundary),
including `provider_id`, `category`, `reason_code`, `policy_id`, `plan_id`, and each evidence
reference. Reject values containing prompt/source/file content or paths before `EventLedger::append`
and make the operation atomic/no-write on every refusal. Add adversarial tests that submit
distinct prompt, source-text, and absolute/relative path sentinels through every retained field
and assert no `PURGED` row is written. The generic ledger's known `String` residual cannot be
inherited by an API that claims its output is a contentless tombstone.

This repair is required for AC1 and constitutional C-09. The leak also breaks the contentless
audit record that SI-020 requires to represent an irreversible purge explicitly: a permanently
retained `PURGED` record must not disguise retained payload as cleanup metadata.

## Other safety obligations checked

- AC2 / SI-020 action distinction: PASS in the reviewed primitive. It admits only
  `ActionClass::Delete + Reversibility::Irreversible`; its exhaustive 30-pair test and direct
  source inspection show every `VendorConditional` pairing is refused before writing.
- `mutation_executor::execute`: PASS on source inspection and its existing adversarial tests.
  It requires `minimum_authority_for(Delete) == AuthorityLevel::Govern` and rejects a Delete
  plan whose reversibility is not `Irreversible` before mutation. The tombstone primitive has no
  mutation/orchestrator caller, so it cannot repair the separate missing crash/retry protocol
  between a future purge and ledger write; that is a future integration obligation, not this
  AC1 failure.
- Concurrency/crash: `&mut EventLedger` prevents aliased in-process calls, and SQLite append is
  transactional. No real purge currently invokes this primitive; therefore the story cannot
  demonstrate a purge/restore/archive race or crash recovery. This does not excuse the directly
  reproduced retained-content violation.

## Gates executed

| Command(s) | Result |
| --- | --- |
| `cargo fmt --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo test --workspace` | PASS. Workspace tests include all 118 `cancellai-store` tests, but the four new tombstone tests do not adversarially exercise field contents. |
| `cargo deny check` | PASS after approved Cargo advisory-DB access; pre-existing unmatched-license and duplicate warnings only. |
| `cargo run --offline --manifest-path /private/tmp/e12s04-tombstone-repro.fCXqqu/Cargo.toml` | FAIL AS EXPECTED: independently reproduced persistence of prompt/source/path sentinels in `PURGED`. |
| `python3 -m pytest tests -v`; `python3 -m ruff check .`; `python3 -m ruff format --check .`; AGENTS.md's full listed `python3 -m mypy ...` target list | PASS. |
| `python3 scripts/gen_docs.py --check`; `project_os.py check`; `check_docs.py check`; `check_workflows.py check`; `check_fixtures.py check`; `check_schemas.py check`; `characterize.py check`; `diff_harness.py check`; `check_rust_workspace.py check`; `check_mutation_boundary.py check`; `check_provider_compatibility.py check`; `check_provider_trust.py check`; `check_platforms.py check`; `rust_python_parity.py self-test`; `rust_python_parity.py check`; `check_process.py check`; `release_manifest.py check`; `check_repository_topology.py check`; `check_agent_skills.py check`; `process_metrics.py check`; `check_risk_classification.py check`; `check_agent_toolchain.py check`; `check_skill_content.py check`; `verifier_handoff.py check`; `check_evidence.py check`; `safety_oracle.py check`; `check_ears.py check`; `gate_sensitivity.py check` | PASS before this record was added. `scripts/release.py` was deliberately not run under the review instruction. |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `project/evidence/E12-S04/VERIFIER_BRIEF.md`;
`docs/architecture/PERSISTENCE_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`;
`docs/security/THREAT_MODEL.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`;
`docs/development/RELEASE_GATES.md`; `project/templates/SAFETY_VERDICT.md`;
`rust/crates/cancellai-store/src/tombstone.rs`; `rust/crates/cancellai-store/src/ledger.rs`;
`rust/crates/cancellai-safety/src/authority.rs`; `rust/crates/cancellai-safety/src/mutation_executor.rs`;
and `rust/crates/cancellai-model/src/vocabulary.rs`.

## Method defects

None. The executor's "no content-typed field" argument conflated a schema-column allowlist with
validation of untyped `String` payloads; this is recorded as the implementation finding above.
