# E17 Independent Verifier Review - Round 3

Review-Scope: epic
Round: 3
Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

- Review target: `8654dea^..8654dea` on branch `review/e17-round3`
- Date: 2026-09-23
- Story contract: `project/epics/E17.json`; E17-S07 [CR4], status returned to `in_progress`

## Story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E17-S07 | FAIL | The two round-2 fixes are present: active re-issues append and authority uses the strictest matching ceiling; release provenance is constructed internally from compile-time version/channel metadata. Two further counterexamples remain: public `KnowledgeProvenance` and `IncidentEvidence` fields allow callers to forge/alter purported incident records; and every verified notice may add up to 64 entries to `ContainmentLedger.active` without a cumulative bound, so repeated signed updates can exhaust memory and defeat offline availability. Detailed invariant evidence and required repairs are appended to `project/evidence/E17-S07/SAFETY_VERDICT.md`. |

## Findings and required repairs

1. **Evidence authenticity (SI-022, AC3):** `KnowledgeProvenance` and `IncidentEvidence` are exported structs whose fields are public. External code can construct knowledge provenance directly and construct or mutate an incident evidence value before serializing it. Make these values opaque to callers with private fields and immutable read accessors; keep authenticated evidence construction inside verified ingestion, and add compile-fail/API regression coverage so caller-created or edited values cannot be presented as ledger-verified evidence.
2. **Cumulative resource exhaustion (SI-029, offline availability):** each notice is bounded to 64 entries, but `ingest` appends each newer notice's evidence indefinitely and tracks sequences indefinitely. A trusted/compromised publisher can issue valid successive sequences containing fresh incident IDs and grow retained state without limit. Bound retained records and publisher sequence state; on capacity, refuse the incoming notice atomically and retain all existing containment. Never evict or weaken active records remotely. Add repeated-ingestion and at-capacity refusal tests.
3. Before resubmission, rerun all gates and complete `cargo deny check`; its advisory database could not be used successfully in this environment (details below).

The AC for monotonic re-issues and internal release provenance are independently confirmed. This round does not pass E17-S07.

## Gates run

- `python3 scripts/check_agent_toolchain.py report`: PASS; no review decision past its date.
- `gh run list --branch main --limit 5`: UNKNOWN; connection to `api.github.com` unavailable.
- `python3 scripts/project_os.py check`, `status`, `next`, `review`: PASS / inspected; E17-S07 was the only item awaiting review at session start.
- Rust: `cargo fmt --check` PASS; `cargo clippy --workspace --all-targets --all-features -- -D warnings` PASS; `cargo check --workspace --all-targets` PASS; `cargo test --workspace` PASS (the scheduled heavy benchmark is annotated ignored); `cargo deny check` NOT COMPLETED. First attempt failed to lock the configured read-only advisory database. A temporary writable `CARGO_HOME` retry stalled refreshing the advisory database and was stopped; no pass is claimed.
- Python: `python3 -m pytest tests -v` PASS (688 tests, 633 subtests). `python3 -m ruff` initially could not import the module; the pinned local executables were present and used instead: Ruff 0.16.5 `check` PASS and `format --check` PASS; mypy 2.3.1 PASS. All remaining Python checks in AGENTS.md passed: `gen_docs.py --check`, `project_os.py check`, `check_docs.py check`, `check_workflows.py check`, `check_fixtures.py check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check`, `check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py check`, `check_provider_trust.py check`, `check_platforms.py check`, `rust_python_parity.py self-test` and `check`, `release.py check`, `release_manifest.py check`, `check_repository_topology.py check`, `check_agent_skills.py check`, `process_metrics.py check`, `check_risk_classification.py check`, `check_agent_toolchain.py check`, `check_skill_content.py check`, `verifier_handoff.py check`, `check_evidence.py check`, `safety_oracle.py check`, `check_ears.py check`, and `gate_sensitivity.py check` (11/11 mutants killed).
- Final recording gates: `project_os.py generate` PASS; `process_metrics.py generate` PASS; `verifier_handoff.py check` PASS; `project_os.py check` PASS; `check_process.py check` FAIL: adding this round-3 record makes E17 appear to have 4 independent review records (`E17-S08-VERIFIER-REVIEW.md`, round 1, round 2, and round 3), exceeding the epic ceiling of 3; `process_metrics.py check` PASS; `check_evidence.py check` PASS with documented legacy packet warnings; `git diff --check` PASS.

## Documents opened

`AGENTS.md` (provided in-session); `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `project/epics/E17.json`; `project/templates/VERIFIER_PROMPT.md`; `project/templates/SAFETY_VERDICT.md`; `project/evidence/E17-S07/VERIFIER_BRIEF.md`; `project/evidence/E17-S07/SAFETY_VERDICT.md`; `project/evidence/E17-VERIFIER-REVIEW-ROUND2.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/INCIDENT_RESPONSE.md`; `docs/security/SUPPLY_CHAIN.md`; `docs/development/RELEASE_GATES.md`; `rust/crates/cancellai-safety/src/incident.rs`; `rust/crates/cancellai-safety/src/authority.rs`; `scripts/project_os.py`.

## Overall verdict

**FAIL.** Both required round-2 repairs are confirmed, but E17-S07 remains unsafe to close until the evidence types are opaque and cumulative containment state is bounded without remote eviction. The story remains `in_progress`; the epic status is unchanged. No other branch was touched and no push was made.
