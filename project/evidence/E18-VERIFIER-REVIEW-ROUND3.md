# E18 Independent Verifier Review — Round 3

Review-Scope: epic
Round: 3
Verifier: Codex
Date: 2026-09-19
Review target: `2fe937d..HEAD`, HEAD fixed at `4dcab698d2f5ea58a31c88efbfc711d3dc207d72`.

Reviewed diff restricted to `rust/crates/cancellai-safety/`, `project/evidence/E18-*`, and
`docs/PRODUCT.md`. Supporting types, callers, manifests, and governance were read for context.
The owner explicitly authorized this LAST round under ADR-0025's three-round cost ceiling;
this is not an automatic continuation or automatic epic closure. Codex is the independent
verifier, distinct from executor Claude. No executor private reasoning was requested or used.
Both scoped stories were confirmed `ready_for_review` in `project/epics/E18.json` and the review
queue before substantive review. E18-S01 is excluded and retains its round-1 PASS.

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
| E18-S02 | PASS_WITH_RESIDUALS | External struct construction fails E0451; four separate external writes after genuine verification fail E0616. No unchecked constructor, conversion, Deserialize, Default, mutable accessor, or enclosing public-field route was found. Clone preserves verified provenance and payload. Nine external runtime tests pass against current HEAD, including replay/target rejection and all 25 action/ceiling combinations. F1 is closed for this library boundary. Shared durable replay, audit/execution integration, and documentation qualifications below remain. A scoped CR4 Safety Verdict is supportable. |
| E18-S03 | PASS_WITH_RESIDUALS | The inherited F1 defect is closed. The public protocol has no producer-specific privileged path. Empty controller policy really refuses verification while an independent synthetic local Govern decision is unchanged; protected/unknown local inputs still lower remote-requested authority. CLI/TUI manifests and call-site search show no production remote integration. Offline library behavior is supported; future transport availability and execution guarantees are not claimed tested. A scoped CR4 Safety Verdict is supportable with the same integration residuals. |

No story FAILs. No repairs or status transitions were made by this review.

## F1 reproduction against current HEAD

The external scratch crate at `/tmp/e18-round3-review/probe` has path dependencies on the
current model and safety crates. I inspected and adapted the prior independent verifier's
scratch signer/tests, not executor reasoning. Its signer frames the protocol independently
and uses a synthetic Ed25519 key; it cannot call private module helpers. Dependency resolution
starts from the prior probe's lockfile. All data is synthetic; no filesystem mutation capability
or real provider data is involved.

The original reproduction now fails:

```rust
let forged = VerifiedRemoteIntent {
    controller_id: "untrusted".into(),
    sequence: 1,
    target: MachineId::new("B"),
    requested_action: ActionClass::Delete,
};
// E0451: all four fields are private.
```

Four separate binaries first build and sign an Observe request, accept it under an Observe
ceiling using the public `RemoteExecutionLog::verify_and_record`, and attempt one of:

```rust
v.controller_id = "forged".into();
v.sequence = 999;
v.target = MachineId::new("B");
v.requested_action = ActionClass::Delete;
// Each separate compilation fails E0616 for that field.
```

The same signer and successful checked-call path compile and execute in the positive runtime
suite. Thus these negative results are privacy errors, not missing dependencies or broken setup.
The committed `compile_fail` doctest also passes in `cargo test --workspace`.

Additional external compilations refuse the private stateless helper (E0603),
`serde_json::from_str::<VerifiedRemoteIntent>` (E0277, no Deserialize), and
`VerifiedRemoteIntent::default()` (E0599). Source inspection of the entire module and workspace
references finds one struct-literal producer in the private verifier, one public fresh-acceptance
operation, and only four immutable accessors. There are no From/Into implementations creating
this type from unchecked data, public enclosing containers exposing an unchecked instance,
DerefMut, mutable references, interior mutability, or serialization/deserialization derives on
this type. MachineId wraps a private String and exposes only `new`/`as_str`; `&MachineId` cannot
change the stored target. Copying an action or cloning a target produces detached data, not a
way to alter the verified result.

### Clone: another way to obtain an instance, not to fabricate verification

