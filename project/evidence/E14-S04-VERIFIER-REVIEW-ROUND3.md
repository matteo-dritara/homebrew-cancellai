# E14-S04 Independent Verifier Review — Round 3

Review-Scope: epic
Round: 3
Review-Target: `5d0329e..fa3657b`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This owner-authorized final round is limited to E14-S04. E14-S01, E14-S02, and E14-S03
were already closed and are not re-judged.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | FAIL | An external-crate-style integration probe obtained a real public `Drifted` `LayoutDriftFinding` from `assess_layout`, observed `Observe` through the bridge, then called public `cancellai_safety::effective_authority` with separately constructed, otherwise identical destructive-capable `AuthorityInputs { provider_capability_ceiling: None, .. }`. That call returned `Autopilot` while the caller still held the real finding. The mandatory field requires spelling `None`; it does not bind the field to the finding or prevent a caller from discarding the finding. `compute_effective_authority` remains a separately public unconstrained construction API as an additional bypass surface. |

## Independent adversarial reproduction

I added a temporary `cancellai-guardian` integration test, compiled as a downstream crate
using only public APIs, then removed it before recording this verdict. It:

1. Constructed a real layout mismatch through `assess_layout("recognized-provider-name", known,
   observed)` and confirmed its public accessor returned `Some(AuthorityLevel::Observe)`.
2. Built maximally permissive authority inputs with a legitimately promoted `TrustedTier` using
   public `TrustPromotionEvidence`; this avoids a test-only trust constructor.
3. Confirmed `effective_authority_after_layout_assessment(inputs, &finding)` returned `Observe`.
4. While retaining that same real `finding`, separately constructed public `AuthorityInputs` with
   every other input unchanged and `provider_capability_ceiling: None`, then called the equally
   public `effective_authority`. It returned `Autopilot`.

The probe passed: it reproduced the bypass rather than asserting the intended safe result. The
existing `compile_fail` doctest also passed: an external crate cannot fabricate a
`LayoutDriftFinding` by struct literal. That is a genuine improvement over round 2, but it does
not close the more fundamental discard path: safe Rust does not require a caller that possesses
a finding to carry it when it constructs the separate public authority input.

`compute_effective_authority(&[AuthorityConstraint { name: "caller_asserted", ceiling:
AuthorityLevel::Autopilot }])` is also publicly callable and returns `Autopilot` without any
`AuthorityInputs` at all. The round-3 direct-`effective_authority` reproduction is sufficient to
reject the story; this generic public API confirms that the claimed single mandatory
authority-construction boundary still does not exist.

## Assessment against AC1 and SI-004

The two `cancellai-policy` call sites stating `provider_capability_ceiling: None` are locally
honest: no live Guardian assessment reaches either scope, so they do not falsely claim a
recognized layout. The same `None` vocabulary is nevertheless structurally insufficient as an
authority input: it conflates "not assessed" with "assessed recognized" and is publicly
assertable by a caller which has in fact assessed and obtained drift. A comment cannot supply
the missing provenance or impose an obligation on every public caller.

The absence of live provider-root-to-authority wiring is plausibly outside this detection
story's primitive scope, consistent with the stated E14 objective and the comparable Guardian
primitives. It would be an acceptable disclosed residual only after a real drift finding became
non-discardable at the authority boundary. It does not cure the failure here: ADR-0034 expressly
narrows its claimed closure to a finding "once obtained," while the reproduction proves that
such a finding can still be ignored in safe public API use. Consequently AC1's automatic
downgrade and SI-004 remain unmet even on the ADR's own narrowed framing; this is not merely a
future-orchestrator gap.

`LayoutDriftFinding` itself is now non-forgeable outside `structural.rs` production code: its
fields are private, `for_tests` is `#[cfg(test)] pub(crate)`, and the external construction
doctest passes. No production construction path outside `assess_layout` was found. The failure
is that the protected result is not the authority input type and is therefore freely discardable.

## Required owner disposition — no fourth patch cycle

This is a structural failure, not a missing regression test. The `Option<AuthorityLevel>` field
is caller-asserted data, whereas the required fact is an assessed capability state whose
provenance must survive into authority construction. A further E14-S04 patch that retains public
construction of `AuthorityInputs` with `None`, plus a separate optional bridge, would repeat the
same approach and is not authorized by the owner's two-round-per-approach limit.

The owner must choose a structurally different design before another implementation attempt. It
must make the capability assessment an unforgeable, non-discardable authority input (or make the
real authority-resolution boundary own and obtain that assessment) and restrict/migrate both
the public `effective_authority` and generic `compute_effective_authority` paths so they cannot
produce a destructive authority after a drift result is available. The owner must also decide
the scope/signature that permits the live provider-root assessment to reach the artifact/session
authority resolver. The follow-on work item, rather than E14-S04 round 4, must own that design.

## CR4 Safety Verdict — E14-S04

`FAIL`

The risk classification remains CR4. The reviewed repair modifies
`rust/crates/cancellai-safety/src/authority.rs`, the safety-kernel authority boundary covered by
the CR4 floor, and purports to constrain destructive authority. Its smaller diff does not reduce
that risk. No PASS or PASS_WITH_RESIDUALS Safety Verdict is supportable: a public caller holding
a real drift finding can retain `Autopilot`, violating SI-004, C-02, C-05, and TM-05.

## Gate status

| Command | Result |
| --- | --- |
| Temporary `cargo test -p cancellai-guardian --test e14_round3_independent -- --nocapture` | PASS — reproduced the public API bypass; temporary source removed. |
| `python3 scripts/project_os.py check` | PASS before review-state update. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS, including the external-construction compile-fail doctest. |
| `cd rust && cargo deny check` | PASS after approved rerun outside the sandbox could acquire Cargo's read-only advisory lock; existing warnings only. |
| `python3 -m pytest tests -v` | PASS — 668 tests, 584 subtests. |
| AGENTS.md Python check list | PASS using the committed `.venv` for Ruff/Mypy; the active `python3` environment lacks those two modules. All listed repository checkers passed. |
| `gh run list --branch main --limit 5` | UNKNOWN — this session could not connect to `api.github.com`; unknown is not treated as green. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E14.json`
- `docs/adrs/0034-provider-capability-authority-is-a-mandatory-authorityinputs-field.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/DOMAIN_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `project/evidence/E14-S04/VERIFIER_BRIEF.md`
- `project/evidence/E14-S04/EVIDENCE.md`
- `project/evidence/E14-VERIFIER-REVIEW.md`
- `project/evidence/E14-VERIFIER-REVIEW-ROUND2.md`
- changed Guardian, policy, and safety authority sources and their public consumers

## Overall verdict

E14-S04 returns to `in_progress`; E14 remains `in_progress` and cannot close or prepare PD-021
release evidence. The final-round ceiling is reached with a surviving structural finding. No
fourth patch/review loop is authorized; a new owner-decided approach and follow-on work item are
required.
