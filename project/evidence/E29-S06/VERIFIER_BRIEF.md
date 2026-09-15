<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S06
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 2d5117b92270859d8badc5e227e59b1803ea65e41dd6df7841e262606febb345

<!-- end handoff header -->
# Verifier Brief - E29-S06 - A licence recorded once is revalidated against the source it was read from

Status: ready_for_review | Change Risk: CR2
Outcome: E28-S04 made `license` a required field with an allow-list, and the value is the upstream's declaration read at one moment. Nothing re-reads it: an upstream that relicenses after the entry was written leaves the manifest asserting something that was true and no longer is. E26-S02 already checks versions and abandonment for the same components and does not check licences, so the mechanism to hang this on exists.
Dependencies: none

## Acceptance Criteria
- A recorded licence shall name the source revision it was read from.
- If the upstream's licence at that revision no longer matches what was recorded, then the check shall report the disagreement and name both values.
- The check shall degrade truthfully with no network, reporting that it could not compare rather than that nothing changed.
- A licence that cannot be re-read shall not silently become permissive.

## Verification Contract
- A component whose upstream licence has changed is shown to be reported, using a synthetic manifest entry.
- The no-network path is exercised and shown to report inability rather than agreement.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_TOOLCHAIN.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