`VerifiedRemoteIntent` DOES derive Clone. Therefore the literal claim that the log is the
"only way to obtain one" is too broad: `v.clone()` (and cloning an enclosing Result/Option)
can produce another instance without a new log call. The independent positive test asserts
both equality and each accessor: controller, sequence, target, and Observe action under an
Observe ceiling all survive cloning unchanged. Clone requires a previously verified source;
`clone_from` likewise requires another verified source. Neither supplies arbitrary checked
fields or escalates the request. No safe public route to a fabricated or rewritten checked
result was found. Unsafe fabrication by an arbitrary local program is not the API guarantee.

This is an immutable verification record, NOT a linear, one-use execution capability. Keeping,
borrowing, moving, or cloning it does not re-check policy, time, or execution deduplication.
Even removing Clone would not by itself prevent reading its action repeatedly. The correct
claim is: all fresh checked provenance originates in successful public log admission; copies
preserve it. Exactly-once execution and current execution-time authorization belong to the
explicitly deferred integration below. Clone is not a new F1 rejection.

## Prior repairs and further counterexamples

The nine runtime tests passed, with the following concrete coverage:

- `public_api_replay_and_target_repair`: a signed node-A request checked for node-B returns
  TargetMismatch without consuming the sequence; node-A then succeeds; the duplicate is Stale.
- `all_actions_by_all_ceilings`: all 25 current combinations agree with an independent exhaustive
  match: Observe -> Observe; Quarantine/Archive/Restore -> Quarantine; Delete -> Govern.
  Requests above the ceiling are refused, not clamped.
- `wire_rejects_authority_missing_unknown_numeric_and_duplicate`: rejects Autopilot, Recommend,
  Govern, unknown/numeric/null actions, missing action, old requested_authority, extra authority
  alongside an action, and duplicate action keys.
- `signature_binds_action_and_every_envelope_field`: Observe-to-Delete tampering fails digest;
  recomputing the digest without signing fails signature. Sequence, issued_at, expiry, target,
  wrong-key, zero-signature, malformed/odd/non-ASCII/empty hex cases are refused.
- `max_sequence_rotation_lowered_ceiling_and_expiry`: zero is valid initially; after MAX,
  zero/MAX-1/MAX are stale; key rotation with the same controller id does not reset that log;
  a lowered ceiling refuses Delete; expiry equal to now refuses; a later duplicate configured
  id does not override the first restrictive entry.
- `shared_log_serializes_concurrent_duplicates`: twelve threads using one Mutex-protected log
  produce exactly one success and eleven Stale results.
- `documented_fresh_and_forked_log_residual`: fresh logs and logs cloned before admission each
  accept the same request. This reproduces the already accepted integration residual.
- `clone_and_accessors_preserve_checked_provenance`: all four accessors match signed inputs,
  a cloned intent is equal, and replacement of a detached target does not change the intent.
- `real_local_authority_and_safety_floors_remain_independent`: empty remote policy refuses;
  local Govern stays Govern; protected state and unknown integrity lower a verified Delete
  request below destructive authority.

Static search finds no production caller of remote verification anywhere else in the workspace;
only the safety crate's re-export appears outside the module. `minimum_authority_for` remains
an exhaustive semantic match, not a discriminant cast. Strict signature verification, target
comparison, expiry/ceiling refusal, and replay insertion ordering are unchanged by this diff.
Audit linkage means the Result plus original request contains audit fields; no persisted event
is claimed. Local authority remains a separate monotonic calculation, with no network dependency
introduced by this change.

### Adversarial axes

