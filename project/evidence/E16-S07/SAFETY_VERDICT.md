# Safety Verdict - E16-S07

- Change: preserve the bundle verifier/store's three optional predicates while removing the
  historical let-chain toolchain incompatibility; add boundary/refusal regressions.
- Risk: CR4.
- Commit/PR: `9f739d6`, clarification `bbc0f9e`, verifier regression `69ab771`; validated
  with the repaired repository source at `ba30b42`.
- Independent verifier: Codex (OpenAI); original executor: Claude.
- Date: 2026-09-13.

## Verdict

PASS

## Safety surface changed

Expiry acceptance and the same-publisher sequence guard in signed knowledge verification,
apply and rollback. No filesystem mutation, trust promotion, public API or persistence format
is added. The separate E17-S08 strict-signature repair is disclosed in its own verdict and is
not attributed to E16-S07's behavior-preserving predicates.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-029 | Expired/invalid/replayed knowledge fails closed, and refused rollback preserves offline current state | Both expiry comparisons remain >=; 30 signed time/expiry cases; complete current-value comparisons after empty/no-prior/expired/repeated rollback refusals; same/different publisher regressions and targeted mutants. | PASS |

## Adversarial cases

- None expiry and zero/one/max expiry; clock before, equal, after and u64::MAX.
- Empty/current-only/no-prior-after-rollback and expired-prior refusals, with complete current
  equality after every refusal and successful earlier-clock retry proving prior retention.
- Same-publisher equal/lower sequences versus another publisher's lower sequence.
- Verification and rollback >= to > mutations both caught independently; one-second-early
  rollback caught by the before-expiry test; dropping publisher equality caught.
- Swapping the two pure comparisons survives as an equivalent mutation, not a test defect.

## Differential / compatibility evidence

Exact source comparison proves the optional/borrow/short-circuit rewrites preserve behavior.
The workspace differential gates pass. No 1.88 toolchain is installed locally; only native
CI 1.88.0 jobs provide minimum-toolchain evidence. Full commands, CI job identities and
mutation outcomes are in [the independent review](../E16-S07-VERIFIER-REVIEW.md).
Final local gates at ba30b42: 483 Python tests plus 444 subtests; 598 Rust tests; all requested
checkers and native/Windows/Linux clippy; cargo-deny with empty ignore list and unsound=all.

## Known residual risks

The scope is these predicates, not a proof of the whole future knowledge service. Local
MSRV execution is unavailable and is replaced by actual tier-1 CI evidence. The report-only
toolchain false positive is deferred as E17-S11; its enforcing check is correct. No unresolved
safety defect in E16-S07's changed predicates remains. Independent verification is model-family
separation from the original executor, not organizational certification.

## Rollback / recovery

A refused rollback keeps current and previous intact; successful rollback consumes previous
only after expiry passes. The rewrite need not be reverted after ADR-0026: it is correct on
1.88.0 as well. If regression is discovered, revert the specific offending predicate change
under CR4 review while retaining boundary tests. No persistent migration or data recovery is
needed for this code-only change.

## Owner decision

The owner explicitly instructed closure of PASS/PASS_WITH_RESIDUALS stories after publication
of independent verdicts. This record supplies the requested PASS and supports that authorized
story-state transition. Separate owner acknowledgment of the published verdict is not recorded
by the verifier; no safety waiver, epic closure or release approval is inferred.
