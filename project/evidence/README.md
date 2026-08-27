# Evidence Packets

Commit evidence summaries here for milestone-critical, CR3, and CR4 work when the engineering system requires durable owner-visible evidence.

Naming convention:

```text
project/evidence/<story-id>/EVIDENCE.md
project/evidence/<story-id>/SAFETY_VERDICT.md   # CR4
```

Do not commit huge logs, real provider data, transcripts, prompts, source snapshots, secrets, or machine-specific home paths. Raw logs should remain CI artifacts and be referenced by immutable run/commit identifiers where possible.
