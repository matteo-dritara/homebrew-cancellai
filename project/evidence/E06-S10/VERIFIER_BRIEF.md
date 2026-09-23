<!-- Rendered by scripts/verifier_handoff.py. The checksum is over the body below this
header; a verdict answering this brief repeats it, so the ledger can tell whether the
verifier was given this document or a paraphrase of it. -->

Story: E06-S10
Rendered-by: Claude
Rendered-on: 2026-09-23
Brief-Checksum: d8e38c5fdaf9a6cf5fec0ab16438c50fbe24eecea7add2d53c03a4cac92afa8b

<!-- end handoff header -->
# Verifier Brief - E06-S10 - clean accepts --keep-claude-history and --verbose as the reference does

Status: ready_for_review | Change Risk: CR3
Outcome: G1 lists disclosed functional gaps. The owner chose to close the two clean flags before cutover, because a script that passes them to the canonical engine would otherwise break, and to accept the rest (--aggressive, status --paths/--coverage/--top) as disclosed divergences stated in the release notes. In the reference, --keep-claude-history does not protect an artifact: it turns off the rewrite of history.jsonl that drops lines tied to deleted Claude sessions. The Rust engine never rewrites history.jsonl - that rewrite is a second kind of mutation the one safety boundary does not have, the same reason E22-S05 declined the Codex native backend - so Rust already behaves as if the flag were always given. Accepting the flag is therefore exact, and the absent trim stays a disclosed divergence rather than being added here as a side effect. --verbose only reports more.
Dependencies: none

## Acceptance Criteria
- When clean is given --keep-claude-history, the system shall accept it and leave history.jsonl untouched, which is what the reference does with the flag.
- If clean runs without --keep-claude-history, then the system shall still leave history.jsonl untouched and say so in its output, because rewriting it is a mutation outside the one safety boundary and stays a disclosed divergence.
- When clean is given --verbose, the system shall report each performed action without changing which actions run.
- The system shall keep refusing --aggressive and the unsupported status flags as usage errors, and the release notes shall list them as intentional divergences.

## Verification Contract
- A CLI test shows --keep-claude-history accepted and history.jsonl byte-identical after clean, with and without the flag.
- A test shows the --verbose action set and results equal the non-verbose ones.

## Safety Obligations

### SI-019 One mutation boundary, evidence-gated

All filesystem/vendor mutations route through the safety executor. CR4 changes to this boundary require independent verification and owner-visible Safety Verdict.

Implemented at `rust/crates/cancellai-safety/src/mutation_executor.rs::execute` (E03-S05),
the sole production caller of `cancellai-platform::mutation::MutationExecutor`.
`scripts/check_mutation_boundary.py` statically enforces that the raw OS primitive and the
capability wrapping it are referenced only from those two files - E03 verifier review round 1
found the capability itself was `pub`, re-exported at `cancellai_platform`'s crate root, and
directly callable (with an unconstrained raw path) by any crate that imported it; repaired by
removing the re-export and extending the static check (`docs/architecture/TARGET.md`,
`docs/architecture/PLATFORM_MODEL.md`).

## Documentation Impact
- docs/CLI_RUST.md
- CHANGELOG.md

## Role
Verify independently from this contract and the final diff. Search for counterexamples; do not rely on executor reasoning. For CR4, issue an owner-visible Safety Verdict.
