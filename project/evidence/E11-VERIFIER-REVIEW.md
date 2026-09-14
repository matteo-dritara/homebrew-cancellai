# E11 Independent Verifier Review - Round 1

- Epic: E11 - Deterministic Policy Engine
- Review target: `d2a4c2d^..b035f83`
- Verifier: Codex (`/root`), independent of the Claude executor
- Date: 2026-09-14

All four stories were `ready_for_review` before review began. I reconstructed the contracts from
the control plane, architecture, invariants, and threat model, and did not read
`project/evidence/E11-SELF-REVIEW.md`.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E11-S01 | FAIL | `serde_json` deserialized duplicate scoped-map keys by silently retaining the later value. Thus a document containing `providers.codex: observe` followed by `providers.codex: autopilot` parsed as `Autopilot`, violating C-03. Repaired in this review with a duplicate-rejecting map visitor and regression; awaiting round-2 verification. |
| E11-S02 | PASS_WITH_RESIDUALS | `resolver.rs:146-154` changes only `AuthorityInputs::user_requested` and passes every other field unchanged to `cancellai_safety::effective_authority`; protected, low-confidence, artifact-ceiling, and untrusted-provider counterexamples stay capped. No production caller of `resolve_effective_authority` exists. See `E11-S02/SAFETY_VERDICT.md`. |
| E11-S03 | PASS | `explain_policy` directly reshapes one `EffectiveAuthority` value: each step's `bound` flag is derived from that value's `binding_constraints`, top-level bindings are moved verbatim, and `suppressed` is equivalent to request exceeding the minimum. Repeated/tied-ceiling cases are deterministic. |
| E11-S04 | FAIL | Two selected eligible artifacts sized `u64::MAX - 1` and `2`, under `u64::MAX` pressure, panicked at `freed_bytes += size`. Repaired with saturating accounting and regression; pinning correctly feeds `ProtectionState::Pinned` into the existing lifecycle ceiling. Awaiting round-2 verification. |

## Findings and repairs

### F-01 — implementation bug: duplicate scoped keys selected a later, stronger request

Reproduction before repair:

```json
{
  "schema_version": 1,
  "providers": {
    "codex": {"authority": "observe"},
    "codex": {"authority": "autopilot"}
  }
}
```

`parse_policy` returned a document whose `providers["codex"].authority` was `Autopilot`.
The ordinary `BTreeMap` deserializer overwrote the earlier occurrence. The repair applies a
single `deserialize_unique_scope_map` visitor to all four keyed scope maps and returns
`PolicyError::Malformed` for a duplicate. `POLICY_MODEL.md` now documents that refusal.

### F-02 — implementation bug: budget accounting panicked at the numeric boundary

With candidate sizes `u64::MAX - 1` and `2`, `current_usage_bytes = u64::MAX`, and
`budget_limit_bytes = 0`, the old addition panicked in debug builds before returning selection.
The repair uses `saturating_add`; the regression confirms both eligible candidates are selected,
the reported freed total is `u64::MAX`, and pressure is satisfied. This cannot widen candidate
eligibility.

Both defects are repaired locally. The 50% first-round rejection yield requires a round 2 under
ADR-0025; E11-S01 and E11-S04 remain `in_progress` until that round passes.

## Counterexamples and falsification axes

| Axis | Independent case and result |
| --- | --- |
| Path / identity | Not applicable to pure policy parsing/resolution; pinning composes only a `ProtectionState`, and no mutation caller exists. |
| Partial reads / permissions | Not applicable: policy accepts supplied typed data and performs no filesystem I/O. `LowUnknown` remains `Recommend` through safety. |
| Links / mounts / reparse points | Not applicable to this pure decision layer; no path is accepted or resolved. |
| Provider layout drift | Unknown `PolicyContext` axes do not match scoped entries; only global/fallback can apply. |
| Concurrency | Inputs are borrowed/read-only and resolution is deterministic; repeated identical calls produced equal explanation/selection results. |
| Crash / failure / retry | Malformed and duplicate policy fails before a result; saturating budget arithmetic eliminates the reproduced overflow panic. |
| Boundary values | Missing/null/unsupported schema versions reject; empty/whitespace scope keys reject; `0d` parses; extreme budget selection saturates safely. |
| Policy / trust conflicts | 32 scope-ladder subsets are deterministic; a specific `Autopilot` request remains bounded by artifact, provider trust, lifecycle, confidence, and constitutional constraints. |
| Platform differences | No platform-specific code, paths, or syscalls were added. |
| Malformed / untrusted input | Invented nested duplicate map key now rejects; unknown fields at all three document-struct levels and executable-shaped fields reject. |
| Performance / large datasets | Schema test parses 2,000 provider keys; scope maps are ordered. Budget selection sorts only supplied candidates and has no recursive walk. |

## Gates actually run

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS; scheduled heavy performance tests remain intentionally ignored |
| `cargo deny check` | PASS; existing unmatched-license and duplicate-dependency warnings only |
| Focused duplicate-key and overflow regressions | PASS after repair; both failed before repair as reproduced above |
| Repository Python/governance gates listed by the review handoff | Pending final record/status generation in this review session |

## Documents opened

`AGENTS.md`, `docs/INDEX.md`, `docs/CONSTITUTION.md`,
`docs/development/ENGINEERING_SYSTEM.md`, `docs/development/AGENT_PROTOCOL.md`,
`docs/development/WORK_ITEM_MODEL.md`, `docs/development/RELEASE_GATES.md`,
`docs/architecture/POLICY_MODEL.md`, `docs/architecture/DOMAIN_MODEL.md`,
`docs/security/SAFETY_INVARIANTS.md`, `docs/security/THREAT_MODEL.md`,
`docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`,
`project/epics/E11.json`, `project/risk_floors.json`, and
`project/templates/SAFETY_VERDICT.md`.

## Round verdict

**FAIL — round 1 yielded 2 rejected stories of 4 (50%).** Both are repaired, but the
measured-yield rule requires a focused round 2 before E11-S01/E11-S04 can close. No release is
cut by this review.
