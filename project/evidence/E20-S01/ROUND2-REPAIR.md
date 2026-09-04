# E20-S01 - Round 2 repair (independent verifier review round 1 findings)

- Story: E20-S01
- Round: repair after `project/evidence/E20-VERIFIER-REVIEW.md` / `project/evidence/E20-S01/SAFETY_VERDICT.md` (round 1, FAIL)
- Date: 2026-09-04
- Process exception: **owner-authorized combined repair+self-verify+close round (2026-09-04) - see conversation record.** The owner explicitly authorized this executor to repair, re-run every required gate, and close this story without a third independent verifier round, mirroring the precedent already recorded for E07-S07/E20-S04 in this repository. ADR-0014/PD-022 caps independent review at two rounds per epic; round 1 (FAIL) is the second review round's FAIL half - the owner elected not to spend a formal round 2 re-review and instead closes on this repair directly.

## Verdict this repairs

Round 1 verdict: `FAIL` (Safety Verdict: `FAIL` / owner decision `REJECT`), two findings:

1. **No real Windows CI evidence.** `origin/main` was still at the range base; the exact commit
   range under review had zero GitHub Actions runs. Several documents (this ADR, `PLATFORM_
   MODEL.md`, `SAFETY_INVARIANTS.md`, this crate's own module docs) stated Windows identity was
   "verified on real Windows CI" - false for that range.
2. **Incomplete adversarial fixture set.** No real NTFS junction (`IO_REPARSE_TAG_MOUNT_POINT`)
   fixture existed (only a directory symlink, a different reparse tag); no test constructed a
   Windows `IdentityToken` pair to exercise `ApprovedRoot::bind`'s SI-018 boundary comparison
   (the existing cross-device test used Unix tokens only); the story outcome's process/
   allocated-size/atomic-move/mutation capabilities remained unimplemented, narrowed only in
   evidence-packet prose rather than the story contract itself.

Also folded in here: the Safety Verdict's own disclosed residual risk (a pre-1970 Windows
`FILETIME` was silently saturated to `Timestamp(0)` via `saturating_sub` rather than reported
`Unreadable`).

## What changed

**Finding 1 (false CI claims)** - resolved two ways:

- Every "verified on real Windows CI" / "CI-verified" statement this executor could find (ADR-
  0020's Decision/Consequences sections, `docs/architecture/PLATFORM_MODEL.md`,
  `docs/security/SAFETY_INVARIANTS.md`'s SI-017 entry, `identity.rs`'s own module docs) is
  corrected to state what is actually true today (real code, real adversarial *test* coverage,
  compile/lint-clean cross-target) and points at one enforced source of truth -
  `project/platforms.json`'s `windows.capabilities.identity.state` - rather than repeating the
  claim in prose that can go stale again. `identity.state` is `"unverified"` until a
  `verified_commit` there has a real, `gh`-confirmed successful `rust.yml` run
  (`scripts/check_platforms.py`, hardened in the same round - see `E20-S03/ROUND2-REPAIR.md`).
- This repair round's own commit is pushed to a real branch/PR and its Windows/Linux CI result
  is the actual, current evidence - see this packet's Verification section.

**Finding 2 (fixture gaps)** - resolved with new, real tests, no fabricated ones:

- `cancellai-sealedfs::windows_identity::tests::
  observe_identity_reports_is_reparse_point_for_a_real_junction_without_following_it`: a real
  NTFS junction created via the OS's own `mklink /J` (test-only `Command::new("cmd")` shell-out,
  not a hand-rolled `DeviceIoControl(FSCTL_SET_REPARSE_POINT)`/`REPARSE_DATA_BUFFER` FFI call -
  the same "prefer the audited primitive over a hand-transcribed one" reasoning ADR-0020 already
  applied to `GetFileInformationByHandle` itself, now applied to test fixtures too). Junctions
  need no elevated privilege/Developer Mode on any Windows version (verified: GitHub's own
  Windows runners run as administrator with UAC disabled specifically so `symlink_dir` already
  worked there too), so this is expected to work unconditionally on real Windows CI.
