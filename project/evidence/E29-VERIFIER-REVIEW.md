# Independent Verifier Review — E29

Review-Scope: epic
Round: 1
Verifier: Codex

## Verdicts

| Story | Verdict | Evidence |
| --- | --- | --- |
| E29-S01 | REPAIRED | ADR-0025 defined yield as `FAIL`/rejection while the report used findings; the ADR and protocol now agree. Epic-record checksums are now checked per story. Brief-Checksum: 00b9e4048efd755637d23ce6534926f5096139f11b39390a9688be4a8306a3f6 |
| E29-S02 | REPAIRED | Reproduction: matching scanner version plus no `revalidated` date returned `[]`. It now refuses absent, malformed, and future dates. Brief-Checksum: 0f8b630eb8886cdd43346a5af3dcf81bf56f861b2e4b18dd09a1d44891a5e0b2 |
| E29-S03 | FAIL | ADR-0029 is `Proposed`, so it is a recommendation by its executor, not the owner decision required by AC1. No verifier may accept that residual on the owner's behalf. Brief-Checksum: 03b8df300a8bb340628be11d1223efb2f2d48bdb3cb5ad13b5ed97105feb7e29 |
| E29-S04 | PASS_WITH_RESIDUALS | Aged proposals report without failing. A date can still be reset without comparison to git history; the packet names that residual. Brief-Checksum: fbccd60d2bc26f28870e263744c9a9f90b5c1c884e956df16827642c15e87c94 |
| E29-S05 | REPAIRED | Reproduction: `{tokens, component_version}` without `taken_on` was current. Incomplete measurements are now unmeasured; origin of a claimed measurement remains unauthenticated. Brief-Checksum: 07575fd4d09140c2d03085360a1d0825052e709a2b3b0b46d499e58969b0dbd8 |
| E29-S06 | REPAIRED | The delivered binding never compared a licence. `updates` now compares source revision and current upstream, and says `could not compare` on unavailable evidence. Brief-Checksum: 2d5117b92270859d8badc5e227e59b1803ea65e41dd6df7841e262606febb345 |
| E29-S07 | REPAIRED | The policy test only searched for existing words, so code and test could widen without a policy edit. It now pins the exact policy statement. Brief-Checksum: 3b58c93324cfd6a21e7091de984d1ea1911ac1a5a889eae32516ba91c5364a9d |

## Residuals

- S01 cannot infer a repair that a reviewer deliberately records as `PASS`; `REPAIRED` removes tool blindness, not a false claim by the reporter.
- S03 needs an owner decision accepting or rejecting ADR-0029 before the story can pass.
- S04 has no immutable first-seen timestamp; a proposal can be re-dated.
- S05 has no attestation that a claimed local token count came from its named component.
- S06 records revisions resolved from default-branch HEAD for unpinned plugins, not installation provenance.

## Verification

- `python3 -m pytest tests -q`
- `pre-commit run --all-files`
- `python3 scripts/process_metrics.py check`
- `python3 scripts/process_metrics.py report`
- `python3 scripts/check_agent_toolchain.py check`
- `python3 scripts/check_skill_content.py check`
- `python3 scripts/verifier_handoff.py check`
