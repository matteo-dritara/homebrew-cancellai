# E18 Independent Verifier Review — Round 2

Review-Scope: epic
Round: 2
Verifier: Codex
Date: 2026-09-19
Review target: `a3d8aba..HEAD`, with HEAD fixed at `2fe937d55eb95557cb4be2e4dcd2b632743790dd`.

Scope is restricted to `rust/crates/cancellai-safety/`, `rust/crates/cancellai-model/`,
`project/evidence/E18-*`, `docs/adrs/0032-remote-execution-requests-carry-actionclass-not-authoritylevel.md`,
`docs/architecture/TARGET.md`, `docs/security/THREAT_MODEL.md`, and `docs/PRODUCT.md`.
Supporting callers, manifests, and governance documents were inspected for context, not repaired.

Independent-person review: Codex, separate from executor Claude. No executor private reasoning
was requested or used. Both E18-S02 and E18-S03 were confirmed `ready_for_review` before review.
E18-S01 is excluded, retaining its round-1 PASS; the ledger still says `ready_for_review`, rather
than closed as the handoff described. No status was changed.

## Handoff identity

### E18-S02

Brief-Checksum: ababc394cafa0c872500d5cbf99e99abcb7a2786c8e2a89a351740cc55021ed3
Verifier: Codex

### E18-S03

Brief-Checksum: b1a9bb24614ecf62be2b39170d95ad793cb6ed9fcb967cc3a3e8a7a3c85e061a
Verifier: Codex

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E18-S02 | FAIL | The original public stateless call now fails compilation (E0603); the checked public operation rejects duplicate sequences and a mismatched expected target. However, `VerifiedRemoteIntent` remains publicly constructible and writable (`remote_execution.rs:253–258`). An independent external crate manufactured one without any request/signature/policy/log, and separately changed a successfully verified Observe request under an Observe ceiling to Delete on another target. The documented conversion then produced Govern, including through `effective_authority` with permissive synthetic local inputs. Thus the claimed mandatory verification path and preservation of its target/ceiling checks are still false. |
| E18-S03 | FAIL | AC2 and SI-031 rely on precisely that E18-S02 public API as the open commercial/self-hosted boundary. It exposes the same forgeable/mutable verified result. The revised offline test now really passes the empty policy through verification and rejects the unknown controller, so that specific unused-policy defect is repaired; it does not fix the inherited authority-boundary defect. |

## F1 — Verified result can bypass or discard verification (both stories)

Location: `rust/crates/cancellai-safety/src/remote_execution.rs:245–258`, with sole-production-path
claims at lines 283–285 and 365–366. The type is also exported from `src/lib.rs`.

The private helper repair closes the literal round-1 function-call reproduction, but **does not
make the checked log operation the only producer**. All four result fields are public. This is
an incomplete repair of round 1's non-bypassable acceptance requirement, not a request for durable
storage inside the kernel and not an ActionClass deserialization defect.

Minimal external-crate reproduction, requiring only path dependencies on the model and safety
crates, compiles and runs against the reviewed HEAD:

```rust
use cancellai_model::{ActionClass, AuthorityLevel, MachineId};
use cancellai_safety::{VerifiedRemoteIntent, authority::minimum_authority_for};

let fabricated = VerifiedRemoteIntent {
    controller_id: "not-trusted".into(),
    sequence: 1,
    target: MachineId::new("synthetic-node-B"),
    requested_action: ActionClass::Delete,
};
assert_eq!(minimum_authority_for(fabricated.requested_action), AuthorityLevel::Govern);
```

No request was authenticated, authorized, target-checked, or recorded. A second independent test
signed an Observe request for synthetic-node-A, accepted it through `verify_and_record` with an
Observe ceiling, and then executed these legal external writes:

```rust
verified.target = MachineId::new("synthetic-node-B");
verified.requested_action = ActionClass::Delete;
assert_eq!(minimum_authority_for(verified.requested_action), AuthorityLevel::Govern);
```

The probe also supplied that computed value to `effective_authority` with locally established,
synthetic permissive inputs and received Govern. Under the original Observe request the remote
ceiling was Observe. Changing the accepted result requires no new signature or log operation.
The controller id and sequence are equally writable, so result provenance is not immutable either.

**Threat qualification:** this is a public Rust API boundary failure, not evidence that a network
JSON payload bypasses `verify_and_record`. No production transport exists, and a malicious local
program could already ignore the whole API. The defect matters because this story explicitly
claims structural containment against caller bypass, exactly the standard used by round 1 and
by the repository's opaque `TrustedTier` precedent. A type documented as verified cannot provide
that guarantee while an ordinary adapter can fabricate or rewrite it. No filesystem mutation or
real user data was involved in these reproductions; other local safety ceilings still apply.