| Axis | Evidence / scope limit |
| --- | --- |
| Path / identity | Signed target tampering and expected-target mismatch tested. Actual filesystem identity must be bound by future local plan construction. |
| Partial reads / permissions | No I/O in this module; missing/malformed input is refused and unknown local integrity lowers authority. |
| Links / mounts / reparse | No path resolution or mutation added; future execution must retain existing root/plan checks. |
| Provider/layout drift | Schema refusal covered by workspace tests; no provider classification added. |
| Concurrency | Shared-log twelve-thread probe; independent logs remain a disclosed gap. |
| Crash / retry | Duplicate admission refused; fresh-log acceptance reproduces missing durability. No durable crash-recovery protocol exists to test. |
| Boundary values | Zero/MAX sequences, exact expiry, malformed signatures and all action/ceiling combinations. |
| Policy/trust conflicts | Unknown controller, wrong key, lowered ceiling, duplicate id, rotation, protected/unknown local constraints. |
| Platform | No platform branch changed; local gates run on macOS, matrix CI disposition below. |
| Malformed/untrusted input | Wire vocabulary and signed-field tamper probes; external checked-result fabrication attempts. |
| Performance/large inputs | New accessors are constant-time and add no scan/allocation. Existing parser allocation and unbounded request sizes have no transport budget yet; future ingress must bound resource use. No new load benchmark claimed. |

## Test quality and remaining documentation findings

The new `every_action_class()` contains a nested `match action` with explicit arms for all five
variants and no wildcard. Rust checks exhaustiveness for the entire ActionClass type, not just
the five arguments currently passed. Adding a sixth variant while leaving this helper unchanged
therefore produces E0004 when tests compile, even if production mapping is updated. No real
variant was added during review. This repairs the stated silent-unchanged-helper problem.

It is not a generated exhaustive iterator: a future author could add a match arm but forget the
separate array, which remains `[ActionClass; 5]`. Both lists still require review when edited.
The regression also still calls the private verifier; this review's 25-combination test uses
the public admission path. These are coverage limitations, not a currently reachable bypass.

The documentation repair is PARTIAL:

- PRODUCT.md now names the existing offline test and correctly describes target/requested_action.
  S03's current AC/Safety rows use the correct test and semantic action field.
- S02's AC2 still says "requested authority" and its Safety table still describes requesting
  Autopilot; the actual test now requests Delete above a Quarantine ceiling.
- S02's Outcome says F1 is "unrepaired as of this packet" while its new round-2 section says
  repaired. Both packets still say no independent verifier yet at the top. Historical logs/test
  names may remain as history, but must not read as current verification evidence.
- S02's ADR-0032 example and the module-level example still use
  `minimum_authority_for(verified.requested_action)` rather than the accessor call.
- S03's AC table header is separated from its rows by the inserted explanatory paragraph,
  breaking the Markdown table. Its AC2/residual prose still refers to CR1 despite the CR4 header.
- The fresh-producer claims should explicitly allow provenance-preserving Clone. TARGET.md's
  persistence wording "if a future caller needs that" must not imply durable replay is optional
  for real remote execution.

These surviving documentation/test-maintenance observations are accepted non-blocking residuals
for this round, consistent with round 2's disposition, not additional story FAILs. They must be
carried into follow-up backlog work by the coordinating executor/owner; this review deliberately
does not edit the project control plane or assign an unregistered story id.

## CR4 Safety Verdicts

Change/commit: reviewed scope at `4dcab698d2f5ea58a31c88efbfc711d3dc207d72`.
Independent verifier: Codex. Date: 2026-09-19. Risk: CR4 for both stories.

### E18-S02 — PASS_WITH_RESIDUALS

Safety surface: authentication/authorization of remote semantic intent, signed target binding,
replay admission, and immutable checked provenance. A CR4 Safety Verdict is now supportable
for the unwired library primitive, not for deployed remote mutation or complete replay prevention.

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-031 / C-01 / TM-17 | Target authenticates intent and retains local authority | Signed-field/target/ceiling probes; opaque result; local safety-floor tests; no production transport | PASS within library scope; integration residuals |
| C-03 / C-05 | Ambiguity cannot escalate authority | Malformed wire refusal, all ceilings, no raw wire AuthorityLevel, immutable checked action | PASS |
| C-07 / SI-019 | No alternate mutation path | No mutation in this module; mutation-boundary gate passes | PASS |
| C-18 | Claims match evidence | Current runtime fix established independently; remaining prose discrepancies explicitly recorded above | PASS_WITH_RESIDUALS |

### E18-S03 — PASS_WITH_RESIDUALS

