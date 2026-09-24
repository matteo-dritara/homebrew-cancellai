# E06 round 10 findings - refused by the harness, kept as a defect report

Codex ran round 10 through `scripts/review_round.py` on 2026-09-24 (base `c5fe88d`). The harness
refused the run - "rust/crates/cancellai-store/src/containment_state.rs is outside what this tier
may change": the reviewer put its adversarial test inside a production source file - so nothing
was imported and the run is not a counted round. Its findings are real and reproducible; the record
is reproduced below, inside a fence, as the defect report the round-10 repairs answer. It is quoted,
not a verdict: the harness never accepted it.

````text
Review-Scope: epic
Round: 10
Verifier: Codex
Date: 2026-09-24
Review target: `8ea967e..c5fe88d` (ADR-0040 and the E33-S03, E06-S14, E06-S15 repairs)

# E06 formal verifier review, round 10

The checked-in queue contained three `ready_for_review` repairs. E33-S01 and E06-S04 were already `blocked`, contrary to the prompt's assertion that all five were ready. Their rows below are prerequisite failures, not claims that their remaining acceptance criteria were independently exercised. CI on `main` was unknown: `gh run list --branch main --limit 5` could not authenticate.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S14 | FAIL | `verify_provenance` invokes `gh attestation verify <archive> --repo matteo-dritara/homebrew-cancellai` without a signer workflow or source tag/ref. `gh attestation verify --help` says `--repo` is the minimum identity and offers `--signer-workflow` and `--source-ref`. An archive attested by another workflow/ref in this repository can satisfy this check; the digest and formula can agree with a manifest naming those bytes. Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda. |
| E06-S15 | FAIL | A Safety Verdict containing `## Round 10`, `FAIL`, then `## Owner note`, `PASS` made the real `project_os.safety_verdict_passes` return `True`; with a matching digest `cutover_authorization_problems` accepts it. The new adversarial test `test_a_later_note_cannot_reverse_a_failed_final_round` fails with `[] is not true`. Separately, `Authorized-by: stranger` with the right version and digest returns `[]`. The earlier failing-final-round test mocks the verdict reader. Brief-Checksum: 13575c2d3f9fdea6c85190d0e7daae57f1299d780b4857a7236d69da5e915ab3. |
| E33-S03 | FAIL | New adversarial test `an_existing_database_without_events_must_not_be_initialized_on_install` fails: `load_events` reports an existing SQLite database without `events` as unreadable, but `transact` runs `CREATE TABLE IF NOT EXISTS events` before `BEGIN IMMEDIATE` and returns `Ok(())` for an append. The test output was `unreadable existing history was accepted: Ok(())`. Brief-Checksum: 06f4b8e341065cb1b07f471d44b7fe59f0964ded2c3c6aa937c285f4c6365e92. |
| E33-S01 | FAIL | The story is `blocked` on E33-S03, whose AC4 fails above. A feed refresh can therefore reach a persistence path that turns an existing unreadable ledger into an empty one. Its AC2 and SI-029 cannot be discharged in this round. Brief-Checksum: d241ce1bedaf2c4e61db3ee33384640f4c5e3a46d03d134b55bf118345bcfced. |
| E06-S04 | FAIL | The story is `blocked` on E06-S14 and E06-S15, both rejected above; no `CUTOVER_AUTHORIZATION.md` or accepted migration Safety Verdict is present. The owner acceptance and full release packet ACs are unmet. Brief-Checksum: b78b236d5e2433f08acd89d4f261ef6d0fe7a37436bdf8644d64ab4f670ca107. |

## Required repairs

- **E06-S14, AC3 / SI-019:** bind archive attestation verification to the canonical release workflow and the exact tag/ref (or an equivalently authenticated release identity), and compare it with the manifest's `build_identity`, in addition to the repository and downloaded-byte digest. Test a same-repository attestation from a different workflow or ref and require refusal with the live formula unchanged.
- **E06-S15, AC1–AC2 / SI-019:** make owner authorization an authenticated owner decision rather than accepting any `Authorized-by` value, and determine the final verdict from the actual final round section. A stray later verdict-shaped line must not turn a failing round into a pass. Test both counterexamples through `finalize` with the formula unchanged.
- **E33-S03, AC4 / SI-029:** distinguish a missing ledger from an existing database before schema creation; refuse an existing database with a missing or unreadable `events` table without modifying it. Create the table only for a genuinely missing ledger, within the intended transaction. Retain the adversarial test and assert the existing database remains unreadable/unchanged.
- **E33-S01:** keep blocked until E33-S03 passes, then re-run feed refusal, concurrency and local-authority cases against the repaired store.
- **E06-S04:** keep blocked until E06-S14/S15 pass and the owner accepts the migration Safety Verdict; then perform the specified native platform reproductions and release checks.

