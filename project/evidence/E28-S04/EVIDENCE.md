# Evidence Packet - E28-S04

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
| AC1 | `measured_tokens()` reads each member skill's YAML frontmatter; a declaration more than 25% from the measurement refuses, naming both numbers. `tests/test_agent_toolchain.py::DeclaredCostAgainstMeasurement::test_an_inflated_declaration_refuses` | PASS |
| AC2 | A user-scope component returns `None` and is reported `unmeasured`, never confirmed. `tests/test_agent_toolchain.py::DeclaredCostAgainstMeasurement::test_an_unmeasurable_component_is_noted_not_confirmed` | PASS |
| AC3 | `license` is a required field and `license_allowlist` gates it; GPL-3.0 refuses. `tests/test_agent_toolchain.py::ComponentLicences::test_a_licence_outside_the_allow_list_refuses` | PASS |
| AC4 | `NONE` refuses as its own state, distinct from a licence not yet recorded. `tests/test_agent_toolchain.py::ComponentLicences::test_an_unlicensed_source_refuses_as_its_own_distinct_state` | PASS |
| AC5 | The allow-list is stated once in `project/agent_toolchain.json` and is not a copy of `rust/deny.toml`'s. `tests/test_agent_toolchain.py::ComponentLicences::test_the_allow_list_is_not_a_copy_of_the_crate_allow_list` | PASS |

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

- **What happened**: `measured_tokens()` read `members` as bare skill names when they are written `skill:.claude/skills/<name>`. It found no files, returned `None`, and the component reported itself **unmeasured** - a silent non-measurement passing as a green gate, which is the exact defect class this story exists to remove. It was caught by printing the measurement rather than by the gate, which stayed green throughout. **Prevented by**: none exists; a gate that can report 'unmeasurable' needs a case proving it measures something when it should. **Disposition**: proposed

## Residual risks

- `CHARS_PER_TOKEN = 4.0` is an approximation and the real figure is model-specific; the 25% tolerance exists because of it, and a component genuinely 20% mis-declared passes.
- Only `cancellai-skill-pack` is measurable from this repository. Ten of the twelve components are user-scope and remain declared-and-unmeasured, which is the honest state but leaves most of the budget unverified.
- Licences are recorded from the upstream's declaration at a point in time; nothing re-checks that an upstream has not relicensed. E26-S02 checks versions and abandonment, not licences.

## Verifier verdict

pending
