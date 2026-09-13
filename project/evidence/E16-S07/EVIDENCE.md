# Evidence Packet - E16-S07

- Original executor: Claude; implementation `9f739d6`, clarification `bbc0f9e`.
- Independent verifier: Codex, 2026-09-13; test-only follow-up `69ab771`.
- Change Risk: CR4; contract: `project/epics/E16.json`.
- Final validated source: `ba30b42`, including the separately recorded E17-S08 security repair.

## Outcome

VERIFIED - PASS. Story done; E16 remains blocked and no release is cut.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - identical predicates | Exact pre/post comparison: None returns false; Option<u64> is Copy; current.as_ref() preserves the shared borrow; publisher equality still precedes sequence comparison. | PASS |
| AC2 - exact expiry rejects | Verification and rollback >= to > mutants both fail their own exact-boundary test. A one-second-early rollback mutant fails the before-boundary test. | PASS |
| AC3 - no expiry never expires | Thirty signed cases cross None/zero/one/1500/u64::MAX expiry with six clocks. None verifies even at u64::MAX; existing no-expiry rollback succeeds. | PASS |
| AC4 - publisher-local staleness | Another publisher's sequence 1 replaces sequence 100; deleting the publisher guard fails that test. Swapping the two pure comparisons survives as an equivalent mutation. | PASS |
| AC5 - operative promised toolchain | ADR-0026 superseded the historical 1.85.0 promise with 1.88.0. Rust CI run 34774901899 at repaired source ba30b42 passes all nine jobs, including native macOS/Linux/Windows MSRV checks. No local 1.88 execution is claimed. | PASS |

## Safety Evidence

| Invariant | Counterexample | Evidence | Result |
| --- | --- | --- | --- |
| SI-029 | Refused rollback destroys accepted state | Independent empty/current-only/expired/repeated rollback cases compare complete current values after every refusal; earlier-clock retry proves previous was retained. | PASS |
| SI-029 | Expiry boundary weakens or another publisher becomes stale | Targeted mutations fail the appropriate tests; same-publisher equal/lower replay still refuses. | PASS |

## Verification Commands

The verifier independently ran every command in AGENTS.md's Current Python checks and all seven
requested Rust commands after the last repair. Python used the already installed repository
virtual environment; local Rust was 1.94.0. Pytest: 483 tests and 444 subtests; Rust: 598 tests,
two existing ignored scheduled benchmarks. Formatting, lint/type checks, all repository
checkers and both cross-target clippy commands passed. Cargo-deny passed all four checks after
permission to lock its advisory database, with ignore=[] and unsound=all.

Full commands, failures encountered before repair, mutation results and CI identities are in
[the independent review](../E16-S07-VERIFIER-REVIEW.md). The new
`verifier_expiry_matrix_and_refused_rollback_preserve_all_current_fields` test was authored by
the verifier and passed before any change to these three production predicates.

## Compatibility and operability

The reason for the original rewrite was historical 1.85 compatibility; the accepted current
minimum is 1.88.0. Keep the correct is_some_and code and boundary tests. No API, wire format,
filesystem operation or persistent migration changed in E16-S07. Predicate cost is unchanged;
no new performance measurement is claimed.

## Residual risks

- Local 1.88 execution is unavailable; actual tier-1 CI is the evidence, not local stable.
- The broad review found a pre-existing strict-signature mismatch and repaired it explicitly
  under E17-S08. That deliberate security narrowing is not part of E16-S07's equivalence claim.
- This scope does not certify the complete future knowledge distribution/revocation service.
- E17-S11 tracks the unrelated report-only toolchain false positive; approvals already exist
  and the enforcing toolchain check recognizes them correctly.

## Safety Verdict

Independent [Safety Verdict](SAFETY_VERDICT.md): PASS.

## Verifier verdict

PASS