## Gates run

- `python3 scripts/project_os.py check`, `status`, `next`, `review`, and five `brief ... --role verifier`: pass before status changes; queue showed three ready and two blocked.
- `python3 scripts/check_agent_toolchain.py report`: pass; no decision past review date.
- `gh run list --branch main --limit 5`: unknown, authentication unavailable.
- `python3 -m pytest tests -q`: after the new adversarial test, 793 passed, 6 skipped, 2 failed: the expected E06-S15 counterexample and a documentation-link failure caused by the pre-existing untracked `.opencode-run/prompt.md` (outside the review diff).
- `pre-commit run --all-files`: all reported hooks passed except `docs-check`, which found the same untracked `.opencode-run/prompt.md` links; no production gate failure was hidden by treating this as green.
- `python3 -m mypy ...` in the system interpreter: unavailable (`No module named mypy`); the pre-commit mypy hook passed.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace`: passed in the chained run before the new adversarial test was compiled.
- `cargo deny check`: could not obtain its advisory database lock under read-only `/Users/matteo.peo/.cargo/advisory-dbs/db.lock`; unknown.
- `cargo test -p cancellai-store an_existing_database_without_events_must_not_be_initialized_on_install -- --nocapture`: failed as the independent counterexample above.
- `CANCELLAI_CHANNEL=stable cargo test -p cancellai-cli --features kill-points,test-curl --test containment`: 16 passed, including concurrent install/refresh and kill-point cases; the missing-table case is absent from that suite.
- `python3 -m pytest tests/test_release.py -q`: 64 passed before the new adversarial test; the targeted new test then failed as expected. Its earlier final-verdict failure test mocks the real verdict reader.
- `pre-commit run ruff-format --files tests/test_release.py` and `pre-commit run ruff-check --files tests/test_release.py`: passed after formatting the new test.
- `cargo fmt --check`: passed after formatting the added adversarial test.
- `python3 scripts/check_workflows.py check`, `scripts/check_mutation_boundary.py check`, `scripts/check_evidence.py check`, `scripts/check_process.py check`, `scripts/release.py check`: passed before the round record/status update.
- After the round record/status update: `scripts/project_os.py check`, `scripts/process_metrics.py check`, `scripts/verifier_handoff.py check`, `scripts/check_process.py check`, and `git diff --check` passed.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/VERIFICATION_STRATEGY.md`; `docs/development/REPOSITORY_GOVERNANCE.md`; `docs/architecture/TARGET.md`; `docs/architecture/PLATFORM_MODEL.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/security/SUPPLY_CHAIN.md`; `docs/security/INCIDENT_RESPONSE.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `docs/adrs/0039-the-cutover-perimeter-binds-cli-authority-to-a-local-containment-ledger.md`; `docs/adrs/0040-release-integrity-by-construction-and-a-transactional-containment-ledger.md`; `project/epics/E06.json`; `project/epics/E33.json`; `project/templates/SAFETY_VERDICT.md`; `.github/CONTRIBUTING.md`; `.github/CODEOWNERS`; `project/evidence/E06-S14/EVIDENCE.md`; `project/evidence/E06-S15/EVIDENCE.md`; `project/evidence/E33-S03/EVIDENCE.md`; prior `project/evidence/E33-S01/SAFETY_VERDICT.md` and `project/evidence/E06-S04/SAFETY_VERDICT.md`.

## Overall verdict

FAIL. Three of three ready repairs have counterexamples; two dependent stories remain blocked. This round's measured defect yield is above 10%, so the owner must decide the next review step under ADR-0025's explicit round-ceiling exception. No cutover or release is authorized by this record.
````
