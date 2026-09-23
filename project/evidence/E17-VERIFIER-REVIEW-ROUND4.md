Review-Scope: epic
Round: 4
Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

# E17 Independent Verifier Review - Round 4

- Review target: `dfba2ff` on branch `review/e17-round4`
- Date: 2026-09-23
- Story contract: `project/epics/E17.json`; E17-S07 [CR4]
- The owner authorized exactly this additional round through `REVIEW_ROUND_EXCEPTIONS["E17"]`; no further round is authorized.

## Story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E17-S07 | FAIL | Evidence fields are now private and compile-fail doctests prove callers cannot construct or edit `KnowledgeProvenance` and `IncidentEvidence`. Capacity refusal is atomic at the configured boundary, exact duplicate scope/ceiling does not consume capacity, and a refusal does not consume that publisher sequence. However, `same_containment` compares `Option<Vec<_>>` by vector order while `applies_to` interprets versions, actions and platforms as membership sets. A same-ID notice with the same scope elements in a different order (or repeated elements) is treated as fresh and consumes ledger capacity despite imposing no new containment. This lets redundant signed notices exhaust the 4096 record budget. `cargo test -p cancellai-safety` passes 200 unit tests and 7 doctests; full workspace tests are blocked by sandbox local-socket denial in five desktop API tests. `cargo deny check` passed. |

## Findings and required repairs

1. **Semantically duplicate scopes consume capacity (SI-029, offline availability; round-3 capacity repair).** `same_containment` at `rust/crates/cancellai-safety/src/incident.rs` compares `provider_versions`, `action_classes`, and `platforms` as `Option<Vec<_>>`, which is order-sensitive and retains duplicate elements. In contrast, `applies_to` treats each list as a membership set. For example, an existing incident scoped to versions `["1.0.0", "2.0.0"]` is semantically identical to a signed re-issue scoped to `["2.0.0", "1.0.0"]`, but the latter is counted as fresh. Repeating semantically redundant re-issues with permutations or duplicate list values can consume the cumulative 4096-record budget without changing any binding; subsequent novel containment is then refused. **Required repair:** canonicalize each scope list as a set (including rejecting or removing duplicate members) before evidence comparison/storage, or compare normalized sets in `same_containment`. Add tests for reordered and duplicate-valued versions, action classes, and platforms showing such notices do not consume capacity, then a genuinely new scope still fits at the boundary. Preserve refusal atomicity and the existing containment on genuinely full capacity.

2. **Deduplication discards distinct incident evidence fields (AC3).** `same_containment` ignores severity, invariant references, affected releases, and knowledge provenance. If a later verified notice repeats the same scope and ceiling but adds an affected release or invariant reference, the call returns the new evidence as `Applied`, while `active` retains only the older record. The owner-visible evidence ledger therefore does not retain all supplied impact/invariant details. **Required repair:** define and implement a bounded, monotonic evidence merge for a duplicate containment so newly verified affected releases and invariant references remain available, or make the deduplication identity include evidence fields and bound the resulting audit records without permitting capacity eviction or authority weakening. Add a regression where a same-containment re-issue carries new `affected_releases` and invariant references, and assert the retained evidence exposes them.

The remaining round-3 repairs hold: provenance/evidence fields are private with compile-fail construction/edit doctests; release provenance remains internal to the safety boundary; records append without replacing broader scopes or stricter ceilings; exact duplicate containment does not consume capacity; at-capacity refusal preserves all active records and does not consume the refused sequence; the publisher count is bounded and a previously tracked publisher may continue updating. Sequence tracking is keyed by publisher ID, so a same-ID key rotation under the current local trust policy retains the last sequence; a new publisher ID is refused at the bound rather than evicting an old sequence. `effective_authority_under_containment` only adds the matched Observe/Recommend ceiling to the pre-existing base and channel constraints, so its result remains their monotonic minimum. No authority elevation counterexample was found.

## Gates run

| Gate | Result |
| --- | --- |
| `python3 scripts/project_os.py check/status/next/review` | PASS / inspected; E17-S07 is queued for review |
| `python3 scripts/check_agent_toolchain.py report` | PASS; no decision past review date |
| `gh run list --branch main --limit 5` | UNKNOWN; GitHub API connection unavailable |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | FAIL in 5 `cancellai-cli/tests/desktop_api.rs` cases; sandbox reports `Operation not permitted` when the local desktop API socket starts. Other displayed tests passed before Cargo stopped at that integration target. |
| `cargo test -p cancellai-safety` | PASS; 200 unit tests, 7 doctests |
| `cargo deny check` | PASS; advisory, bans, licenses and sources OK; existing duplicate hashbrown/syn and unmatched BSD-2-Clause/ISC allowance warnings |
| Python tests | PASS; `python3 -m pytest tests -v`: 689 passed, 637 subtests passed |
| Ruff | PASS; workspace venv Ruff 0.16.5 `check` and `format --check` (the active Python 3.13 interpreter does not have the module installed) |
| Mypy | PASS; workspace venv mypy 2.3.1 on the AGENTS.md file set |
| Python repository gates | PASS; `gen_docs.py --check`, `project_os.py check`, `check_docs.py check`, `check_workflows.py check`, `check_fixtures.py check`, `check_schemas.py check`, `characterize.py check`, `diff_harness.py check`, `check_rust_workspace.py check`, `check_mutation_boundary.py check`, `check_provider_compatibility.py check`, `check_provider_trust.py check`, `check_platforms.py check`, `rust_python_parity.py self-test` and `check`, `check_process.py check`, `release.py check`, `release_manifest.py check`, `check_repository_topology.py check`, `check_agent_skills.py check`, `process_metrics.py check`, `check_risk_classification.py check`, `check_agent_toolchain.py check`, `check_skill_content.py check`, `verifier_handoff.py check`, `check_evidence.py check`, `safety_oracle.py check`, `check_ears.py check`, `gate_sensitivity.py check` (11/11 mutants killed). Existing baseline process/evidence/contract warnings were reported; no new gate failure resulted. |

## Documents opened

- `AGENTS.md` (provided in-session)
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `project/templates/VERIFIER_PROMPT.md`
- `project/templates/SAFETY_VERDICT.md`
- `project/evidence/E17-S07/VERIFIER_BRIEF.md`
- `project/epics/E17.json`
- `project/evidence/E17-VERIFIER-REVIEW-ROUND2.md`
- `project/evidence/E17-VERIFIER-REVIEW-ROUND3.md`
- `project/evidence/E17-S07/SAFETY_VERDICT.md`
- `docs/security/SAFETY_INVARIANTS.md` (SI-022, SI-029, SI-030)
- `docs/security/THREAT_MODEL.md` (TM-11)
- `docs/security/INCIDENT_RESPONSE.md`
- `docs/security/SUPPLY_CHAIN.md`
- `rust/crates/cancellai-safety/src/incident.rs`
- `rust/crates/cancellai-safety/src/authority.rs`
- `rust/crates/cancellai-safety/src/knowledge_bundle.rs`
- `rust/crates/cancellai-safety/src/lib.rs`
- `scripts/check_process.py`

## Overall verdict

**FAIL.** The evidence sealing repair holds, but semantic duplicate scopes can still spend bounded ledger capacity, and deduplicated re-issues can lose added impact/invariant evidence. E17-S07 returns to `in_progress`; E17 status remains unchanged. This is the owner-authorized final round, so there is no round 5. The findings must be carried as new backlog work under the applicable process; the verdict does not authorize another E17-S07 review.
