Review-Scope: epic
Round: 2
Verifier: Codex
Brief-Checksum: 255d8f22ed2d1d30165dd673fdbc2d6d045646d1ee799f5a416669efb31c62ea

# E17 Verifier Review - Round 2

- Review target: implementation commit `ae927b3` (on base `bfffef5`), plus the verifier regression test in this review commit
- Date: 2026-09-23
- Scope: E17-S07 only; E17-S01..S06 were judged in round 1 and are already `done`.
- CI on `main`: unknown; `gh run list --branch main --limit 5` could not connect to `api.github.com`.

## Story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E17-S07 | FAIL | `incident::tests::a_reissued_incident_cannot_shrink_scope_or_be_replaced_by_another_publisher` reproduces a trusted publisher replacing a broad containment under the same incident ID with a narrower scope. Version `2.0.0` becomes uncontained. Separately, `ContainmentLedger::ingest` accepts caller-supplied `ReleaseProvenance`, whose public fields can claim a version/channel unrelated to the running build. |

## Findings and required repairs

1. **Remote re-issue shrinks an active containment (SI-029; AC1 and replay/rollback verification contract).** The regression first ingests `INC-1` for `codex` versions `2.0.0` and `3.0.0`, ceiling `Observe`, from trusted publisher `acme`; it then ingests the same incident ID with only `3.0.0`, same ceiling, signed by a second locally trusted publisher `backup`. `binding_for(codex, 2.0.0, Delete)` changes from `Some` to `None`. `incident.rs` stores globally by `incident_id` and only retains the existing record when its ceiling is strictly lower; equal ceilings replace the record and its scope. The same loss is possible with a later sequence from the same publisher. **Required repair:** make remote re-issues monotonic across all fields and publishers: an active incident's affected target set may not shrink and its effective ceiling may not rise. Reject conflicting re-issues or merge them so the old scope and strictest ceiling remain in force; keying sequence by publisher alone does not establish incident-scope monotonicity. Add same-publisher and multiple-publisher regression cases.
2. **Evidence can falsely attribute the running release (SI-022 provenance; AC3).** `ReleaseProvenance` has public writable `version` and `channel`, and public `ContainmentLedger::ingest` accepts it as an argument. A caller can pass `ReleaseProvenance { version: "9.99.9", channel: Stable }` to a valid signed bundle and receive `IncidentEvidence` recording those claims even when that is not the running build. This does not forge ledger authority, but it undermines the release-provenance evidence the story requires. **Required repair:** derive release provenance inside the trusted safety boundary from compiled package/build metadata and an opaque build-channel value; do not let the caller or remote notice provide the value recorded as the running release. Add a test showing forged caller input cannot alter evidence, or make such input unconstructible.

The high-level ceiling checks, signature verification, refusal atomicity, local-only lift, and offline preservation are supported by the implementation and existing tests. They do not close the two failures above. `effective_authority_under_containment` is also not wired into a live mutation path, as the docs disclose; that is deferred to E06-S04 and is not independently treated as a defect in this story's stated infrastructure outcome.

## Dependency decision

PD-026 is sound against the story-level graph. E17-S07 depends on E16-S05, E17-S03 and E17-S05; none requires E06. E06-S04 in turn waits on E17-S07, so retaining E17 → E06 would create the wrong closure order. Replacing that coarse edge with E02, E16, E20 and E26 matches the epics consumed by E17 stories and preserves E06-S04's own E17-S07 prerequisite. PD-026 does not justify closing E17 while S07 fails; the epic status remains unchanged.

## Gates run

| Gate | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS at review start |
| `python3 scripts/project_os.py status` | PASS; status read |
| `python3 scripts/project_os.py next` | PASS; no explicitly ready stories |
| `python3 scripts/project_os.py review` | PASS; E17-S07 queued |
| `python3 scripts/check_agent_toolchain.py report` | PASS; no overdue decisions |
| `gh run list --branch main --limit 5` | UNKNOWN; network/API connection failed |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | FAIL as expected: the new regression test fails on the reproduced scope shrink; other tests passed |
| `cargo deny check` | PASS; warnings for existing duplicate `hashbrown`/`syn` versions and unmatched BSD-2-Clause/ISC allowances |
| `pre-commit run --all-files` | PASS after the round record and regression fixture were added; all configured Python/repository hooks passed |
| `cargo test -p cancellai-safety a_reissued_incident_cannot_shrink_scope_or_be_replaced_by_another_publisher -- --nocapture` | FAIL as expected; confirms the regression |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `project/templates/VERIFIER_PROMPT.md`
- `project/templates/SAFETY_VERDICT.md`
- `project/evidence/E17-S07/VERIFIER_BRIEF.md`
- `project/epics/E17.json`
- `project/epics/E06.json` (E06-S04)
- `project/epics/E16.json` (E16-S05)
- `project/decisions.json` (PD-026)
- `docs/security/SAFETY_INVARIANTS.md` (SI-022, SI-029, SI-030)
- `docs/security/THREAT_MODEL.md` (TM-11)
- `docs/security/INCIDENT_RESPONSE.md`
- `docs/security/SUPPLY_CHAIN.md`
- `docs/development/RELEASE_GATES.md`
- `rust/crates/cancellai-safety/src/incident.rs`
- `rust/crates/cancellai-safety/src/authority.rs`
- `rust/crates/cancellai-safety/src/knowledge_bundle.rs`
- `project/evidence/E17-VERIFIER-REVIEW.md` (round 1)

## Overall verdict

**FAIL.** E17-S07 remains `in_progress`; the remote ledger can lose an active scope without a local lift, and incident release evidence can be caller-forged. The authored regression is intentionally failing and preserves the concrete counterexample. The owner must return the exact repairs above to implementation and request another review round after `ready_for_review`.
