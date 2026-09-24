# Evidence Packet - E34-S06

- Commit/PR: the E34-S06 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR2
- Spec version/commit: `project/epics/E34.json` at this commit; owner decision 2026-09-24 (OS sandbox for the OpenCode reviewer)

## Outcome

PASS

## Why this story exists

`E34-SELF-REVIEW.md` showed that OpenCode, unlike Codex, runs with no sandbox as the owner's user:
an allowed command or a test the reviewer wrote could write the main working tree, the run record,
or another repository's `.git`, and the harness - which judges only the worktree's diff - could not
see it. `AGENT_PROTOCOL.md` said the worktree was the boundary; for OpenCode it was not. The owner
chose an OS sandbox over recording the boundary as advisory.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - OpenCode runs under an OS sandbox that allows writes only to the worktree, its git administrative directory and OpenCode's own data, state and cache | `scripts/review_round.py`: `SANDBOX_PROFILE` / `sandboxed`, applied to every OpenCode run; `TMPDIR` points inside the log directory. `test_the_sandbox_confines_writes_to_the_worktree`: inside the sandbox a shell writes the worktree and runs `git status`, and fails to write the main tree or the run record | PASS |
| AC2 - OpenCode's configuration directory is not writable | `test_opencode_configuration_is_not_writable` | PASS |
| AC3 - with no sandbox available the review is refused | `sandboxed` raises unless `sys.platform == "darwin"` and `sandbox-exec` exists; `test_no_sandbox_means_no_opencode_review` | PASS |
| AC4 - OpenCode still completes a session under it | Smoke run 2026-09-24 on a throwaway worktree: `opencode agent list` lists both reviewer agents; `opencode run --agent pre-reviewer` on `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` answered `OK`, exit 0, one model in the stream log | PASS |

Mutation check: replacing the profile's `(deny file-write*)` with an allow fails both sandbox tests.

## Verification Commands

```text
python3 -m pytest tests/test_review_round.py -q   -> 32 passed
```

## Residual risks

- **The network stays open** inside the sandbox: the model provider is reached through it, and
  `sandbox-exec` filters by address, not host name. The agents' denials and `CARGO_NET_OFFLINE`
  (E34-S02) are what keep a reviewer's commands off the network; code a test runs could reach it.
- **macOS only.** On Linux or Windows an OpenCode review is refused; `bwrap`/AppContainer would be
  a later story.
- **`sandbox-exec` is deprecated by Apple** but still ships and is what Codex itself relies on.
- **The sandbox tests skip inside another sandbox** (Codex's refuses a nested one), so an
  independent Codex round cannot run them; the smoke run and this host's suite are the evidence.
- **OpenCode's data directory is writable**, because its session store lives there; it also holds
  the OpenRouter credential. A reviewer could overwrite that credential - which breaks the next
  review, fail closed - but not read anything it could not already read.

## Verifier verdict

pending
