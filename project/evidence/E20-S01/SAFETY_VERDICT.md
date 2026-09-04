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