Safety surface: the open protocol shared by commercial/self-hosted coordinators and independence
of local workflows. A CR4 Safety Verdict is supportable for that current library/product boundary.
AC1/AC3 are supported by unchanged local callers, no new network/config requirement, real empty-
policy refusal, local Govern independence, and the workspace suite. AC2/SI-031 inherit the repaired
opaque verified-result boundary and the action/ceiling checks. There is no privileged producer
branch. No claim is made about the availability of a future network transport or end-to-end
remote execution, neither of which exists here.

### Residuals and owner disposition (both stories)

1. **Accepted open replay residual:** replay state is per retained log, not durable/global.
   Restart, fresh logs, and pre-admission clones reset/fork the floor. Future transport work must
   supply shared durable acceptance and crash-safe ordering with persisted audit and execution,
   including recovery/idempotency. This is mandatory before real remote execution.
2. **Deferred integration:** no transport, persisted audit, execution consumer, operator config
   surface, or remote plan builder exists. Trusted clock, current local policy/target identity,
   execution-time plan revalidation, confidentiality/availability and ingress resource limits
   remain future caller obligations. Reusing a stored/cloned intent cannot substitute for them.
3. **Documentation and test-maintenance residuals:** the concrete items above remain; no current
   unchecked construction route was demonstrated. S03's no-network dependency claim is inspected,
   not a permanent new enforcement gate.
Compatibility is no longer an outstanding CI residual: the final exact-HEAD matrix passed
(see below). Local macOS success alone was not used to certify that matrix.

Disposition: retain these as accepted residual risk for this owner-authorized last round and
carry surviving work into new backlog items under ADR-0025. The coordinator must assign actual
story IDs and record owner acceptance/status/release evidence; no such transition is made here.
The prompt authorizes the cost-ceiling round and continuation of the durable replay residual;
it does not constitute blanket authorization to enable production remote execution.

Compatibility: this repair makes previously public result fields private (intentional Rust API
break fixing F1), without changing the wire format since round 2. ADR-0032's earlier wire break
remains intentional and there is no deployed producer. Rollback/recovery: keep the remote
surface unwired; disable/remove the additive remote module if its assumptions fail. Do not
restore public forgeable fields as a safety rollback. Local authority callers do not depend on
this module. No durable remote state exists to migrate, and production crash recovery is not
claimed tested.

## Gates actually run

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS, exit 0. |
| `python3 scripts/project_os.py review` | PASS, exit 0; scoped stories ready for review. |
| `python3 scripts/project_os.py status` and `next` | PASS, read-only. |
| `python3 scripts/check_agent_toolchain.py report` and `check` | PASS; no overdue decision/unmanaged component reported. Unobservable always-on costs warned. No tooling installed/changed. |
| `git diff --check 2fe937d..HEAD -- rust/crates/cancellai-safety/ 'project/evidence/E18-*' docs/PRODUCT.md` | PASS. |
| `cargo fmt --check` | PASS, exit 0. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS, exit 0. |
| `cargo check --workspace --all-targets` | PASS, exit 0. |
| `cargo test --workspace` | PASS, exit 0, including checked-result compile-fail doctest. |
| `cargo deny check` | Initial exit 1 due to read-only advisory lock; approved-access rerun PASS, exit 0, advisories/bans/licenses/sources all ok. Configured unmatched-license/duplicate-version warnings remain. |
| `cargo test --offline --manifest-path /tmp/e18-round3-review/probe/Cargo.toml --lib` | PASS, nine tests. Probe-only unused-assignment warning in detached-target test; production clippy clean. |
| `cargo check --offline --manifest-path /tmp/e18-round3-review/probe/Cargo.toml --bin <name>` | Expected exit 101 for each of forge, mutate_controller_id, mutate_sequence, mutate_target, mutate_requested_action, private_api, deserialize, default; diagnostic codes above. |
| `python3 scripts/check_mutation_boundary.py check` | PASS, 87 Rust source files. |
| `python3 scripts/check_rust_workspace.py check` | PASS, 13 crates, acyclic, model/safety isolated. |
| `python3 scripts/check_schemas.py check` | PASS, four golden documents. |
| `python3 scripts/check_fixtures.py check` | PASS, 13 fixtures. |
| `python3 scripts/verifier_handoff.py check` | PASS before and after review-file creation; both brief checksums accepted. |

