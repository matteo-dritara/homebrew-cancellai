# Evidence Packet - E10-S03

- Story: E10-S03 — Verifier repair: reclaim probe authority and risk correction
- Risk: CR4
- Author: Codex, acting under the owner's 2026-09-13 authorization to repair and ship the
  independent review findings
- Date: 2026-09-13

## Outcome

The released E10-S01 implementation is independently classified CR4. Its declared CR2 remains
historical; `project/risk_floors.json` records the disagreement rather than rewriting the
released story. ADR-0027 supplies the missing authority for the macOS `statfs` observation in
the otherwise mutation-focused unsafe crate.

## Evidence

- Native macOS: `cargo test -p cancellai-platform filesystem_kind -- --nocapture` passes,
  including a real-path observation; `cargo test -p cancellai-sealedfs` is included in the
  workspace gate.
- Source review confirms `CString::new` rejects an interior NUL, non-existent paths return the
  OS error, signed `c_char` values preserve their byte representation through `as u8`, and lossy
  decoding forces an unrecognised value into the conservative classifier.
- `cargo miri test -p cancellai-sealedfs --lib macos_filesystem` was attempted. The active
  `stable-aarch64-apple-darwin` toolchain has no Miri component, and installing a toolchain is an
  owner decision, so it was not installed.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `project/risk_floors.json` records E10-S01 as independently classified CR4 by Codex and reports the disagreement. | PASS |
| AC2 | ADR-0027 names the FFI boundary, ABI assumptions, failure behaviour, alternatives, and CR4 consequence. | PASS |
| AC3 | Native sealedfs tests cover real and missing paths; source review verifies `CString` NUL refusal and lossy `c_char` decoding; classifier tests prove unknown names downgrade. | PASS_WITH_RESIDUALS |

## Residual risks

The probe follows Darwin symlink semantics because it observes only the backing filesystem; it
does not bind or mutate a path. Windows remains `Unsupported`, and unknown filesystem names are
never verified reclaim.
