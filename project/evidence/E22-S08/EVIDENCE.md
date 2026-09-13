# Evidence Packet - E22-S08

- Commit/PR: the Windows shell declaration on `main`
- Executor: Claude
- Independent verifier: none - the owner waived the Codex round for this session's work. The failing observation is release run 34545116539 (v1.12.0).
- Change Risk: CR1
- Spec version/commit: `project/epics/E22.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every such step declares its shell | One step needed it: `generate the CycloneDX SBOM for this target`. Every other multi-line step in `build-artifacts` already had `shell: bash`, which is why this one was easy to miss. | PASS |
| AC2 - the gate refuses and names the step | `windows_shell_errors()`, wired into `validate_workflows()`. Run against the committed workflow before the fix it named exactly the offending step and nothing else; after the fix the workflow set is clean. `test_a_multiline_run_with_no_shell_on_a_windows_leg_is_an_error`. | PASS |
| AC3 - mentioning Windows is not running on Windows | `runs_on_windows()` reads `runs-on` and, when that defers to the matrix, the matrix's own `os` values. The first version matched the word anywhere in the job body and flagged two jobs that run on macOS and Linux: one names `x86_64-pc-windows-msvc` in an artifact list, the other discusses a past Windows failure in a comment. `test_a_job_that_only_mentions_windows_is_not_a_windows_job`. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | The gate passing because it could not see the step | It did, at first. `job_steps` splits at the first `- ` marker, which in a matrix job is the `include:` list rather than the steps - so all four steps landed in one block and the missing `shell:` was masked by a sibling that had one. `steps_section()` scopes the split to the `steps:` key, and `test_steps_are_split_from_the_steps_key_not_the_matrix_list` pins it. A gate that reports clean because its parser collapsed the input is the failure mode worth naming here. | PASS |

## Verification Commands

```text
python3 scripts/check_workflows.py check   -> named the SBOM step before the fix; clean after
python3 -m pytest tests/test_workflows.py  -> 33 passed
python3 -m mypy scripts/check_workflows.py -> clean
```

## Compatibility

- CI configuration and a static checker. No product behaviour, no schema.

## Performance / operability

- Not applicable.

## Residual risks

- **The rest of that Windows leg is still unproven.** v1.12.0 failed before the SBOM was written,
  so nothing downstream of it has ever run on Windows: whether `cargo cyclonedx` names its output
  the same way there, and whether the `mv` step finds it, will be discovered by the next tag.
- **The gate checks that a shell is declared, not that the right one is.** A step declaring
  `shell: pwsh` and then writing bash passes.
- **A single-line `run:` is not examined.** It cannot carry a continuation, but it can still carry
  something PowerShell parses differently.

## Verifier verdict

closed on the owner's waiver; no independent verdict
