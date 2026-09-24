# Evidence Packet - E34-S02

- Commit/PR: the E34 commit on `main`
- Executor: Claude
- Independent verifier: round 1 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND1.md); round 2 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND2.md); round 3 FAIL (Codex, E34-VERIFIER-REVIEW-ROUND3.md); repaired
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

## Repairs from the configuration audit (2026-09-24)

| Finding | Repair |
| --- | --- |
| Language servers are downloaded and run by default; formatters rewrite edited files | `opencode.json`: `"lsp": false`, `"formatter": false`; the `lsp` permission removed from both agents |
| Any configured provider could serve a session | `"enabled_providers": ["openrouter"]` |
| `gh` was allowed but the harness now gives it no credentials | `gh *` denied in both agents |
| Personal skills from `~/.claude` loaded into the reviewer | `"skills": {"paths": [".claude/skills"]}` with the harness disabling `.claude` loading (E34-S01) |

## Repairs after independent round 1 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND1.md` (Codex) failed AC1: `"python3 *": allow` re-admitted what the direct
denials refused - `python3 -m pip install`, `python3 -c "os.remove(...)"`, `python3 -c "urllib..."` -
and `cargo *` admitted arbitrary cargo subcommands.

| Repair | Evidence |
| --- | --- |
| Both agents allow only named commands: `python3 -m pytest *`, `python3 scripts/*`, `cargo test/check/clippy/fmt --check`, and read-only git and text tools | `.opencode/agents/verifier.md`, `pre-reviewer.md` |
| The shell rules end with denials that OpenCode, applying the last matching rule, puts above every allow: `-c`, `..`, `;`, `&&`, `\|\|`, backticks, `$(`, `>`, piping into a shell or Python, `pip`, `install`, `curl`, `wget`, `http`, `urllib`, `requests`, `socket`, `rm `, `rmtree`, `remove`, `unlink`, `find -delete/-exec` | `tests/test_opencode_agents.py` evaluates every committed rule with last-match-wins over 21 denied commands (round 1's three included) and 8 needed ones, for both agents; against the pre-repair agents 22 cases fail |
| OpenCode loads the result as intended | `opencode debug agent verifier` (1.18.30): 65 ordered bash rules, first `*: deny`, last `*-exec*: deny`; model `openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` |

**Residual, stated rather than claimed away:** a formal-tier reviewer may write an adversarial test
under `tests/` and `python3 -m pytest` will run it; that test is arbitrary Python, which no
command-level permission can bound. What bounds it is the review worktree, the reviewer
environment (no credentials, no push URL) and the harness's path check and import (E34-S01). The
AC's "deny package installation, file removal and web access" is met for every command the agent
issues, not for code a test it wrote executes. Put to the owner on 2026-09-24, who chose to narrow AC1 to say exactly this: the permissions deny
those operations for every command, including through an allowed interpreter or test runner, and
code run by a test the reviewer wrote is bounded by the worktree and the harness (E34-S01).

## Repairs after independent round 2 (2026-09-24)

`E34-VERIFIER-REVIEW-ROUND2.md` (Codex) failed the narrowed AC1: `python3 scripts/*` admitted
`check_agent_toolchain.py updates`, which calls `gh api`.

| Repair | Evidence |
| --- | --- |
| `python3 scripts/*` is replaced by exact, reviewed script/subcommand pairs; `check_platforms.py check` (probes CI through `gh`), `check_agent_toolchain.py updates` and every `release.py` command but `check` are left out, so denied | `test_every_allowed_script_is_an_exact_reviewed_subcommand`: every allowed script rule has no wildcard, and a script containing a network marker (`urllib.request`, `which("gh")`, `"curl"`, `socket.`, `http.client`) must be listed with the reason its allowed subcommand never reaches it |
| Every cargo a reviewer runs - directly, or inside an allowed script such as `check_provider_compatibility.py check` - is offline: the harness sets `CARGO_NET_OFFLINE=true`, and `--config`, `net.offline` and `CARGO_NET` overrides are denied | `test_cargo_runs_offline_in_the_reviewer_environment`; `DENIED` gains round 2's command and seven more |

## Repairs after the forked self-review (`E34-SELF-REVIEW.md`, 2026-09-24)

The self-review showed, through OpenCode's own permission engine, that allowed read commands could
write: `sed -n w<path>` moved another repository's `refs/heads/main` back one commit and truncated a
file, and `sort -o`, `uniq in out` and `git diff --output=` wrote outside the worktree.

| Repair | Evidence |
| --- | --- |
| `sed -n`, `sort`, `uniq`, `mkdir`, `touch`, `chmod` and `mktemp` are no longer allowed (`cat`/`head`/`tail`/`grep` and the read tool cover reading); `--output`, `--pre` (`rg` preprocessor), `find -fprint/-fls/-ok` and any `&` (background jobs) are denied last | `DENIED` gains the self-review's probes and six more; `ALLOWED` reads with `head` |
| What no command rule can bound - code a reviewer-written test runs - is confined by the OS sandbox (E34-S06) | E34-S06 |

## Round 3 (Codex, `E34-VERIFIER-REVIEW-ROUND3.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| OpenCode's own defaults begin with `"*": allow`, so a permission category neither agent names resolved to allow; the bash-only test could not see it | Each agent's `permission` map now starts with `"*": deny`, applied after OpenCode's defaults and before every named rule | `test_every_unnamed_permission_category_is_denied_first`; `test_opencode_itself_denies_what_the_file_does_not_name` asks OpenCode 1.18.30's engine (`opencode debug agent`) and gets deny for an unnamed category, `webfetch` and `task`, allow for `read`; both fail against the round-3 agents |

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
