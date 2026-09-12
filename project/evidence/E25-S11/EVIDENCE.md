# Evidence Packet - E25-S11

- Executor: Claude | Independent verifier: self-review | Change Risk: CR0
- Spec version/commit: `project/epics/E25.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a review record names the documents it opened | `project/templates/VERIFIER_PROMPT.md` and `.claude/skills/epic-verifier/SKILL.md` both require it, with the reason: it is the only readership signal this project has. | PASS |
| AC2 - once per phase, the never-opened documents are listed for an owner decision | `scripts/process_metrics.py`'s "Documentation readership" section, generated into `project/generated/PROCESS_METRICS.md`. | PASS |
| AC3 - no document is deleted by this story | Nothing is deleted. The output is a list and a sentence saying it is a list. | PASS |
| AC4 - accepted ADRs remain undeletable regardless of readership | `docs/adrs/` is in `READERSHIP_EXEMPT`, and the report states the rule in the text a reader sees. | PASS |

## What it found

**6 documents have ever been named in a committed review record; 42 have not.** That is a
measurement of the review process rather than of the documents: reviews have not been recording
what they read, which is exactly why the requirement is now in the template.

## Verification Commands

```text
python3 scripts/process_metrics.py generate -> the readership section renders
python3 scripts/process_metrics.py check    -> the committed report matches
python3 scripts/check_docs.py check         -> 274 Markdown files consistent
```

## Documentation updated

- `project/templates/VERIFIER_PROMPT.md`, `.claude/skills/epic-verifier/SKILL.md`,
  `project/generated/PROCESS_METRICS.md`.

## Residual risks

- **Never named is not never read**, and the report says so twice. The first list is mostly an
  artefact of reviews not having been asked to record this until now; it will only become
  informative after several rounds under the new template.
- **A review could name a document it did not open.** Nothing verifies the claim, and nothing
  could.
- **The exemptions are a judgement.** Evidence packets, generated files and ADRs are excluded
  because they are addressed by id or by location rather than by being read; a document wrongly in
  that set becomes invisible to this measurement.
- **Nothing schedules the once-a-phase decision.** It is stated in the story and in the report, and
  no gate enforces the cadence.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.
