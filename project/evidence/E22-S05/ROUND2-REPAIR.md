# E22-S05 - Round 2 repair (independent verifier review round 1 findings)

- Story: E22-S05
- Round: repair after `project/evidence/E22-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04

## Verdict this repairs

Round 1 verdict: FAIL. E22-S03's clap migration replaced `docs/CLI_RUST.md`'s
`## Known gaps versus the Python reference (tracked, not silent)` heading with
`## Argument parsing` while retaining every gap bullet (including E22-S05's Codex disclosure)
underneath it. `docs/PROVIDERS.md`, `CHANGELOG.md`, and this story's own AC1 all point at a
"Known gaps" section that no longer existed. The review also asked that "permanent divergence"
be reconciled with the evidence packet's own statement that wiring remains wanted future work.

The verifier's independent structural check of the no-wiring decision itself (CR4-shaped
mutation-boundary reasoning, the four `NativeDeleteSupport` outcomes staying distinct) was
already correct and required no change.

## What changed

- `docs/CLI_RUST.md`: restored a real `## Known gaps versus the Python reference (tracked,
  not silent)` heading immediately before the Codex/`--aggressive`/platform-gap bullet list,
  and moved the `--help`/`-h`/`--version` precedence note (E22-S03) into "Argument parsing"
  where it belongs, since it is a parsing-precedence decision, not a reference-parity gap.
  `docs/PROVIDERS.md` and `CHANGELOG.md`'s existing "Known gaps" references now resolve to a
  real section again with no further edits needed there.
- Reconciled "permanent" with "wanted future work": the Codex bullet now states explicitly
  that "permanent" means this build does not close the gap and will not close it as a side
  effect of an unrelated story - not that closing it is out of scope forever. Wiring it
  remains real, wanted future work, gated behind its own dedicated CR4 story, matching
  `project/evidence/E22-S05/EVIDENCE.md`'s "Residual risks" section rather than contradicting
  it.

## Verification

`python3 scripts/check_docs.py check` passes (183 Markdown files; local links and safety IDs
consistent) - the "Known gaps" cross-references from `docs/PROVIDERS.md` and `CHANGELOG.md`
now resolve. `rg '^## ' docs/CLI_RUST.md` shows Commands, Shared flags, Exit codes, Argument
parsing, and Known gaps versus the Python reference (tracked, not silent) - the section the
story's AC1 names exists again, with the Codex disclosure directly underneath it.

## Residual risk

Same as round 1: wiring the native Codex delete path remains open, dedicated future work
(kernel mutation-boundary change, CR4-shaped, its own TM-09 review) - not resolved by this
repair, which is documentation-only.