**Required repair — E18-S02:** make `VerifiedRemoteIntent` opaque and immutable to external
callers: private fields, read-only accessors, no unchecked public constructor/conversion or
Deserialize route. Keep successful construction behind the target-bound, replay-checking log
operation. Add external/compile-fail regressions for struct-literal construction and writes to
all checked fields, plus positive public-API tests proving the returned target/action/provenance
match the signed request and configured ceiling. Update the sole-producer claims to precisely
what is enforced. This repair needs no I/O or new dependency.

**Required repair — E18-S03:** depend on that repaired result boundary, exercise the public
operation in its conformance coverage, and align the product/evidence claims with the immutable
checked result and the explicitly deferred integration guarantees. A doc-only assertion cannot
close this inherited defect.

Violations: E18-S02 AC1/AC2; E18-S03 AC2; SI-031; C-01/C-05 and TM-17's local authorization
boundary. The claim that the type proves verification additionally conflicts with C-18.
Neither story receives a passing CR4 Safety Verdict on this evidence.

## Repairs independently confirmed and adversarial coverage

The independent probe is an external crate in `/tmp/e18-round2-review/probe`, not a test inside
`remote_execution` with access to its private helpers. It constructs its own framed payload and
signing bytes, uses a synthetic Ed25519 key, and calls only the public API. The final run used a
copy of the workspace Cargo.lock as its dependency-resolution baseline. Ten tests passed;
two intentionally establish F1 and another reproduces the disclosed replay-state residual.
An initial extension of the scratch probe used incorrect vocabulary names; those probe-only
compile errors were corrected before the final successful run. No repository code was edited.

- **Original stateless bypass:** importing `remote_execution::verify_remote_execution_request`
  from the external crate fails with E0603. The root re-export is also removed. Calling the
  public log twice with the same valid signed sequence-5 request produces Ok then Stale.
- **Target confusion at acceptance:** a genuinely signed node-A request checked for node-B
  produces TargetMismatch. The rejected attempt does not consume the sequence; checking it
  for node-A then succeeds. Rewriting a request target and recomputing its digest without
  re-signing produces InvalidSignature. Preservation after acceptance fails as F1 describes.
- **ADR-0032 wire vocabulary:** `requested_action` accepts only ActionClass; `autopilot`,
  `recommend`, `govern`, an unknown future variant, numeric 3, and null are rejected. Omitting
  `requested_action`, substituting the old `requested_authority`, including both fields, and
  duplicating `requested_action` are rejected. Shared names such as `observe` are semantic
  actions here; there is no independently supplied AuthorityLevel field.
- **Exact mapping and ceilings:** an exhaustive independent match asserts Observe -> Observe,
  Quarantine/Archive/Restore -> Quarantine, Delete -> Govern. All 25 combinations of the five
  actions and five ceilings were verified through the public operation. Only combinations
  at or below the independently expected authority pass. Neither enum discriminant casts nor
  ActionClass declaration order participate in the calculation. Production code explicitly
  calls `minimum_authority_for(request.requested_action)` before comparing the authority ceiling.
- **Action binding:** Observe -> Delete without updating the digest produces
  ContentDigestMismatch; recomputing that digest without re-signing produces InvalidSignature.
  The independently generated valid request succeeds, demonstrating agreement on actual
  framing rather than copying the private signer. Sequence, issued_at, expiry, and target
  tampering also fail signature verification. Wrong keys, zero signatures, non-ASCII/malformed
  hex, odd-length hex, and empty signatures are refused.
- **Replay/boundaries/concurrency:** sequence 0 is accepted as a first sequence; after u64::MAX,
  zero, MAX-1, and MAX are all stale, with no increment/wraparound. Twelve concurrent requests
  serialized through one Mutex-protected log yield exactly one success and eleven Stale errors.
  A rotated key using the same controller id cannot reset the shared sequence floor. Lowering
  the controller ceiling refuses Delete. Expiry equal to now refuses. Duplicate configured ids
  use the first entry: a later permissive entry does not override the first restrictive one.
  Duplicate-id ordering is an operator-configuration concern, not wire-controlled escalation.
- **Local safety/offline behavior:** an unconfigured controller is refused, while an independent
  local Govern decision with synthetic trusted local inputs is unchanged before/after that
  refusal. Protected state and unknown integrity still lower a verified Delete request below
  Quarantine. Static call-site search found no production remote caller; mutation-boundary and
  workspace isolation checks pass. Nothing in the changed implementation adds network or
  filesystem calls or platform-specific branches.

## Test-quality and documentation observations

