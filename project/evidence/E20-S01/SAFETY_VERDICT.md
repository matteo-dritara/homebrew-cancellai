# Safety Verdict - E20-S01

- Change: Windows native file/volume identity and reparse observation
- Risk: CR4
- Review target: `54b8f3567b958db767376fefde1eb1f8d9c75963..30ce089e2946d557740772777927f5d499b41622`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-04

## Verdict

`FAIL`

## Safety surface changed

Windows `SystemIdentityObserver` changes from an unconditional `Unsupported` result to a real
`IdentityToken::Windows`, using Win32 volume serial number/file index/reparse attributes. That
token can establish and bind an `ApprovedRoot`, changing the identity and filesystem-boundary
authority available on Windows. Windows deletion/root sealing remains explicitly unsupported.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-017 | Windows identity/reparse semantics gain authority only with a verified native mapping; unsupported semantics lower authority. | Windows GNU cross-target check/clippy pass, but the exact target has zero GitHub runs and has never executed its Windows-only code/tests. Nevertheless the implementation returns `Identity`, and multiple canonical documents claim real Windows verification. The fixture covers a directory symlink but not a true NTFS junction. | FAIL |
| SI-018 | Recursive mutation/quarantine cannot silently cross Windows volumes or junction boundaries. | `IdentityToken::device()` returns the Windows volume serial and `ApprovedRoot::bind` compares it, but the only synthetic cross-device test constructs Unix tokens. No Windows-token boundary test, real multi-volume fixture, junction fixture, or Windows mutation/quarantine fixture exists. | FAIL |

## Adversarial cases

- Queried GitHub for the exact target SHA: no Actions runs exist; remote `main` is the base.
- Searched every `IdentityToken::Windows` construction and every Windows-native test. No
  Windows volume-boundary case exists.
- Searched native reparse fixtures. The sole link fixture uses
  `std::os::windows::fs::symlink_dir`; no NTFS junction is constructed.
- Confirmed by source and local tests that Windows deletion remains refused at both the safety
  executor and platform mutation layers; no unintended Windows deletion path was found.
- Cross-target check/clippy passed, establishing type/cfg correctness but not Win32 behavior.

## Differential / compatibility evidence

- `cargo check --workspace --all-targets --target x86_64-pc-windows-gnu`: PASS.
- Windows cross-target clippy with `-D warnings`: PASS.
- `cargo check --workspace --all-targets --target x86_64-unknown-linux-gnu`: PASS.
- Full local macOS Rust and Python gate sets: PASS.
- Exact-target `windows-latest` and `ubuntu-latest` GitHub Actions: FAIL (no runs exist).
- Real WSL2: unavailable and not claimed as E20-S01 evidence.

## Known residual risks

- The unexecuted FFI path may fail on real Windows despite cross-compilation.
- Junction/reparse-tag behavior and Windows volume-boundary enforcement are unverified.
- Pre-Unix-epoch Windows `FILETIME` values are saturated into `Timestamp(0)` rather than
  reported unreadable; the raw subsecond remainder does not preserve the discarded whole
  pre-epoch time. This was source-inspected but not independently exercised on Windows.
- Allocated-size, process, atomic-move, root-sealing, and mutation capabilities named by the
  story outcome remain unimplemented.

## Rollback / recovery

Do not close or release E20-S01. Until native Windows evidence exists, restore the Windows
observer to `IdentityObservation::Unsupported` or keep the change unreleased and remove claims
that it has earned verified authority. No real provider data was touched during review.

## Owner decision

`REJECT`

Owner note: E20-S01 must complete the required Windows CI and junction/volume adversarial
coverage, and reconcile the unimplemented story-outcome capabilities, before round 2.

## Addendum: owner-authorized closure after repair (2026-09-04)

Every finding above was repaired by the executor in the same session
(`project/evidence/E20-S01/ROUND2-REPAIR.md`): a real NTFS junction fixture (`mklink /J`), a
synthetic Windows-token volume-boundary test pair, a best-effort real multi-volume test, the
`checked_sub` fix for the disclosed `FILETIME` saturation residual, and the story-outcome
capabilities not delivered here moved to a new backlog story (E20-S05) through the story
contract itself.

The repaired commit was pushed and its real `windows-latest`/`ubuntu-latest` `rust.yml` run
(https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33885998630) found one
further genuine defect this session's local checks could not have caught - a stale pre-E20-S01
test still asserting the old `Unsupported`-on-Windows behavior - fixed in a follow-up commit
whose own real run
(https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/33886584899) passed every
job on all three platforms, both MSRV and stable. `project/platforms.json`'s
`windows.capabilities.identity.state` is `"verified"`, citing that commit
(`8622405118127c723f559d5ccdffdd0b3d7e0568`) as `verified_commit`
(`scripts/check_platforms.py check` independently re-confirms this offline via `git
merge-base --is-ancestor` and, network-permitting, via `gh run list`).

**This addendum is an owner decision, not a new independent-verifier PASS.** The project owner,
present in this session, explicitly authorized the executor to repair, re-run every required
gate including real Windows/Linux CI, and close this story without spending a formal round-2
independent-verifier re-review - mirroring the precedent already recorded for E07-S07/E20-S04 in
this repository (`docs/development/AGENT_PROTOCOL.md`'s "never write your own CR4 Safety
Verdict" describes the *default* path; this is the same kind of owner-elected exception those
precedents already established, recorded here rather than silently taken). SI-017/SI-018 are now
both backed by real CI-executed Windows fixtures, and the WSL2 mutation gap the same review round
found (via E20-S03's cross-cutting finding) is closed at the source
(`cancellai-platform::mutation::refuse_unverified_wsl2_mutation`).

Owner decision (this addendum): **ACCEPT**, story closes to `done`.
