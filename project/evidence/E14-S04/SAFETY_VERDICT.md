# Safety Verdict - E14-S04

Change: Structural anomaly detection - provider-layout drift bounds authority
Risk: CR4
Verifier: Codex
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This is an append-only round history (`docs/development/AGENT_PROTOCOL.md`): each round's record
stays when a later round repairs and passes it. The operative verdict is the most recent one by
position in this file (Round 8), not whether an earlier round ever failed - see
`scripts/project_os.py`'s `safety_verdict_passes` for the exact rule this file is read against.
Each round's full reproduction, adversarial cases, and gate results are recorded in its own
top-level file under `project/evidence/`, linked below; this file is the consolidated verdict
history the governance gate reads to decide whether E14-S04 may close.

## Round 1-2 (E14-VERIFIER-REVIEW.md, E14-VERIFIER-REVIEW-ROUND2.md)

`FAIL`, `FAIL`. A layout-drift recommendation reached no authority computation at all (round 1);
the round-1 repair was a second, opt-in function the still-public plain `effective_authority`
could bypass, and `LayoutDriftFinding`'s public fields let a caller fabricate a fake `Recognized`
result (round 2).

## Round 3 (E14-S04-VERIFIER-REVIEW-ROUND3.md)

`FAIL`. ADR-0034's mandatory `Option<AuthorityLevel>` ceiling field was a caller-asserted
*conclusion*, disconnected from the facts it summarized - a caller could compute the right
ceiling once, then separately construct an `AuthorityInputs` asserting `None` while holding the
real finding.

## Round 4 (E14-S04-VERIFIER-REVIEW-ROUND4.md)

`FAIL`. ADR-0035's raw-observation field (`AuthorityInputs::provider_layout:
ProviderLayoutAssessment`) closed round 3's exact reconstruction but not the general one: a
caller holding a real `Observed{Drifted}` value could still construct a second, sibling
`AuthorityInputs` asserting `NotAssessed`, because `AuthorityInputs` remained a plain, publicly
constructible struct. No fifth patch on the same field-level approach was authorized; the owner
directed a structurally different design (ADR-0036).

## Round 5, first pass (E14-S04-VERIFIER-REVIEW-ROUND5.md)

`FAIL`. ADR-0036 removed the layout fact from `AuthorityInputs` entirely, replacing it with
`cancellai_platform::BoundLayoutObservation` (real directory I/O) feeding an opaque
`ProviderExecutionPermit`. The first implementation's `BoundLayoutObservation::observe` still
took a public `identity_observer: &dyn IdentityObserver` parameter; a caller could pair one
directory's real markers with a fabricated identity (via the legitimately-public
`SyntheticIdentityObserver`) equal to a different, genuinely drifted root's real identity.

## Round 5, second pass (E14-S04-VERIFIER-REVIEW-ROUND5-PASS2.md)

`FAIL`. The observer-parameter closure was confirmed, but `observe` still read identity
(`symlink_metadata`) and markers (`read_dir`) as two separate path-based syscalls - a TOCTOU
race admitting a root swap between them - and did not refuse a symlinked root.

## Round 6 (E14-S04-VERIFIER-REVIEW-ROUND6.md)

`FAIL`. The TOCTOU/symlink repair (binding through `cancellai_sealedfs::SealedRoot::
bind_existing`, reading identity/markers from one held descriptor) was confirmed sound and the
new `unsafe` code's ownership/lifetime reasoning audited and passed. `list_child_names` treated
every `readdir` null result as end-of-directory, so a real mid-enumeration read failure returned
a truncated-but-`Ok` marker list; the `DT_UNKNOWN` `fstatat` fallback had the same fail-open
shape.

## Round 7 (E14-S04-VERIFIER-REVIEW-ROUND7.md)

`FAIL`, test-only. The POSIX errno-discrimination repair was confirmed correct in production,
but its regression test closed the descriptor before `list_child_names` ran at all, failing at
the earlier `try_clone` step and never reaching `readdir`'s own errno handling.

## Round 8 (E14-S04-VERIFIER-REVIEW-ROUND8.md) - operative verdict

## Verdict

`PASS_WITH_RESIDUALS`

## Safety surface changed

`cancellai-safety`'s provider-layout authority boundary (SI-004): whether a real, non-forgeable
observation of a provider root's structure can bound the authority level a
`ProviderExecutionPermit` carries, without being discardable, forgeable, or racy.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 | Layout drift/incomplete observation cannot preserve destructive capability | The `list_child_names_with_hook`-based regression reaches the repaired `readdir`/`errno` branch (confirmed via a negative control: temporarily bypassing the errno check made the test fail as required, then restored) | PASS |
| C-02 | Ambiguity never escalates privilege | A real mid-enumeration failure is `Err`, never a truncated `Ok` that could match a known signature | PASS |
| C-05 / TM-05 | Authority bounded by real, non-forgeable evidence | `BoundLayoutObservation::observe` binds identity and markers to one `SealedRoot`-held, `O_NOFOLLOW` descriptor; no public constructor, observer parameter, or conversion path exists to substitute either fact | PASS |

## Adversarial cases

- Root-rename-after-bind and final-symlink-refusal (round 6), re-confirmed.
- Public observer-injection and `EffectiveAuthority`-to-permit conversion (rounds 4-5),
  re-confirmed absent.
- `readdir` mid-enumeration failure via a private test-only hook, with a negative control proving
  the test depends on the repaired errno-check code (round 8).
- Hook visibility audit: `list_child_names_with_hook` is private; no public API accepts one or
  can otherwise interfere with enumeration (round 8).

## Differential / compatibility evidence

Not applicable - this is new Rust-side kernel primitive work with no Python reference
counterpart.

## Known residual risks

- `known_signatures` (what counts as a "recognized" layout) remains caller-supplied, not drawn
  from a trust-bounded provider manifest; no production call supplies a non-empty value today, so
  every current call resolves the layout constraint to `Observe`, never a silently invented
  "recognized" result. Closing this needs its own dependency-ring ADR (`cancellai-safety` may not
  depend on `cancellai-provider-api` without one, per ADR-0019).
- No mutation-boundary call site consumes a `ProviderExecutionPermit` yet; `cancellai_safety::
  mutation_executor::execute` is unchanged and continues to authorize on a plain
  `AuthorityLevel`. This is E14-S05's scope.
- `BoundLayoutObservation::observe` only succeeds on Unix; Windows and other non-Unix platforms
  fail closed with `SealError::Unsupported`, matching this crate's own precedent for every other
  capability without a verified non-Unix implementation.

## Rollback / recovery

Not applicable - no mutation capability is granted by this story; it establishes an observation
primitive with no production consumer yet.

## Owner decision

Independent verifier verdict: `PASS_WITH_RESIDUALS` (Round 8, Codex). This is the eighth
independent review round across ADR-0034, ADR-0035, and ADR-0036; each prior round's finding was
either repaired within an owner-authorized budget or, when that budget was exhausted (round 4),
resolved by an owner-directed structurally different design (ADR-0036) rather than a further
self-authorized patch. Recorded here as owner-visible per CR4's requirement
(`docs/CONSTITUTION.md`); the story moves to `done` on this verdict. E14-S05 remains open to
close the disclosed mutation-boundary-wiring and trusted-manifest residuals.