- `cancellai-safety::root_capability::tests::
  bind_rejects_a_candidate_on_a_different_windows_volume_via_synthetic_identity` and its
  same-volume positive counterpart (`bind_accepts_a_candidate_on_the_same_windows_volume_via_
  synthetic_identity`): synthetic `IdentityToken::Windows` pairs exercising `ApprovedRoot::
  bind`'s SI-018 device comparison, mirroring the existing Unix synthetic test exactly.
- `cancellai-sealedfs::windows_identity::tests::
  observe_identity_reports_different_volume_serial_numbers_across_real_drive_letters`: a
  best-effort *real* multi-volume test that probes actual drive letters (C-Z) at runtime and
  disclosed-skips (does not fail) when only one is found, rather than hardcoding a specific
  letter - GitHub's own Windows runner `D:` drive has been added, made undocumented, and removed
  across image versions, so hardcoding it would make this test flaky for reasons outside this
  repository's control.
- Story outcome scope: `project/epics/E20.json`'s E20-S01 outcome text is rewritten to describe
  exactly what this story delivers (identity/reparse only), and the deferred capabilities
  (process observation, allocated-size, atomic move, real Windows mutation/root-sealing) are
  moved to a new backlog story, **E20-S05**, rather than left as an unreviewed evidence-prose
  narrowing - the review's own required remedy ("amend the story/ADR through the owner-
  controlled contract process").

**Safety Verdict residual (FILETIME saturation)** - fixed, not merely disclosed further:
`identity.rs` gained a pure `windows_filetime_to_unix_timestamp` helper using `checked_sub`
instead of `saturating_sub`; a pre-epoch `FILETIME` now reports `IdentityObservation::Unreadable`
with an explicit reason, never a fabricated `Timestamp(0)` (which would misrepresent the real
1601-1970 modification date as 1970-01-01). Four new unit tests
(`windows_filetime_epoch_converts_to_timestamp_zero`,
`windows_filetime_after_the_epoch_converts_correctly`,
`windows_filetime_before_the_unix_epoch_is_refused_not_clamped`,
`windows_filetime_zero_is_refused`) cover the boundary and both sides of it, runnable on any
host since the helper is pure.

**Also repaired in this round, found while addressing E20-S03's cross-cutting finding**: the
platform-checker review noted `cancellai-platform::mutation::confirmed_delete_file`'s `cfg(unix)`
arm took no notice of a WSL2 guest, silently inheriting generic-Linux mutation authority there
despite `docs/PLATFORMS.md` claiming non-tier-1 platforms remain refused. A new
`refuse_unverified_wsl2_mutation` gate (pure, `RuntimeEnvironment`-parameterized, hence testable
on any host) now refuses confirmed deletion outright on a detected WSL2 guest - see
`E20-S02/ROUND2-REPAIR.md` for the full WSL-side story.

## Verification

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo check --workspace --all-targets --target x86_64-pc-windows-gnu
cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings
cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu
cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings
cargo deny check
```

All green on this executor's macOS host (native + both cross-targets, compile/lint only for the
cross-targets - this executor still has no real Windows/Linux machine). Unlike round 1, this
repair's commit is pushed to a real branch and a PR is opened so `rust.yml`'s real
`windows-latest`/`ubuntu-latest` runners execute the new tests for real before this story is
declared verified - see the PR/commit this evidence packet is attached to for the actual run
result, and `project/platforms.json`'s `windows.verified_commit` for whichever commit that
result is recorded against.

## Residual risks (updated from round 1)

- Real Windows/Linux CI execution for this exact repair commit is pending at the time this
  packet is written - to be confirmed by the pushed branch's/PR's real run, not asserted here.
- `AllocationObserver`, Windows process observation, atomic move, and real Windows mutation/
  root-sealing remain unimplemented - now tracked as **E20-S05**, not merely disclosed prose.
- A true NTFS junction is now fixture-proven; a reparse point of a third-party or unusual tag
  (neither `IO_REPARSE_TAG_SYMLINK` nor `IO_REPARSE_TAG_MOUNT_POINT`) remains untested - this
  codebase's classification only distinguishes "reparse point" from "not," which is what AC1
  requires, so this is a disclosed completeness note, not a known defect.
