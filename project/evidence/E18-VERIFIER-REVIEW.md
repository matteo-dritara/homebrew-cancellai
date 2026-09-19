# E18 Independent Verifier Review — Round 1

Review-Scope: epic
Round: 1
Review target: `96f645e..a3d8aba`
Verifier: Codex
Date: 2026-09-19

## Handoff identity

### E18-S01

Brief-Checksum: f500b766be7c4210fb12ed5fcd451504e0665a83b52153d394f5c1779263efb0
Verifier: Codex

### E18-S02

Brief-Checksum: ababc394cafa0c872500d5cbf99e99abcb7a2786c8e2a89a351740cc55021ed3
Verifier: Codex

### E18-S03

Brief-Checksum: b1a9bb24614ecf62be2b39170d95ad793cb6ed9fcb967cc3a3e8a7a3c85e061a
Verifier: Codex

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E18-S01 | PASS | `InventoryOrigin::Local` has no `MachineId`, while a remote origin is necessarily `Remote(MachineId)`, so even empty, `local`, `localhost`, Unicode, or control-character ids cannot compare equal to local. `RemoteTarget::new` starts disconnected and `state_is_current()` is true only for `Connected`; the lifecycle transition tests cover disconnect and reconnect. The model remains intentionally unwired to inventory/planning in this story. |
| E18-S02 | FAIL | The public `verify_remote_execution_request` is explicitly stateless and returns `Ok` for the same valid signed request repeatedly; it bypasses `RemoteExecutionLog` altogether. An independent temporary test called it twice with the identical signed sequence-5 request and both calls succeeded. A new/clone/reset in-memory log likewise loses the prior sequence. Separately, a verified request for `ci-runner-1` exposes a public bare `requested_authority`; `AuthorityInputs` contains no target, so the same value can be supplied while acting on `another-machine`. An independent temporary test demonstrated that mismatch and reached `Quarantine`. Neither path is exercised by the committed tests. This violates AC1, AC2's replay requirement, SI-031, C-01, C-05, and TM-17. A CR4 Safety Verdict is not supportable. |
| E18-S03 | FAIL | The claimed open protocol boundary is the E18-S02 public API and inherits its replay bypass and target-confusion path. Therefore a commercial/self-hosted transport can use the stateless verifier or apply an accepted request to a different target, contrary to the claim that it has no privileged route into local authority. The sole offline test constructs an empty `TrustedRemoteControllers`, never passes it to the local decision, and only asserts the pre-existing `effective_authority` result; it is tautological evidence for the three acceptance criteria. This violates AC2, AC3, SI-031, C-01, C-05, and TM-17. A CR4 Safety Verdict is not supportable. |

## Required repairs and reproductions

### E18-S02

1. **Replay bypass reproduction.** A valid request was created with the module's test signer and
   policy, then `verify_remote_execution_request(&request, &policy, 1_500)` was invoked twice.
   Both calls returned `Ok`; the temporary independent test
   `verifier_api_accepts_an_identical_signed_request_twice_without_a_log` passed. Its public
   documentation confirms this is intentional: the function performs every check except
   replay/staleness. A process restart or a fresh/clone log provides the same bypass.

2. **Target-confusion reproduction.** A valid request naming `ci-runner-1` was verified, its
   public `requested_authority` was placed in `AuthorityInputs::user_requested`, and the
   hypothetical local target was `another-machine`. There is no target field or comparison in
   `AuthorityInputs`/`effective_authority`; the temporary independent test
   `verified_request_for_one_target_can_supply_authority_for_another_target` passed and yielded
   `Quarantine` authority. The temporary tests were removed after execution; the review leaves
   no implementation changes.

