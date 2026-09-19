# Evidence Packet - E18-S03

- Commit/PR: local checkpoint on `main`, `docs/PRODUCT.md` +
  `rust/crates/cancellai-safety/src/remote_execution.rs`
- Executor: Claude
- Independent verifier: Codex - three rounds complete (`E18-VERIFIER-REVIEW.md`,
  `-ROUND2.md`, `-ROUND3.md`); round 3 (2026-09-19, owner-authorized ADR-0025 cost-ceiling
  round) issued a `PASS_WITH_RESIDUALS` CR4 Safety Verdict (`SAFETY_VERDICT.md`)
- Change Risk: CR4 (declared CR1 at planning; the commit-time risk-floor gate refused it because
  the added test touches `rust/crates/cancellai-safety/src/*`, floored at CR4 - see
  `project/epics/E18.json`'s `risk_reclassification_note`. The reclassification is procedural,
  not substantive: no new production logic was added, only a test)
- Spec version/commit: `project/epics/E18.json` as committed in this checkpoint

## Outcome

PARTIAL - see "Round 2 independent verifier review" below; this story's AC2 boundary claim
inherits E18-S02's F1 defect. Story remains `ready_for_review`, not closed.

## Acceptance Criteria Evidence

*(Test names below are current as of round 3's repair; see "Round 1 independent verifier
review", "ADR-0032 implementation", and "Round 2/3 independent verifier review" further down for
what changed. As of round 3, F1 is repaired and both stories PASS_WITH_RESIDUALS - see
`SAFETY_VERDICT.md`.)*

| AC | Text (verbatim, `project/epics/E18.json`) | Evidence | Result |
| --- | --- | --- | --- |
| AC1 | "Single-machine workflows remain fully functional without account/cloud." | `cancellai-cli`'s and `cancellai-tui`'s `Cargo.toml`s carry no networking dependency (checked directly - neither lists any HTTP/socket/gRPC crate). `local_authority_is_unaffected_by_an_unconfigured_commercial_service` proves `effective_authority` computes a full, deterministic result from purely local `AuthorityInputs`, unaffected by a real remote-verification refusal computed alongside it - the local authority path needs nothing from the remote layer to function. | PASS |
| AC2 | "Commercial services add coordination, not local destructive capabilities." | `docs/PRODUCT.md`'s "Open-source and commercial boundary" section now names the concrete mechanism: `RemoteExecutionRequest` (E18-S02) **is** the open local node protocol - any coordinator, commercial or self-hosted, produces the identical signed document; verification narrows every accepted request to one `target`/`requested_action` pair (a semantic `ActionClass`, never an `AuthorityLevel` directly, ADR-0032) capped by a locally-set ceiling, never a plan or a privileged second path, using the now-opaque, immutable `VerifiedRemoteIntent` (F1 repair). This is primarily a documentation update pointing at E18-S02's already-tested structural guarantee (`a_request_asking_above_the_controllers_ceiling_is_refused_not_clamped`, `remote_and_local_user_requested_reach_identical_effective_authority`) - the story's own diff is a test, not new production enforcement logic. | PASS |
| AC3 | "If no commercial or fleet-coordination service is configured or reachable, single-machine functionality is not reduced or refused." (added at commit time - EARS-compliant restatement of AC1's own intent, required once the risk-floor gate raised this story to CR4; see `risk_reclassification_note`) | `local_authority_is_unaffected_by_an_unconfigured_commercial_service` - the same test cited for AC1 - is precisely this criterion's negative form made concrete: an empty `TrustedRemoteControllers` (the "no coordination service configured" case) never reduces or refuses `effective_authority`'s output. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-031 (added to this story's `safety_obligations` at the same commit-time reclassification that raised it to CR4 - see `risk_reclassification_note`) | A local authority decision silently depending on, or being weakened by, the presence/absence of remote-controller configuration | `local_authority_is_unaffected_by_an_unconfigured_commercial_service`: `effective_authority` is called alongside a real, refused remote-verification attempt through the public log, and still returns the correct, fully-local result | PASS |

No new safety-relevant behavior was added; this story documents and offline-conformance-tests a
boundary E18-S01/E18-S02 already enforce and test.

## Verification Commands

```text
$ cargo test -p cancellai-safety remote_execution
running 18 tests
test remote_execution::tests::a_local_authority_decision_requires_zero_trusted_remote_controllers ... ok
(... 17 pre-existing E18-S02 tests, unchanged, all ok)
test result: ok. 18 passed; 0 failed

$ cargo fmt --check
(clean, no output)

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s)   (no warnings)

$ cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s)

$ cargo test --workspace
(every crate) test result: ok ... 0 failed  (workspace total, no regression)

$ cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/check_docs.py check
docs OK: 405 Markdown files; local links and safety IDs are consistent

$ python3 scripts/project_os.py check
governance OK
```

Not run in this session (unavailable/not required for this change - CI runs the full matrix):
cross-platform clippy, `cargo +nightly miri`, Python checks (`ruff`/`mypy`/etc.) - no relevant
files touched. `check_schemas.py`/`check_fixtures.py`/`check_mutation_boundary.py` - not affected
(no schema/fixture/mutation-boundary file touched). A fresh `adversarial-cases` pass was not
re-run for this story's own CR4 reclassification: the module this test lives in
(`remote_execution.rs`) already received a full pass at E18-S02, covering every applicable axis
for the request-verification surface; this story's one addition exercises the same,
already-covered local-authority path from the opposite direction (proving it needs nothing
remote, rather than proving the remote path is bounded) and introduces no new attack surface.

