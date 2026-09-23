<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E19-S01
Rendered-by: Claude (executor)
Rendered-on: 2026-09-23
Brief-Checksum: c10ad52d3cec6803f9258a0d0d414c0275502ba53eb46d21c87ef4a34ef06658

<!-- end handoff header -->
# Verifier Brief - E19-S01 - Desktop API boundary

Status: ready_for_review | Change Risk: CR3
Outcome: Expose a narrow local IPC/API that reuses engine plans and explanations.
Dependencies: E15-S04

## Acceptance Criteria
- Desktop cannot bypass safety executor.
- API is local-authenticated and versioned.
- If a local client connects without the session token, with a wrong token, or before authenticating, then the API shall refuse the request, disclose no engine data, and close the connection.
- If a client requests an API version the engine does not support, then the API shall refuse it and name the versions it supports.
- The API shall expose read-only documents only - the same inventory and plan documents the CLI's `status --json` and `plan --json` produce - and no request that performs or schedules a mutation.

## Verification Contract
- Unauthorized local client and schema compatibility tests.

## Safety Obligations
- none

## Documentation Impact
- docs/architecture/TARGET.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
