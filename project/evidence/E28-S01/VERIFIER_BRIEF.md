<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E28-S01
Rendered-by: Claude Opus 5 (executor)
Rendered-on: 2026-09-15
Brief-Checksum: 941d7f0de0e0cd153057c2fd44cdcfd7a4087af4dcee343360d9e5c1e1278b59

<!-- end handoff header -->
# Verifier Brief - E28-S01 - What a toolchain component contains is scanned, not trusted

Status: ready_for_review | Change Risk: CR1
Outcome: The manifest records a trust tier per component and `check_agent_toolchain.py` enforces that a component which runs code, reaches the network or touches credentials is FirstParty or Vendor. That bar is about provenance, which is the right question and only half of it: nothing reads the component. A skill is prompt content injected into the agent that writes a file-deletion tool, and prompt content can carry instructions to ignore a rule, an exfiltration path, or a tool permission far wider than its purpose - none of which change its trust tier or its pinned version. The same argument this repository already accepts for crates (`cargo deny` reads the dependency, it does not ask who published it) applies here and has not been applied. NVIDIA's SkillSpector is the instrument: a CLI, Apache-2.0, 71 patterns across 17 categories, JSON and SARIF output, exit codes, and `--no-llm` to keep every file local. It enters as a pinned development dependency behind a gate script, in the shape every other gate here already has - not as a component in the manifest it would be checking.
Dependencies: none

## Acceptance Criteria
- The content of every prompt-bearing component the repository carries shall be scanned by a gate, and the gate's verdict shall come from a real scan rather than from the manifest's trust field.
- If the scanner reports a finding at or above a stated severity, then the gate shall refuse and name the component, the file and the pattern, so the owner decides against evidence rather than against a score.
- If the scanner is not installed, then the gate shall say so and refuse, rather than reporting a clean result it did not obtain.
- A finding the owner has accepted shall be recorded as an explicit, dated waiver naming the component and the pattern, and an unwaived finding shall not be silently inherited from a previous run.
- The scanner shall run without sending file contents to any third party by default, and any mode that does shall be opt-in and named in the evidence.
- The scanner's own version shall be pinned and recorded, because a scanner that changes what it detects changes the meaning of a passing run.

## Verification Contract
- The gate is shown failing on a deliberately planted malicious pattern in a scratch copy of a skill before it is shown passing on the real pack - a gate demonstrated only by passing is a gate whose failure path was never executed.
- A waiver is shown to suppress exactly the finding it names and no other.
- The 'scanner absent' path is exercised by running the gate with the tool removed from PATH, asserting refusal rather than a clean report.
- The recorded scanner version is asserted against the installed binary, the way scripts/check_coverage.py asserts its measurement toolchain.

## Safety Obligations
- none

## Documentation Impact
- docs/development/AGENT_TOOLCHAIN.md
- AGENTS.md
- .pre-commit-config.yaml

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