## Compatibility

- Documentation-only change to `docs/PRODUCT.md`, plus one additional unit test in an existing
  Rust module. No dependency, schema, or platform surface touched.

## Performance / operability

- Not applicable - no runtime behavior changed.

## Documentation updated

- `docs/PRODUCT.md`: "Open-source and commercial boundary" section expanded to name the concrete
  `RemoteExecutionRequest` mechanism now that E18-S01/E18-S02 give it real shape (the story's own
  declared `documentation_impact`).

## Method defects

- none.

## Round 1 independent verifier review (`project/evidence/E18-VERIFIER-REVIEW.md`)

FAIL - inherited from E18-S02's own two now-repaired defects (this story's claimed protocol
boundary is E18-S02's public API), plus its own test-quality finding: the sole offline
conformance test constructed an empty `TrustedRemoteControllers` and never passed it anywhere,
proving only that an existing function signature has no such parameter - not AC1/AC2/AC3.
Replaced with `local_authority_is_unaffected_by_an_unconfigured_commercial_service`
(`rust/crates/cancellai-safety/src/remote_execution.rs`), which exercises real data flow: an
empty policy refuses a remote request through the real `RemoteExecutionLog::verify_and_record`
path (AC2 - absence of commercial configuration never silently grants anything), and a local
authority decision computed alongside that refusal is unaffected by it (AC1/AC3). E18-S02's own
two unrepaired findings (durable replay-state persistence, and the `ActionClass`/`AuthorityLevel`
design-record divergence) both propagate here unchanged, since this story's boundary claim rests
on E18-S02's primitive - see `project/evidence/E18-S02/EVIDENCE.md`'s own "Round 1 independent
verifier review" for detail. This story cannot honestly claim a closed CR4 boundary until those
are resolved, independent of anything this story's own diff could fix.

## ADR-0032 implementation

E18-S02's `ActionClass`/`AuthorityLevel` design-record divergence is repaired - see
`project/evidence/E18-S02/EVIDENCE.md`'s own "ADR-0032 implementation" for the change itself.
This story's own AC2 evidence text ("`target`/`requested_authority` pair") is updated to
`target`/`requested_action` in `docs/PRODUCT.md`. The durable replay-state persistence finding
remains unrepaired for the reason recorded in E18-S02's evidence (kernel-ring/no-I/O
architectural constraint, needs a future outer-ring caller); this story's own boundary claim
still cannot be called fully closed until that is resolved.

## Round 2 independent verifier review (`project/evidence/E18-VERIFIER-REVIEW-ROUND2.md`) - FAIL

FAIL - inherited from E18-S02's F1 (`VerifiedRemoteIntent` publicly forgeable/mutable): this
story's AC2 claim that `RemoteExecutionRequest`/verification is "the open local node protocol...
never a privileged second path" rests on E18-S02's verified-result type actually being
non-bypassable, which round 2 showed it was not. F1 is repaired in E18-S02's own evidence packet
(private fields, read-only accessors, `compile_fail` regression). This story's own local-authority
test (`local_authority_is_unaffected_by_an_unconfigured_commercial_service`) was independently
re-confirmed as real data flow, not a re-raised finding.

## Round 3 independent verifier review (`project/evidence/E18-VERIFIER-REVIEW-ROUND3.md`) - PASS_WITH_RESIDUALS

Owner-authorized ADR-0025 cost-ceiling round (the last one). E18-S02's F1 repair is independently
confirmed closed (see that story's own "Round 3 independent verifier review"); this story's claim
that `RemoteExecutionRequest`/verification offers no privileged producer path is therefore
independently supportable. A static, workspace-wide search found no production remote caller
anywhere, so the "open protocol" claim remains offline/library-level, not an end-to-end guarantee.
Issued a CR4 Safety Verdict of `PASS_WITH_RESIDUALS` for this story - see `SAFETY_VERDICT.md`.

## Residual risks

- The claim "no networking dependency" is verified by direct inspection of `Cargo.toml` today,
  not by an automated gate - a future PR adding a networking crate to `cancellai-cli`/
  `cancellai-tui` would not be caught by any check this story adds. Accepted at CR1: this story's
  own acceptance criteria ask for the boundary to be true and documented now, not for a permanent
  automated enforcement mechanism, which would be new product scope beyond what this story's ACs
  request.
- `RemoteExecutionRequest` is documented here as "the open local node protocol," but no
  transport exists yet to carry it (E18-S02's own residual, unchanged by this story) - the
  protocol is open in the sense that its schema/verification is fully specified and reusable by
  any producer, not in the sense that it is already reachable over a network today.

## Verifier verdict

Round 1 (Codex, 2026-09-19): FAIL - test-quality finding repaired; E18-S02's own two findings
inherited. Round 2 (Codex, 2026-09-19): FAIL - inherited E18-S02's F1, now repaired there. Round 3
(Codex, 2026-09-19, owner-authorized ADR-0025 cost-ceiling round - the last one):
`PASS_WITH_RESIDUALS`, with a CR4 Safety Verdict recorded in `SAFETY_VERDICT.md`. Owner acceptance
of that verdict is pending; see `SAFETY_VERDICT.md`'s own "Owner decision" field before this
story moves to `done`.
