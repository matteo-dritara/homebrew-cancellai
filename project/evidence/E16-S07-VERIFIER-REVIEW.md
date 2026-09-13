# E16-S07 independent verifier review

- Verifier: Codex (OpenAI), distinct from the original executor, Claude.
- Date: 2026-09-13.
- Review target: implementation `9f739d6^..9f739d6` and contract clarification
  `bbc0f9e^..bbc0f9e`, inspected as committed at `4c2801b`. Interleaved commits in the wider
  range are not attributed to this story.
- Final regression target: `ba30b42`; E16-S07's test-only follow-up is `69ab771`.
- Scope: one standalone CR4 story review, explicitly authorized by the owner. E16 remains
  blocked; this is not epic closure, release approval, or reliance on an executor review waiver.

## Story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E16-S07 | PASS | The three predicates preserve acceptance, expiry and publisher scope; independent boundary/refusal tests pass and non-equivalent mutants are caught. Current promised-toolchain CI is recorded below. |

## Per-AC assessment

| AC | Verdict | Concrete evidence |
| --- | --- | --- |
| AC1 - identical accept/reject conditions | PASS | Compared the exact pre/post source at all three sites. `Option<u64>` is Copy; None returns false without evaluating a predicate. `self.current.as_ref()` passes a shared reference, neither consuming nor replacing current. Publisher equality remains the left operand of `&&`; sequence comparison is evaluated only for that publisher. No guard contains side effects. |
| AC2 - exact expiry refuses verification and rollback | PASS | Both production comparisons remain `now_unix >= expires_at`. Independent matrix spans zero, one, 1499/1500/1501 and `u64::MAX`; both original exact-boundary tests fail when their own comparison changes to `>`. A one-second-early rollback mutant fails `rollback_just_before_the_expiry_boundary_succeeds`. |
| AC3 - absent expiry never expires | PASS | Signed bundles with None verify at all six clocks in the matrix, including `u64::MAX`. Existing successful rollback reinstates a bundle with None. No sentinel timestamp or overflow arithmetic is introduced. |
| AC4 - another publisher is not stale on sequence alone | PASS | Applying sequence 1 from another locally known publisher over sequence 100 succeeds. Dropping publisher equality fails the existing different-publisher regression. Same-publisher lower/equal sequence still refuses and preserves current. |
| AC5 - promised minimum on all tier-1 platforms | PASS | The operative promise is 1.88.0 under accepted ADR-0026, not historical 1.85.0. Original-target Rust CI has successful macOS/Linux/Windows 1.88.0 check jobs; repaired-source CI is recorded below. Local checks used 1.94.0, not 1.88.0. |

## Counterexamples actually tried

- Thirty signed verification cases: expiry in `{None, 0, 1, 1500, u64::MAX}` crossed with
  clocks `{0, 1, 1499, 1500, 1501, u64::MAX}`. Expiry is exclusive at equality; all matched.
- Empty-store rollback, no-prior rollback with a current bundle, no-prior after successful
  rollback, and expired-prior rollback at equality/after/max. Every refusal compares the
  complete `current()` value, not just its payload, with a pre-call clone. All matched.
- Repeated expired rollback refusals followed by a supplied-clock retry at 1499 succeeded,
  proving the prior slot was not consumed on refusal. This tests API clock inputs; it does
  not claim wall clocks normally move backwards.
- Existing same-publisher equal/lower replay and different-publisher lower-sequence cases
  ran against both original and mutated source. Schema, signer, digest, signature and expiry
  failures remain before the state replacement in apply.
- Shared-reference lifetime/order inspection found no move/drop change: both `take()` calls
  remain after all refusal paths. `rollback` borrows previous before checking expiry.
- The broad review also found the pre-existing permissive signature call did not satisfy
  ADR-0024. That was reproduced and repaired explicitly under E17-S08 (`28c29a7`), with its
  own added AC and evidence. It is not disguised as an E16 predicate-equivalence change.

### Mutation campaign

