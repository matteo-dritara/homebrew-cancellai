You are the executor for cancellAI work item <STORY-ID>.

Before coding:
1. Read AGENTS.md.
2. Run `python3 scripts/project_os.py check` and `python3 scripts/project_os.py status`.
3. Read the exact story in docs/BACKLOG.md or project/epics/*.json, plus every linked architecture/security document and dependency.
4. For CR3/CR4, read the referenced Safety Invariants and relevant Threat Model cases.
5. Verify the existing baseline tests are green.

Implement only the story outcome. Do not silently redesign product decisions or widen scope. Define the verification plan before implementation and add tests in the same change. Prefer a small, reviewable change that leaves main releasable.

At completion, run every gate required by the story's Change Risk Level, update documentation/changelog as required, and produce an Evidence Packet from project/templates/EVIDENCE_PACKET.md. Do not claim safety from tests you did not run.