`no_action_class_ever_maps_to_recommend_or_autopilot` checks all five **current** variants via a
hand-maintained array. It will still pass if a sixth variant is added and mapped to Autopilot,
unless the author remembers to extend that array. The production exhaustive match forces a
mapping decision for a new variant, but does not force the regression's list to grow. Therefore
TARGET.md's assertion that this test proves the range can *never* change is too strong. Current
behavior is correct; this is a regression-coverage weakness, not a presently reachable wire bypass.
Use an exhaustiveness-enforced enumeration/guard and test the real public acceptance path when
strengthening coverage. The independent probe used an exhaustive match to check today's mapping.

The new E18-S03 test repairs the unused empty policy: it now asserts UnknownController from the
real log path. Its local half remains a separately invoked authority calculation capped at Observe
by untrusted provider trust. It cannot establish end-to-end CLI/TUI availability or prove an
unreachable transport will not block a future caller. The independent local Govern case improves
current library evidence, but no transport exists to test end-to-end. Do not describe the test as
that broader proof. `docs/PRODUCT.md` and the early AC tables in EVIDENCE.md still cite the removed
`a_local_authority_decision_requires_zero_trusted_remote_controllers` test; early evidence tables
also retain `requested_authority` wording. Later dated sections explain the correction, but current
claim/reference text should be aligned by the executor. These observations are not additional
independent story rejections beyond F1.

## Durable replay-state residual: accepted as unresolved, not the reason for FAIL

The probe confirms that a fresh log, or two clones made before accepting a request, each accept
that request. Protection is per retained log instance, not durable across restarts or across
independent handlers. `&mut self` prevents simultaneous unsynchronized access to the same instance;
it does not provide global coordination. No durable restore/export interface or caller exists.

Deferring persistence to an outer-ring integration is sound. ADR-0019 explicitly governs dependency
rings and keeps SQLite out of the safety kernel; it is not literally a blanket prohibition on all
kernel I/O (platform/sealedfs are kernel members). The pure safety-module architecture and absence
of a production caller nevertheless justify an outer-ring durable design rather than adding
storage here. The evidence packets honestly state the gap, including that a CR4 claim of complete
replay prevention cannot yet be granted. I do **not** reject either story merely because this
accepted residual exists.

Before real remote execution, the future caller must provide shared, durable replay acceptance,
crash-safe ordering relative to audit/execution, appropriate recovery semantics, current local
policy/target identity, and audit persistence. In-memory verification alone cannot discharge those
requirements. TARGET.md's “if a future caller needs that” should be understood as mandatory for a
production replay-prevention claim, not optional hardening; TM-17's “replay-protected” currently
means only the shared-log lifetime. Transport confidentiality/availability, trusted clock input,
operator trust configuration, and actual local plan construction/revalidation remain caller
responsibilities. None is silently claimed tested here.

## CR4 Safety Verdicts

**E18-S02: FAIL.** Surface: remote authentication/authorization, target binding, replay admission,
and ActionClass-derived authority input. SI-031 is preserved within the checked function but not
by the public verified-result boundary (F1). A passing CR4 verdict is not supportable until F1 is
repaired and independently verified. Durable replay/audit integration remains unresolved even
then; no verdict here authorizes production remote mutation.

**E18-S03: FAIL.** Surface: the open local protocol offered equally to commercial and self-hosted
coordinators. Local offline authority remains independent, but AC2 inherits F1. A passing CR4
verdict is not supportable. It inherits the durable replay/audit and transport residuals above.

Compatibility: the ActionClass wire change is intentionally breaking per accepted ADR-0032; no
production producer/transport exists. Current parsing and signature tests support that correction.
Rollback/recovery: retain the unwired state; reverting the remote module addition removes this
entry surface without changing existing local authority callers. There is no durable remote state
or deployed wire producer to migrate in this implementation. This is not a tested production
recovery mechanism for a future transport.

Owner decision: pending; no owner acceptance or status transition is written by this review.

## Gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS, exit 0. |
| `python3 scripts/project_os.py review` | PASS, exit 0; all three E18 stories listed, only S02/S03 judged. |
| `python3 scripts/project_os.py status` / `next` | PASS, exit 0; status read only. |
| `python3 scripts/check_agent_toolchain.py report` | PASS, exit 0; no recorded component review past its date. No component installed/updated/removed. |
| `git diff --check a3d8aba..HEAD` | PASS, exit 0. |
| `cargo fmt --check` | PASS, exit 0. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS, exit 0. |
| `cargo check --workspace --all-targets` | PASS, exit 0. |
| `cargo test --workspace` | PASS locally, exit 0. |
| `cargo deny check` | Initial attempt blocked by read-only advisory lock (exit 1); rerun with approved access PASS, exit 0: advisories/bans/licenses/sources all ok; unmatched-license and duplicate-version warnings. |
| `cargo test --offline --manifest-path /tmp/e18-round2-review/probe/Cargo.toml --lib` | PASS, ten independent tests, including reproduced F1 and accepted replay-state residual. |
| `cargo check --offline --manifest-path /tmp/e18-round2-review/probe/Cargo.toml --bin private_api` | Expected compile failure, exit 101, E0603: stateless verifier is private. |
| `python3 scripts/check_mutation_boundary.py check` | PASS, exit 0; 87 Rust source files. |
| `python3 scripts/check_rust_workspace.py check` | PASS, exit 0; 13 crates, acyclic, model/safety isolated. |
| `python3 scripts/verifier_handoff.py check` | PASS before and after writing this review. |
| `gh run list --branch main --limit 5` and `gh run view 35443687056 --log-failed` | CI is NOT green: tests success (35443687110), governance success (35443687108), CodeQL success (35443687225), Rust failure (35443687056). Initial runs were pending; one intervening network lookup failed, followed by successful retrieval. |

The required Rust commands ran from `rust/` on the local macOS host. Cross-platform clippy was
not run: this diff changes neither platform code nor workspace lint policy. Windows/Linux behavior
is not independently certified by this local run. Full G1/G2 cannot pass due to F1; G3 cannot be
called green with the failed Rust CI run; production G4 crash/recovery/audit is deferred, not passed.
No Python implementation changed; the entire Python quality suite was not rerun.

### Out-of-scope CI finding for the owner

Rust CI's Windows quality job reports five `cancellai-store` test failures, each while cleaning up
a synthetic test directory, with Windows error 32 (“file ... being used by another process”):
`ledger::tests::open_refuses_a_provider_owned_file_that_only_mimics_this_ledgers_schema`,
`ledger::tests::open_refuses_before_mutating_a_file_at_an_intermediate_user_version_with_no_marker`,
`rollup::tests::open_refuses_a_provider_owned_file_that_only_mimics_this_modules_schema`,
`tests::open_refuses_before_mutating_a_file_at_an_intermediate_user_version_with_no_marker`, and
`tests::open_refuses_a_provider_owned_file_that_only_mimics_this_crates_schema`.
The safety crate's 108 Windows unit tests passed in that log. This does not establish the root
cause or excuse the red workflow; it is outside the requested E18 diff and is reported to the
coordinating owner/executor for separate disposition. No store fix or backlog/status edit was made.

## Documents actually opened

- `AGENTS.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/CONSTITUTION.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md` (including TM-17)
- `docs/architecture/TARGET.md` (including Remote execution boundary)
- `docs/PRODUCT.md` (including Open-source and commercial boundary)
- `docs/rfcs/0001-remote-execution-boundary.md`
- `docs/adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md`
- `docs/adrs/0032-remote-execution-requests-carry-actionclass-not-authoritylevel.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/development/RELEASE_GATES.md`
- `.github/SECURITY.md`
- `project/templates/SAFETY_VERDICT.md`
- `project/epics/E18.json`
- `project/evidence/E18-S02/VERIFIER_BRIEF.md` (full)
- `project/evidence/E18-S03/VERIFIER_BRIEF.md` (full)
- `project/evidence/E18-S02/EVIDENCE.md`
- `project/evidence/E18-S03/EVIDENCE.md`
- `project/evidence/E18-VERIFIER-REVIEW.md`

Source inspected includes `remote_execution.rs`, `authority.rs`, `trust_promotion.rs`, safety
`lib.rs`, model `vocabulary.rs`, and safety/CLI/TUI Cargo manifests. Temporary probes and logs
remain under `/tmp/e18-round2-review/`; the reproductions above are retained in this review so
its findings do not depend on those temporary files surviving. No vulnerability was posted to a
public service; this requested local review is the handoff for owner triage.

## Overall round verdict

**FAIL. E18-S02 and E18-S03 fail because the claimed checked remote-intent boundary remains
publicly forgeable and mutable (F1).** The ActionClass wire correction and literal private-helper /
expected-target repairs work, but do not close that boundary. Durable replay persistence is an
accepted, honestly disclosed open residual and is not counted as a fresh failure.

Two of two judged stories fail (100% rejection yield); another independent round is required
after repair under ADR-0025. Findings overlap round 1's mandatory acceptance/target-authority
boundary concerns; this is not a zero-overlap closure. A third round reaches the protocol's cost
ceiling and needs the coordinating owner's recorded decision. No story status, implementation,
generated project file, commit, or remote branch was changed by this review.
