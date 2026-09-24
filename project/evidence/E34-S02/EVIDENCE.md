# Evidence Packet - E34-S02

- Commit/PR: the E34 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR1
- Spec version/commit: `project/epics/E34.json` at this commit; PD-028

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - verifier/pre-reviewer deny commit, push, tag, reset, installs, removal and web | `.opencode/agents/verifier.md`, `.opencode/agents/pre-reviewer.md`: `bash` is `"*": deny` with an allow-list of read/test commands, and `git commit/push/tag/reset/checkout/worktree`, `rm`, `brew`, `curl`, `pip` explicitly denied; `webfetch`, `websearch`, `external_directory`, `task`, `question` denied; edit limited to review paths (pre-reviewer: its own `*-PRE-REVIEW-*.md` only). `opencode agent list` loads both. | PASS |
| AC2 - one pinned model for every agent and the small model | `opencode.json` pins `model` and `small_model` to `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free`, both agents pin the same model; the harness's stream check (E34-S01 AC3) catches a session that streamed another one. The smoke run showed the default small model (a second family) before pinning - the reason for this AC. | PASS |
| AC3 - a permission not explicitly allowed is denied, not prompted | Every permission map starts from `"*": deny`, `doom_loop: deny`, `question: deny`; `opencode.json` denies edit/bash/web by default for any other agent. `share: disabled`, `autoupdate: false`. | PASS |

## Verification Commands

```text
opencode agent list   -> verifier, pre-reviewer listed with the pinned model
```

## Residual risks

- **OpenCode's permission engine is the enforcement**, not this repository; the harness's path
  check (E34-S01 AC4) is the independent second barrier on what a reviewer changed.
- **The OpenRouter credential lives outside the repository** (`~/.local/share/opencode/auth.json`,
  mode 600) and is never read by these files.
- **Free-tier limits and 503s** make a free-model run unreliable; that is availability, not safety.

## Verifier verdict

pending