Each mutation was applied alone in a disposable archive of `4c2801b`, then restored. Commands
were `cargo test --locked --offline -p cancellai-safety`; the last row used the named test
filter. No compile failure is classified as a caught behavior mutation.

| Mutation | Original suite | After added regression | Meaning |
| --- | --- | --- | --- |
| verify_bundle expiry `>=` to `>` | Caught by `expiry_a_bundle_past_its_expires_at_is_rejected` | Also caught by verifier matrix | Exact verification boundary is pinned. |
| rollback expiry `>=` to `>` | Caught by `rollback_at_exactly_the_expiry_boundary_refuses` | Also caught by verifier matrix | The newly added executor test is effective. |
| Swap publisher and sequence comparisons inside the closure | Survives | Survives | Equivalent mutation: both comparisons are total and side-effect-free. Boolean conjunction is commutative here; swapping is not dropping the publisher condition. No test gap is inferred. |
| Drop publisher equality from the staleness closure | Caught by `a_first_bundle_from_a_different_publisher_is_never_stale` | Caught | Different publishers remain independent. |
| Rollback compares against `expires_at.saturating_sub(1)` | Caught by `rollback_just_before_the_expiry_boundary_succeeds` | Caught | The before-boundary executor test also detects an actual changed boundary. |

Machine-readable results: [verifier-mutations.json](E16-S07/verifier-mutations.json).

### Applicability of the eleven axes

| Axis | Scope and concrete work |
| --- | --- |
| Path/identity | No path operation in these predicates; inspected that state replacement remains after checks. Platform-dependent work is in E17 review. |
| Partial reads/permissions | No I/O in this module's changed paths; invalid bundle/signer paths exercised by the safety suite. |
| Links/mounts/reparse | No filesystem traversal in this change; no claim of new filesystem coverage. |
| Provider/version drift | Existing unknown-signer and unsupported-schema cases run; different publisher tested directly. |
| Concurrency | API requires exclusive `&mut self` for transitions; no concurrent mutation primitive was added. Shared borrow verified before take. |
| Failure/retry | Every rollback refusal preserves complete current state; repeated refusal and retry exercised. No persistent/crash boundary exists here. |
| Boundaries | Thirty-case matrix plus independent exact/before expiry mutations. |
| Policy/trust | Different-publisher case plus known/unknown signer and local tier tests; strict-verification defect repaired separately. |
| Platform | Pure predicates plus native tier-1 CI; no new platform-specific logic. |
| Malformed input | Existing digest/signature/schema failures plus E17's new malformed-signature corpus. |
| Large datasets | Conditions remain constant-count comparisons; no traversal/allocation added by the rewrite. Workspace performance guards ran; no new stress benchmark claimed. |

## Recommendation on retaining the rewrite

Keep `is_some_and`. It is correct at the current minimum, makes the optional predicate clear,
and changes neither ownership nor authority. The historical toolchain reason is no longer a
constraint, but provides no reason to revert correct kernel code. Retain both executor boundary
tests and the verifier's stronger state-preservation matrix.

## CI evidence

At original target `4c2801b`, [Rust run 34753145038](https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/34753145038)
passed all nine jobs: stable quality on macOS/Linux/Windows and workspace check on each at
stable and 1.88.0. The three MSRV jobs are 103712952427 (macOS), 103712952476 (Linux), and
103712952373 (Windows). Tests, governance and CodeQL also passed at that target.

Repaired source `ba30b42`: [Rust run 34774901899](https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/34774901899)
passed all nine jobs, including 1.88.0 checks on macOS (103771115709), Linux (103771115708),
and Windows (103771115698), plus stable checks and each native quality job. Job steps were
inspected, including the real workspace check, tests and cargo-deny commands. This is evidence
about the repaired source revision, not just the executor's original commit.

## Documents opened

