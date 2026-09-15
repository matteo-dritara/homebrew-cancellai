<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E28-S04
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 89b88f7741dc6ed57d58d678d9ffb7ec7eeacc822c522ad27dd8ba755e0934de

<!-- end handoff header -->
# Verifier Brief - E28-S04 - The manifest's own fields are measured or constrained, not merely recorded

Status: ready_for_review | Change Risk: CR1
Outcome: E26 built a manifest whose fields are judgements, and E28-S01 measures one of them. Two others are still asserted. `always_on_tokens` is a number a person typed: `check_agent_toolchain.py` sums it and compares the total to the budget, and nothing compares any entry to the component's real description - the 3206-of-6000 figure that decided against Task Observer rests on inputs nobody verified, and a number wrong by a factor of ten would pass. `license` does not exist at all: this repository keeps a licence allow-list in `deny.toml` for Rust crates and governs nothing for the prompt content it carries into the agent. The 2026-09-15 census made that concrete - `anthropics/skills` (176k stars), `vercel-labs/agent-skills` and `hamelsmu/claude-review-loop` carry no licence file, so by default all rights are reserved; one candidate was GPL-3.0; and the pack already installed, `trailofbits/skills`, is CC-BY-SA-4.0, a share-alike licence that matters because AGENTS.md records harvesting patterns from external packs into `.claude/skills/`. The two are one change because they are one defect on one file: a manifest that records what somebody believed.
Dependencies: none

## Acceptance Criteria
- A component's declared always-on cost shall be checked against a measurement of what it actually contributes, and if the two disagree beyond a stated tolerance then the gate shall refuse and name both numbers.
- If a component's always-on cost cannot be measured - the component is installed at user scope and absent from the repository - then the gate shall report it as unmeasured rather than treat the declared number as confirmed.
- Every component shall record the licence of its source, and a licence outside the allow-list shall refuse until the allow-list itself is changed by a reviewed edit.
- If a component's source declares no licence at all, then that shall be recorded as the distinct state it is - all rights reserved by default - and shall not pass as though it were permissive.
- The allow-list shall be stated once in the manifest, not duplicated from deny.toml, because two lists that must agree eventually do not.

## Verification Contract
- The token gate is shown refusing a deliberately inflated declaration before it is shown passing on the real manifest, and the tolerance is tested from both sides.
- A component is given an unlicensed source in a scratch manifest and the gate is shown to refuse it, distinguishing 'no licence' from 'licence not yet recorded'.
- The measured figure is compared against the number E26 recorded by hand, and any disagreement is reported rather than silently corrected - the point is to learn whether hand-entry was reliable.
- The allow-list is shown to refuse GPL-3.0, using the real candidate the census rejected.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_TOOLCHAIN.md
- project/agent_toolchain.json
- AGENTS.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
