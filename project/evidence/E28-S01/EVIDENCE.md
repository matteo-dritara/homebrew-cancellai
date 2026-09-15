# Evidence Packet - E28-S01

- Commit/PR: on `main`, this epic's commits
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR1
- Spec version/commit: project/epics/E28.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | `scripts/check_skill_content.py check` runs SkillSpector over `.claude/skills` and reads the per-skill `issues`; the verdict comes from that scan, not from the manifest's `trust` field. It now also refuses `skills_omitted > 0` and `entirely_uninspected_files > 0`, so an omitted skill or file cannot pass without entering the finding ladder. `tests/test_skill_content.py::SeverityLadder` and `::ScanCoverage` | PASS after verifier repair |
| AC2 | A HIGH or CRITICAL finding refuses, naming skill, severity, category, pattern, file and line - demonstrated end to end on a planted skill: two refusals at `SKILL.md:6` (Prompt Injection / Instruction Override) and `SKILL.md:7` (Privilege Escalation / Credential Access). `tests/test_skill_content.py::SeverityLadder::test_refusal_severity_and_above_refuse` | PASS |
| AC3 | `provenance_errors(None)` refuses before scanning; the gate draws no conclusion rather than reporting a pack it never read. `tests/test_skill_content.py::ScannerProvenance::test_an_absent_scanner_refuses_rather_than_reporting_a_clean_pack` | PASS |
| AC4 | A waiver names one `match_fingerprint`, a reason and a date, and suppresses only that finding; an unwaived finding is not inherited. `tests/test_skill_content.py::Waivers`; the committed waiver is `project/skill_content_waivers.json` | PASS |
| AC5 | The scan is `--no-llm` and a test asserts the flag is present in `scan()`, so no file content leaves the machine by default. `tests/test_skill_content.py::ScannerProvenance::test_the_scan_never_enables_the_provider_backed_analysers` | PASS |
| AC6 | `PINNED_SCANNER = "2.11.2"`; a different installed version refuses before scanning. `tests/test_skill_content.py::ScannerProvenance::test_a_different_scanner_version_refuses_before_scanning` | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | This story declares no safety obligation: it governs the toolchain and the process, not the mutation boundary. | The mutation-boundary gate (SI-019) is unchanged and still passes. | PASS |

## Verification Commands

```text
python3 -m pytest tests -q
python3 scripts/check_skill_content.py check
python3 scripts/verifier_handoff.py check
python3 scripts/check_agent_toolchain.py check
python3 scripts/check_evidence.py check
pre-commit run --all-files
```

## Compatibility

- Platforms/providers/schemas exercised: macOS, Python 3.13. No product surface is touched; no schema changed.

## Performance / operability

- The skill-content scan adds roughly 30 seconds to `pre-commit run --all-files`. It is the only gate in the set that shells out to a third-party analyser.

## Documentation updated

- `docs/development/AGENT_TOOLCHAIN.md`, `docs/development/AGENT_PROTOCOL.md`, `AGENTS.md`

## Method defects

- **What happened**: the gate's first `scanner_version()` matched the banner case-insensitively but split the original string on an uppercase `V`, so it raised `IndexError` on the real output - a gate failing closed for the wrong reason, which reads to a caller exactly like a scanner that is not installed. **Prevented by**: none exists; no rule here says a checker's own parsing of a tool's output must be exercised against that tool's real output before the checker is trusted. **Disposition**: proposed

- **What happened**: moving the scanner into `requirements-dev.txt` broke the Python 3.10 leg of the test matrix outright - `pip install -r requirements-dev.txt` failed with `Package 'skillspector' requires a different Python: 3.10.11 not in '<3.15,>=3.12'`. The constraint had been printed in this session's own output hours earlier, when the scanner's `pyproject.toml` was read to find its version, and was not carried forward. The change made the tooling installable by one command while making it uninstallable on a supported interpreter, which is the same defect it was meant to remove, pointed the other way. Repaired with a `python_version >= "3.12"` marker, verified by resolving the file against both 3.10 and 3.14. **Prevented by**: none exists; nothing here requires a new dependency to be resolved against every interpreter the test matrix declares before it is committed, and the local machine runs only one of them. **Disposition**: proposed

## Residual risks

- SkillSpector reports `analysis_completeness: partial` with `--no-llm` on this pack: all eight files are partially inspected and none fully. The gate prints this on every run rather than hiding it, but a static-only pass is a weaker claim than the word "scanned" suggests.
- The scanner's severity assignment is its own judgement, not this repository's. A finding it rates MEDIUM that this project would consider disqualifying passes the gate and appears only in the report.
- Waivers are matched on `match_fingerprint`, which was stable across two runs of one version. Stability across versions is untested and the pin is what protects it.

## Verifier verdict

pending
