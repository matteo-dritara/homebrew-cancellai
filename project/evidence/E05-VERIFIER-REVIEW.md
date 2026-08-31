# E05 Independent Verifier Review - Round 1

- Review target: `5d62a00..44c175b`
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-08-31
- Epic: E05 - Provider API and Reference Adapters

All five stories were `ready_for_review` before review began. This review reconstructed the
story contracts and safety obligations from the control plane and linked architecture/security
documents; it did not use executor reasoning as evidence.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E05-S01 | PASS | `CapabilityKind::ALL` covers all nine documented capabilities. `CapabilityOutcome` makes confidence mandatory and requires at least one evidence entry. The trait has one explicit per-capability method; a provider identity supplies no default support claim. Workspace tests pass. |
| E05-S02 | FAIL | Public `cancellai_model::ProviderTrust` variants and public `AuthorityInputs::provider_trust` allow an external caller to pass `BuiltinVerified` directly to `effective_authority`, obtaining the highest trust ceiling without `trust_promotion::promote` or provenance. Separately, `mutation_executor::execute` consumes plan-recorded authority rather than the trust-aware computation. This violates AC1, AC2, SI-021, and SI-022. See `project/evidence/E05-S02/SAFETY_VERDICT.md`. |
| E05-S03 | PASS_WITH_RESIDUALS | The five recipe-parity tests independently reproduce the normative fixture situations; the protected-name list is explicit, symlinked projects are not walked, partial companions remain marked partial, and unknown root fingerprinting caps detect/fingerprint at `Observe`. Residuals are correctly bounded to recipe rather than whole JSON-document differential parity, Rust simple lowercase rather than Python full casefold, and resolved-view loss when symlink resolution errors; all are additional classification coverage, not an alternate mutation path. |
| E05-S04 | PASS_WITH_RESIDUALS | Independent workspace tests exercise root/subagent grouping, unknown parents/cycles, symlinked directory non-descent, fake native-delete outcomes, timeout, and large pipe output. Native support remains distinct from filesystem fallback. The documented timeout residual is real: a malicious probe can leave a grandchild and reader threads alive for the grandchild's lifetime; it cannot make the caller wait or turn probe failure into support. Full JSON differential parity and shared casefold residuals remain as documented. |
| E05-S05 | PASS_WITH_RESIDUALS | Re-ran `scripts/check_provider_compatibility.py check`: deterministic 36-row per-capability matrix matches the committed generated block. It truthfully covers only default-root and unknown-layout scenarios, not version-tagged layouts; this is stated in the document and does not claim version-specific coverage. |

## E05-S02 required repair

Reproduction: use the public API as an external consumer would:

```rust
let authority = effective_authority(AuthorityInputs {
    user_requested: AuthorityLevel::Autopilot,
    artifact_ceiling: AuthorityLevel::Autopilot,
    confidence: KnowledgeConfidence::Verified,
    activity: ActivityState::Orphaned,
    protection: ProtectionState::Normal,
    integrity: IntegrityState::Healthy,
    provider_trust: ProviderTrust::BuiltinVerified,
});
assert_eq!(authority.level, AuthorityLevel::Autopilot);
```

No `TrustPromotionEvidence` or call to `promote` is involved. This is precisely the hostile
manifest/self-assignment path SI-021 prohibits; passing tests only prove the mapping after a
caller chooses a tier, not that the choice is authorized.

Required repair: replace raw caller-supplied trust in authority inputs with a safety-owned,
opaque trusted-tier capability that defaults to `Untrusted` and is minted only by promotion
after non-forgeable provenance validation. Then require the resulting effective authority as a
sealed-plan/execution precondition. Add an external-consumer regression proving a manifest
claim cannot obtain `Govern` or `Autopilot` without the gate.

## Gate status

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS after approved advisory-cache access; three unmatched-license-allowance warnings only |
| `.venv/bin/python -m pytest tests -v` | PASS: 179 tests, 22 subtests |
| `.venv/bin/python -m ruff check .` and `ruff format --check .` | PASS |
| `.venv/bin/python -m mypy ... check_provider_compatibility.py` | PASS |
| Generated docs, project OS, docs, workflows, fixtures, schemas, characterization, differential harness, Rust workspace, mutation boundary, provider compatibility, process, and release checks | PASS |

The system `python3` lacks `pytest`; its attempted package install was refused by its
externally-managed environment. The existing repository `.venv` provided the pinned tools for
the successful full Python gate run.

## Overall verdict

**FAIL - round 1 of at most 2.** E05-S01 is verified; E05-S03 through E05-S05 are verified
with the recorded residuals. E05-S02 returns to `in_progress` for the required trust-boundary
repair. The epic remains `in_progress` and has one review round remaining.
