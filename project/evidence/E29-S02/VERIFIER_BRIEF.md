<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E29-S02
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 0f8b630eb8886cdd43346a5af3dcf81bf56f861b2e4b18dd09a1d44891a5e0b2

<!-- end handoff header -->
# Verifier Brief - E29-S02 - A scanner upgrade invalidates the waivers written against the old one

Status: ready_for_review | Change Risk: CR2
Outcome: `project/skill_content_waivers.json` suppresses findings by `match_fingerprint`, which was measured stable across two runs of SkillSpector 2.11.2 and is untested across versions. The pin in `check_skill_content.py` is what protects it today, which means the protection disappears exactly when somebody bumps the pin - and a bump is a routine act that currently carries no obligation to revisit the waivers. A stale waiver that stops matching is reported; a waiver whose fingerprint drifted onto a different finding would not be.
Dependencies: none

## Acceptance Criteria
- The waiver file shall record the scanner version its fingerprints were computed against.
- If the gate's pinned scanner differs from the version the waivers were written against, then the gate shall refuse until the waivers are explicitly revalidated.
- Revalidation shall be an explicit dated act recorded in the waiver file, not a side effect of the gate passing once.

## Verification Contract
- An old waiver is shown unable to suppress a finding after a pin change, before the revalidation is recorded and after.
- The refusal names both versions, so the reader knows what changed.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_TOOLCHAIN.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