3. **Required repair.** Provide one mandatory, non-bypassable acceptance operation for remote
   inputs: it must verify signature, schema, expiry, controller ceiling, a shared per-controller
   replay state, and equality with the target node's expected `MachineId` before yielding a value
   usable as `user_requested`. The stateless verifier must not remain a public production path
   that yields usable authority. Replay state must be injected through a durable outer-ring
   boundary (or the primitive must fail closed without it), so restart, concurrent handlers, and
   controller-key/ceiling rotation cannot reset the sequence floor. Add adversarial tests for
   two direct verifier calls, restart/fresh state, concurrent serialisation, `u64::MAX`, target
   mismatch, duplicate controller ids/key rotation, wrong keys, malformed signatures, and a
   request above a lowered ceiling.

4. **Design-record repair.** RFC-0001 and ADR-0031 say the signed payload carries an
   `ActionClass` and that controller policy bounds action classes; the implementation and current
   architecture documentation instead sign an `AuthorityLevel`. Reconcile this material protocol
   and authority-boundary change through the accepted design record before claiming the code
   implements it. This documentation inconsistency violates C-18's evidence-gated delivery even
   apart from the runtime failures.

### E18-S03

1. **Required repair.** After E18-S02 supplies a non-bypassable target-bound, replay-safe
   primitive, replace the unused-empty-policy test with offline conformance tests that prove the
   local CLI/TUI authority path does not require controller configuration, an unconfigured or
   unreachable controller cannot affect it, and every coordinator producer (including a
   commercial one) reaches only that same checked open protocol operation. Update the product
   claim only to the boundary the repaired API actually enforces.

## Test-quality findings

- E18-S02's committed replay tests cover `RemoteExecutionLog::verify_and_record`, but not the
  simultaneously exported `verify_remote_execution_request` path that returns a usable verified
  intent without replay checking. They do not cover restart/clone state or target matching.
- `remote_and_local_user_requested_reach_identical_effective_authority` proves the authority
  lattice treats two raw values alike, not that a remote value is authenticated, replay-safe, or
  bound to the target consuming it.
- E18-S03's `a_local_authority_decision_requires_zero_trusted_remote_controllers` deliberately
  leaves `empty_policy` unused. It proves no protocol or offline boundary property beyond the
  existing function signature.

## Gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS, exit 0. |
| `python3 scripts/project_os.py review` | PASS, exit 0; listed E18-S01, E18-S02, and E18-S03 for independent review. |
| `git diff --check 96f645e..a3d8aba` | PASS, exit 0. |
| `cargo fmt --check` | PASS, exit 0. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS, exit 0. |
| `cargo check --workspace --all-targets` | PASS, exit 0. |
| `cargo test --workspace` | PASS, exit 0. |
| `cargo deny check` | PASS, exit 0; emitted configured unmatched-license and duplicate-version warnings only. |
| Targeted independent adversarial tests described above | Both PASS, demonstrating the two failure reproductions; removed after execution. |
| `python3 scripts/verifier_handoff.py check` | PASS, exit 0; every verdict that names a rendered brief answers the committed brief. |

## Documents opened

`AGENTS.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/CONSTITUTION.md`;
`docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`;
`docs/architecture/TARGET.md`; `docs/PRODUCT.md`;
`docs/adrs/0019-dependency-rings-per-crate.md`;
`docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md`;
`docs/rfcs/0001-remote-execution-boundary.md`;
`docs/adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md`;
`docs/architecture/DOMAIN_MODEL.md`; `project/epics/E18.json`; and all three rendered E18
verifier briefs under `project/evidence/E18-S01/`, `E18-S02/`, and `E18-S03/`.

## Overall round verdict

`FAIL` — E18-S01 passes, but E18-S02's CR4 remote authority boundary has reproducible replay and
target-binding bypasses. E18-S03 depends on and documents that unsafe boundary, with insufficient
offline conformance evidence. Two of three judged stories fail (66.7% finding and rejection
yield), so a further independent review round is mandatory after repair. No CR4 Safety Verdict is
supportable for E18-S02 or E18-S03 on this evidence.
