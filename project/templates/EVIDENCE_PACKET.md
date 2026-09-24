# Evidence Packet - <STORY-ID>

- Commit/PR: ...
- Executor: Claude | Codex | human
- Independent verifier: Claude | Codex | OpenCode/<model> | human
- Change Risk: CRx
- Spec version/commit: ...

## Outcome

PASS | PARTIAL | FAIL

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 | test/command/link | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-xxx | ... | ... | PASS |

## Verification Commands

```text
<commands>
```

## Compatibility

- Platforms/providers/schemas exercised: ...

## Performance / operability

- ...

## Documentation updated

- ...

## Method defects

<!-- Defects in how the work was done, not in what was built. "none" is the common and honest
answer. Each entry needs three things: what happened, what would have prevented it (a document,
gate or skill - or "none exists", which is itself the finding), and a Disposition the owner
decided. An observation nobody dispositioned is a note, not a finding, and the gate refuses it. -->

- none | **What happened**: ... **Prevented by**: docs/... | none exists **Disposition**: proposed | accepted 2026-01-01 | declined 2026-01-01 - reason

## Residual risks

- none | ...

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