Read contracts and relevant sections before judging code; this list records actual access,
not a claim that every historical paragraph remains current.

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E16.json`
- `project/epics/E17.json`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/security/SUPPLY_CHAIN.md`
- `docs/architecture/PROVIDER_MODEL.md`
- `docs/architecture/TARGET.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/development/AGENT_TOOLCHAIN.md`
- `docs/adrs/0015-rust-workspace-toolchain-and-repository-layout.md`
- `docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md`
- `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`
- `docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md`
- `project/evidence/E16-S07/EVIDENCE.md`
- `project/templates/SAFETY_VERDICT.md`
- `.claude/skills/orient/SKILL.md`
- `.claude/skills/toolchain/SKILL.md`
- `.claude/skills/epic-verifier/SKILL.md`
- `.claude/skills/adversarial-cases/SKILL.md`
- `.claude/skills/risk-gate/SKILL.md`

## Residual risks and disposition

No unresolved defect in the three predicates was found. Local MSRV execution is unavailable;
CI is the actual minimum-toolchain evidence. This verdict covers the stated story, not the
whole knowledge service or its future rollback/revocation policy. The unrelated report-only
agent-toolchain defect is filed as E17-S11; the enforcing check recognizes approved members.
The independent assessment and the verifier-authored test addition are disclosed separately.
A single owner-requested story-scoped round concludes here after repairs and reruns; neither
epic's other stories nor its release state is closed by this record.
## Gates run independently

Full rerun after the last repair at `ba30b42`, using the existing `.venv` for Python and local
`rustc 1.94.0`. No toolchain component was installed. Commands below returned exit 0; Rust
commands ran from `rust/`, all others from the repository root.

| Command | Result |
| --- | --- |
| `python3 -m pytest tests -v` | PASS |
| `python3 -m ruff check .` | PASS |
| `python3 -m ruff format --check .` | PASS |
| `python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py scripts/check_mutation_boundary.py scripts/check_provider_compatibility.py scripts/check_provider_trust.py scripts/check_platforms.py scripts/rust_python_parity.py scripts/release_manifest.py scripts/check_repository_topology.py scripts/check_agent_skills.py scripts/process_metrics.py scripts/check_risk_classification.py scripts/check_agent_toolchain.py scripts/check_evidence.py scripts/check_ears.py scripts/safety_oracle.py scripts/gate_sensitivity.py` | PASS |
| `python3 scripts/gen_docs.py --check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_workflows.py check` | PASS |
| `python3 scripts/check_fixtures.py check` | PASS |
| `python3 scripts/check_schemas.py check` | PASS |
| `python3 scripts/characterize.py check` | PASS |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/check_provider_compatibility.py check` | PASS |
| `python3 scripts/check_provider_trust.py check` | PASS |
| `python3 scripts/check_platforms.py check` | PASS |
| `python3 scripts/rust_python_parity.py self-test` | PASS |
| `python3 scripts/rust_python_parity.py check` | PASS |
| `python3 scripts/check_process.py check` | PASS |
| `python3 scripts/release.py check` | PASS |
| `python3 scripts/release_manifest.py check` | PASS |
| `python3 scripts/check_repository_topology.py check` | PASS |
| `python3 scripts/check_agent_skills.py check` | PASS |
| `python3 scripts/process_metrics.py check` | PASS |
| `python3 scripts/check_risk_classification.py check` | PASS |
| `python3 scripts/check_agent_toolchain.py check` | PASS |
| `python3 scripts/check_evidence.py check` | PASS |
| `python3 scripts/safety_oracle.py check` | PASS |
| `python3 scripts/check_ears.py check` | PASS |
| `python3 scripts/gate_sensitivity.py check` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS |
| `cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings` | PASS |
| `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings` | PASS |

Pytest: 483 passed and 444 subtests. Rust: 598 passed, two existing scheduled benchmarks ignored.
System Python initially lacked ruff/mypy; the existing virtual environment passed both, including
mypy over all 25 named modules. Cargo-deny initially refused its sandboxed database lock; the
explicitly permitted rerun passed all four checks. An earlier final-gate attempt caught
`repeat(1)` in the verifier's test; `ba30b42` fixes it, and this entire table was rerun afterward.
Pre-commit hooks also ran on each repair commit. No skipped hook is counted as a full test run.
The final evidence/status-only changes are followed by regenerated-project, documentation,
process, evidence and release checks before commit.
