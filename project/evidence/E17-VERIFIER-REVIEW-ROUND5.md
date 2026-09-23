Review-Scope: epic
Round: 5
Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

# E17 Independent Verifier Review - Round 5

- Review target: `f4eefe5` on the current working branch
- Date: 2026-09-23
- Story contract: `project/epics/E17.json`; E17-S07 [CR4]
- Owner authorized exactly this additional round through `REVIEW_ROUND_EXCEPTIONS["E17"]`.

## Story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E17-S07 | PASS_WITH_RESIDUALS | Round-4 repairs independently confirmed. Canonical sets are built before ledger comparison/storage; tests exercise permutations and repeated values across versions/actions/platforms at capacity and show a genuinely new scope still fits. Dedup identity includes severity, invariant references, and affected releases; tests prove each independent evidence-field change is retained. At-capacity refusal preserves held containment and leaves sequence retryable. No authority-elevation path found. Existing residual: raw bundle text has no maximum byte-size check before parsing/verifying, so list bounds do not bound a single oversized input. |

## Findings and required repairs

No blocking defect found in the round-4 repairs or this adversarial pass. No required repair for this round.

## Adversarial pass

- **Path / identity changes:** incident, provider, version and release identifiers are bounded strings; matching uses exact provider/version identity. Unknown installed version is conservatively treated as affected by a version-scoped containment.
- **Partial reads / permissions:** the incident API consumes a complete supplied bundle string; unavailable service is represented separately and leaves the ledger unchanged. Raw input size remains unbounded before parsing (residual below).
- **Links / mounts / reparse points:** no filesystem path is interpreted by this notice schema; path-like characters are rejected by identifier validation.
- **Provider version / layout drift:** typed optional version scope and conservative unknown-version matching fail toward containment. Provider layout is outside the incident payload's authority.
- **Concurrency:** ledger mutation requires exclusive `&mut ContainmentLedger`; sequence and append updates occur only after verification, notice validation, and capacity checks.
- **Crash / failure / retry:** invalid signature, malformed notice, expiry, replay and capacity refusal leave active records and sequence state unchanged; local lift is the sole removal path. A refused sequence is retryable.
- **Boundary values / capacity:** entry/list/string bounds are validated; cumulative active records and publishers have fixed caps; redundant canonical records consume no capacity; full capacity refuses atomically without eviction.
- **Policy / trust conflicts:** bundle verification checks trusted publisher policy before parsing accepted payload; containment ceilings only add Observe/Recommend constraints to the minimum authority calculation. Promotion freeze is independent of that ceiling.
- **Platform differences:** supported platforms are typed enum values; scopes are exact and platform-specific. No path or OS call occurs in the ledger implementation.
- **Malformed / untrusted input:** unknown fields, bad schema/kind, duplicate incident IDs, malformed IDs, invalid signatures, unknown publishers and replay are refused. No payload content field exists in evidence.
- **Performance / large datasets:** retained records/publishers and entry/list counts are bounded. A single raw bundle string has no pre-parse byte cap; this remains an availability risk if a caller accepts arbitrarily large network responses.

## Gates run

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS; all workspace tests and doctests passed. The chained initial invocation reached `cargo deny check`; no test failure was reported. |
| `cargo deny check` | PASS on retry with writable temporary `CARGO_HOME`; advisory, bans, licenses and sources OK. Existing duplicate crate and unmatched allowance warnings. |
| `python3 -m pytest tests` | PASS; 689 passed. |
| `python3 scripts/project_os.py check` | PASS before evidence/status update; rerun after generation. |
| `python3 scripts/verifier_handoff.py check` | PASS before evidence/status update; rerun after generation. |
| `python3 scripts/check_process.py check` | PASS before evidence/status update; recorded warnings for historical over-ceiling epics, including E17 exception history. Rerun after adding this record. |

## Known residual risks

- `ContainmentLedger::refresh` accepts a `&str` and parsing/signature verification has no explicit raw-byte maximum before allocation. The caller's fetch layer must impose a response-size limit; without one, a very large signed or invalid response may consume excessive memory/CPU. No fix was part of the two round-4 repairs.
- The mechanism is not wired to a live mutation path or shipped distribution channel; this remains E06-S04 cutover work as documented in the story and incident runbook.

## Documents opened

- `AGENTS.md` (provided in-session)
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `project/evidence/E17-S07/VERIFIER_BRIEF.md`
- `project/epics/E17.json`
- `project/evidence/E17-VERIFIER-REVIEW-ROUND2.md`
- `project/evidence/E17-VERIFIER-REVIEW-ROUND3.md`
- `project/evidence/E17-VERIFIER-REVIEW-ROUND4.md`
- `project/evidence/E17-S07/SAFETY_VERDICT.md`
- `docs/security/SAFETY_INVARIANTS.md` (SI-022, SI-029, SI-030)
- `docs/security/THREAT_MODEL.md` (TM-11)
- `docs/security/INCIDENT_RESPONSE.md`
- `docs/security/SUPPLY_CHAIN.md`
- `rust/crates/cancellai-safety/src/incident.rs`
- `rust/crates/cancellai-safety/src/authority.rs`
- `rust/crates/cancellai-safety/src/knowledge_bundle.rs`
- `rust/crates/cancellai-safety/src/build_channel.rs`
- `scripts/check_process.py`

## Overall verdict

**PASS_WITH_RESIDUALS.** The two round-4 repairs hold under independent review and no new blocking counterexample was found. The raw-input byte-size limit and lack of live cutover integration remain visible residual risks. E17-S07 may advance to `verification`; the owner retains the CR4 decision.
