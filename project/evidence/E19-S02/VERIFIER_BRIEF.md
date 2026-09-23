<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E19-S02
Rendered-by: Claude (executor)
Rendered-on: 2026-09-23
Brief-Checksum: f3743d691096e0e2e9eff43754d06170d6a45d62fa3ccebb7b8a306c2b04e18c

<!-- end handoff header -->
# Verifier Brief - E19-S02 - Cross-platform desktop shell

Status: ready_for_review | Change Risk: CR1
Outcome: Implement tray/menu-bar/dashboard experience using shared core, with Tauri or an equivalent evidence-backed choice.
Dependencies: E19-S01

## Acceptance Criteria
- Desktop is optional and core remains headless-capable.
- UI shows the same inventory/policy semantics as CLI/TUI.

## Verification Contract
- View-model parity tests.

## Safety Obligations
- none

## Documentation Impact
- docs/PRODUCT.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
