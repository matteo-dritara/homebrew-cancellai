# Safety Verdict - E17-S08

- Change: accepted MSRV 1.88.0/ratatui upgrade, six lint rewrites, plus owner-authorized
  strict-signature and transitive-advisory repairs found during independent review.
- Risk: CR4.
- Commit/PR: original `05defd4`, `31206c2`, `4c2801b`; repairs `28c29a7`, `5c36b3b`,
  `003d00e`, `ba30b42`; final source reviewed at `ba30b42`.
- Independent verifier: Codex (OpenAI); original executor: Claude.
- Date: 2026-09-13.

## Verdict

PASS_WITH_RESIDUALS

## Safety surface changed

Manifest root validation, manifest matching, WSL mount decoding, Windows process-name
observation, signature hex decoding, and build/dependency verification. Independent review
also found the edited bundle module violated ADR-0024 by using permissive signature
verification; AC6 records the authorized repair to strict verification. This intentional
acceptance narrowing is distinct from the six behavior-neutral lint changes.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-017 | Platform observations preserve native semantics and malformed mount paths are not silently rewritten | 512 octal cases plus overflow/signed/truncated/Unicode escapes; mutations of advance/continue/digit guard caught. Exact Windows function tested with synthetic enumeration/failure/dedup; native and both cross-target clippy pass. | PASS with native-runtime limits below |
| SI-021 | Untrusted manifest data cannot expand roots or assign authority | Optional root probes and existing escaping-root/empty-env/unknown-layout tests; guard-deletion mutants caught; no trust or capability elevation path introduced. | PASS |
| SI-022 | Invalid signature/provenance does not authenticate knowledge | New malformed-signature/hex corpus and restored ADR-0024 verify_strict; independently reproduced weak-key input rejected, valid fixtures accepted, permissive-call reversion mutant caught. | PASS |
| SI-029 | Tamper/expiry/replay/refused replacement preserves last accepted state | Weak-key apply refusal compares complete current state; existing tamper/replay tests plus E16's expiry/refusal matrix pass. | PASS |

## Adversarial cases

- Deleted/inverted hex parity; odd/non-ASCII signatures; every byte in lower/upper hex.
- Deleted root validation and glob guards; None/nonempty/empty/Unicode-whitespace env names;
  safe, empty, absolute and traversing subdirs; existing symlink-descent fixtures.
- Deleted octal-digit guard, shortened advance, missing continue, overflow and signed digits.
- Windows unknown enumeration, unmatched names and duplicate case-insensitive matches, using
  the actual extracted function with synthetic OS results. Host cargo tests alone miss its mutant.
- Weak local-policy key/signature: pre-existing permissive call authenticated it; strict repair
  rejects it without altering current. Reverting that call causes the regression to fail.
- Old dependency graph with default advisory scope passed; explicitly including transitive
  unsoundness fails on RUSTSEC-2026-0002. New graph passes the stronger policy with no ignore.

## Differential / compatibility evidence

All six original rewrites preserve guard order and fallthrough; the parity predicate is
mathematically equal for usize and the literal divisor 2 cannot be zero. A 65,538-value native
probe agrees. Full Python/Rust differential and repository gates pass. Kernel dependency
graphs (normal/build, all targets) are unchanged, and every selected new dependency reaches
workspace consumers only through TUI. Exactly one crossterm is selected on each target.

Eighty TestBackend character-buffer cases match before/after ratatui; the four populated
screens were visually inspected. All eight private border signatures consume static values.
Final local source ba30b42: 483 Python tests plus 444 subtests, 598 Rust tests, all requested
checkers and three-target clippy, and cargo-deny with ignore=[]/unsound=all. Actual commands,
CI job identities and evidence are in [the independent review](../E17-S08-VERIFIER-REVIEW.md).
Minimum-toolchain proof comes from native CI at 1.88.0, never from the local 1.94 compiler.

## Known residual risks

- No native Windows mutation-test campaign or new live Windows/Linux terminal session was run
  locally. The exact-function probe and cross-target clippy have narrower claims; actual
  tier-1 CI supplies native compilation/tests.
- TUI comparison/visual inspection checks character layout, not colors, escape transport or
  live raw-mode restoration. No upgrade-specific performance benchmark was measured.
- The verifier authored the requested repairs and their regressions. The original review is
  independent of Claude; the repairs do not claim a second independent reviewer.
- hashbrown/syn duplicate versions remain warnings under existing policy; they do not duplicate
  terminal-state ownership. Advisory scans cannot exclude undisclosed vulnerabilities.
- E17-S11 tracks the unrelated toolchain-report false positive. Its enforcing check already
  recognizes the approved pack members correctly.

No unresolved HIGH/CRITICAL safety finding remains in the reviewed scope. These are explicit
coverage/process limits, not acceptance of the reproduced weak-key or advisory-gate defects;
both are repaired and mutation/reversion-sensitive.

## Rollback / recovery

No persisted data migration is introduced. Bundle verification/apply refuses before replacing
current; ordinary rollback retains its expiry guard. A future dependency rollback must retain
strict signature verification and transitive advisory coverage, and choose a compatible patched
graph rather than silently returning to vulnerable lru or the paste waiver. Restore an older
minimum only by a new owner decision and tier-1 verification. No release is cut here.

## Owner decision

The owner explicitly authorized repairs and instructed moving a PASS/PASS_WITH_RESIDUALS
story to done after its independent record and Safety Verdict exist. This verdict supports
that conditional, story-scoped disposition. Separate owner acknowledgment of these published
residuals is not invented; no epic closure, release approval or unresolved-high-risk waiver
is granted by the verifier.