Required Rust commands ran locally from `rust/` on macOS. Full Python quality suite was not
rerun (no Python implementation changed); no full pre-commit pass is claimed. Cross-platform
clippy was not separately run: neither platform source nor workspace lint policy changed.
Miri is weekly, not a per-change gate. No Python counterpart to this remote protocol exists
for differential execution. G1/G2 support the scoped library verdict with disclosed residuals;
G3 has completed stable/MSRV and quality matrix evidence for this change; G4 production
crash/audit/transport integration remains
deferred, not passed. This is not a blanket G1–G4 production release approval.

### CI health and the separate Windows fix

Initial `gh run list --branch main --limit 5` could not connect; approved network access then
retrieved results. At the last pre-write check, exact-HEAD workflows tests (35444609035), CodeQL
(35444609122), and governance (35444608920) passed. Rust run 35444608904 was still in progress:
coverage, Ubuntu/macOS quality, and all six stable/MSRV platform check jobs passed; Windows
quality remained running. Thus whole-branch green was not yet established at that observation.

`git show eb651b9 -- rust/crates/cancellai-store/` confirms five `drop(verify)` additions before
synthetic-directory cleanup, plus comments, and no production store change. This spot-check is
consistent with closing open Windows handles; it is not an E13 re-review or a substitute for
Windows CI. No E13 status/verdict was changed.

Final post-write CI read: `gh run view 35444608904 --json status,conclusion,headSha,jobs`
returned completed/success for the exact reviewed HEAD. All ten jobs passed: coverage,
Ubuntu/macOS/Windows quality, and all six stable/MSRV platform checks. Together with the
three successful workflows above, the reviewed commit is now green. In particular, Windows
quality no longer reproduces the five cleanup failures reported in round 2.

## Documents actually opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md` (Verifier procedure and method/handoff sections)
- `docs/CONSTITUTION.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md` (TM-17)
- `docs/architecture/TARGET.md` (Remote execution boundary)
- `docs/PRODUCT.md` (Open-source and commercial boundary)
- `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`
- `docs/adrs/0031-remote-execution-requests-are-signed-intents-never-plans.md`
- `docs/adrs/0032-remote-execution-requests-carry-actionclass-not-authoritylevel.md`
- `docs/development/RELEASE_GATES.md`
- `project/templates/SAFETY_VERDICT.md`
- `.github/SECURITY.md`
- `.claude/skills/orient/SKILL.md`
- `.claude/skills/epic-verifier/SKILL.md`
- `.claude/skills/adversarial-cases/SKILL.md`
- `.claude/skills/risk-gate/SKILL.md`
- `project/epics/E18.json`
- `project/evidence/E18-VERIFIER-REVIEW.md` (full)
- `project/evidence/E18-VERIFIER-REVIEW-ROUND2.md` (full)
- `project/evidence/E18-S02/EVIDENCE.md` (full)
- `project/evidence/E18-S03/EVIDENCE.md` (full)
- `project/evidence/E18-S02/VERIFIER_BRIEF.md` (full)
- `project/evidence/E18-S03/VERIFIER_BRIEF.md` (full)

Source inspected includes the full remote_execution module/diff, safety re-exports,
minimum_authority_for, MachineId, CLI/TUI manifests, and the separate store cleanup diff.
Temporary probes/logs remain under `/tmp/e18-round3-review/` and `/tmp/e18-r3-*`.

## Overall round verdict

**PASS_WITH_RESIDUALS. E18-S02 and E18-S03 both PASS_WITH_RESIDUALS; no story FAILs.**
F1's unchecked construction and post-verification mutation are closed. Prior checked-path
repairs still hold. Clone is provenance-preserving, not an unchecked constructor or one-use
execution guarantee. Surviving integration and documentation work is explicitly recorded above.

This was the owner-authorized ADR-0025 cost-ceiling round (round 3); there is no round 4.
Zero of two judged stories are rejected. This does not imply zero residual observations or
silently close the epic: surviving work goes to the coordinator's backlog and owner disposition.
Only this review file is written in the repository; no code, project status, generated file,
staging area, commit, or remote branch is changed.
