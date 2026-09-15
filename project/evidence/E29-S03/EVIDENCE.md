# Evidence Packet - E29-S03

- Commit/PR: on `main`, this epic's commit
- Executor: Claude
- Independent verifier: pending - Codex
- Change Risk: CR2
- Spec version/commit: project/epics/E29.json

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | ADR-0029 decides not to adopt per-role identities and states the residual verbatim. | PASS |
| AC2 | Not applicable: the decision is not to adopt, and the ADR says why rather than naming a mapping that will not exist. | PASS |
| AC3 | ADR-0029's cost section treats squash merges explicitly - the commit introducing a verdict on `main` is routinely not the one signed. | PASS |
| AC4 | ADR-0029 records why a single shared owner key answers a different question, so the shortcut is not re-proposed. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| none declared | This story governs the engineering process and the agent toolchain, not the mutation boundary or any runtime authority. | The mutation-boundary gate (SI-019) is unchanged and still passes. | PASS |

## Verification Commands

```text
python3 -m pytest tests -q
pre-commit run --all-files
python3 scripts/process_metrics.py check
python3 scripts/check_agent_toolchain.py check
python3 scripts/check_skill_content.py check
python3 scripts/check_evidence.py check
```

## Compatibility

- Platforms/providers/schemas exercised: macOS, Python 3.13. No product surface, no schema change.

## Performance / operability

- None measurable. All changes are to governance scripts already in the gate set.

## Documentation updated

- `docs/development/AGENT_PROTOCOL.md`, `docs/development/AGENT_TOOLCHAIN.md`, `docs/development/WORK_ITEM_MODEL.md`, `docs/adrs/0029-a-verdict-s-author-is-declared-not-authenticated-and-that-is-accepted-for-now.md`

## Method defects

- **What happened**: the executor proposed binding a verdict's authorship to the GPG signature on its commit, and put that proposal in the verifier's own mandate as a likely closure. It is wrong: this repository signs with one owner key, so the signature authenticates the owner and says nothing about which agent wrote the verdict. The executor had read `commit.gpgsign` and reasoned from the presence of signing to the presence of per-actor identity. **Prevented by**: none exists; nothing requires a proposal put to a reviewer to state what it assumes about the mechanism it names. **Disposition**: accepted 2026-09-15 - recorded in ADR-0029 so the shortcut is not re-proposed


## Residual risks

- The decision is to accept the residual. Attribution in the evidence ledger stays a declaration, and the process-metrics independence split rests on the same declaration.
- ADR-0029 is `Proposed`: the executor drafted it and acceptance is the owner's. Until accepted it records a recommendation, not a decision.
- The revisit conditions are stated but nothing watches for them.

## Verifier verdict

pending
