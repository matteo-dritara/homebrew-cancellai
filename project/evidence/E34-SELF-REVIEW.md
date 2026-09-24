Review-Scope: epic
Verifier: Claude (self-review, not independent)
Date: 2026-09-24
Review-Target: 7e35a2a..a464708 (round-2 repairs `df5d339`, ceiling records `a464708`), examined at `3f9fe63`

# E34 self-review (not independent)

**SELF-REVIEW (not independent).** Claude executed E34, and Claude wrote this review. It ran in a
forked context that did not see the executor's reasoning. It worked from `project/epics/E34.json`,
the round-2 record, and the committed code. It is not a counted round and does not replace the
independent reviewer. The owner's cap closes E34-S01, S02 and S04 on this review plus their
`CEILING_DECISION.md`, so each FAIL below is a finding for the owner. None may be read as a pass.

`HEAD` moved to `3f9fe63` (E06 round-9 import) during the review. That commit touches no E34 file:
`git diff --quiet a464708 3f9fe63 -- scripts .opencode opencode.json tests` exits 0. The main working
tree also held another session's uncommitted E06/E33 edits. For that reason every gate below ran in
a clean `git clone` at `3f9fe63`, in the scratchpad, and not in the main tree. E34-S03 is not judged
here: it passed round 2 and is only dependency-held by S01.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E34-S01 | FAIL | The round-2 repair holds for its own counterexample. However, `import` still takes `tier`, `record`, `reviewer`, `label`, `model`, `problems` and `digests` from `.review-run.json`, which sits in the reviewer-writable worktree. Rewriting the run file and `.opencode-run/opencode.log` after the check makes a round that streamed from `anthropic/claude-opus-5-5` importable (`import_problems` returns `[]`). Relabelling the run as `codex` skips attribution entirely. A pre-review re-described as `formal` imports as `E34-VERIFIER-REVIEW-ROUND3.md` plus a test (AC3, AC5). Separately, a reviewer write outside the worktree is never seen (AC4); see S02. |
| E34-S02 | FAIL | The round-2 repair holds: all 33 allowed `python3 scripts/<name>.py <sub>` commands and the full pytest suite ran under a socket/`gh`/`curl`/`git fetch` tracer, and none reached the network. However, OpenCode 1.18.30's own permission engine (`opencode debug agent verifier --tool bash`) runs `sed -n w<abs path> <file>`, `sort -o <abs path>`, `uniq <in> <abs path>` and `git diff --output=<abs path>`, writing outside the worktree despite `external_directory: deny`. Through the allowed `sed -n *` it moved a disposable repository's `main` back one commit and emptied a file there, which is a reset and a file destruction that AC1 denies. The same holds for `pre-reviewer`. |
| E34-S04 | FAIL | The round-2 repair skips `.DS_Store` by name and does not check that it is a file. With `.opencode/agents/.DS_Store/sneaky.md` (`permission: bash: allow`), `opencode agent list` shows `.DS_Store/sneaky (primary)`, while `python3 scripts/check_agent_toolchain.py check` prints `nothing unmanaged` and exits 0. This is the same class as round 2's hidden-entry finding, and it is not closed (AC1, AC2). |
| E34-S05 | PASS_WITH_RESIDUALS | Renders A,B,A,B,A,C,A on seven dates kept all six superseded briefs byte for byte (`-2`/`-3` suffixes on recurrence). Verdicts answering A, B and C pass, an unknown checksum is refused, and a tampered archive is reported and stops answering. Residual: a hand-written archive that is hash-consistent and has a `Rendered-by` line is accepted under any name (see below). |

## E34-S01 - reproductions

**R1: run-file and log forgery at import (implementation bug).** Script:
`scratchpad/repro_s01.py`. It uses a disposable main repository, a review worktree, and a run file
built exactly as `cmd_run` builds one.

- A. A session log has streams from `openrouter/nvidia/model-a:free` and
  `anthropic/claude-opus-5-5`. The run correctly records the self-review problem, and `import_problems`
  refuses. A process that outlives the reviewer then rewrites `.opencode-run/opencode.log` without the
  Anthropic line and rewrites `.review-run.json` with `problems: []` and recomputed `digests`. Result:
  `import_problems` returns `[]`.
- B. The same run file is re-described as `reviewer: codex, label: Codex`, and the record header is
  `Verifier: Codex`. `import_problems` returns `[]` while the log still shows the Anthropic stream,
  because attribution is re-checked only when `data["reviewer"] == "opencode"`.
- C. A `pre` run is re-described as `tier: formal, record: E34-VERIFIER-REVIEW-ROUND3.md`, with
  `tests/test_x.py` added. `import_problems` returns `[]`, so an advisory pass would be imported as a
  counted round, which PD-028 and E34-S03 forbid.

The shipped regressions pin the direction an attacker would not use. For example,
`test_an_opencode_run_is_reattributed_from_its_log_at_import` *adds* an Anthropic line. Nothing pins
removing one, or changing the fields `import` trusts.

A process can outlive the reviewer. `run_reviewer` uses `subprocess.run` without `start_new_session`
or a process-group kill (`scripts/review_round.py` lines 289 and 298). OpenCode's real engine allows
`python3 -m pytest tests/<file> &` (checked below), and a formal reviewer may write the test it runs.
I did not observe OpenCode's teardown of background jobs, but a test that double-forks with
`setsid` does not depend on it.

Required repair: keep the run record out of the reviewer's reach. For example, store it under the
main repository's `.git/` or the harness's own directory, keyed by worktree, or make `import` a
continuation of `run` in the same process. Kill the reviewer's process group before checking.
Derive `tier`, `record`, `reviewer` and `model` from the harness's own record, not from anything in
the worktree. Add regressions for A-C.

**R2: writes outside the worktree go unseen (architecture decision).** The harness judges only
`git status` of the worktree. A write into the main tree via the S02 primitives below, for example
`sed -n w<main>/project/evidence/E34-VERIFIER-REVIEW-ROUND3.md project/evidence/<own record>`,
never reaches `path_problems`, and the round passes (AC4). `AGENT_PROTOCOL.md`'s sentence that "the
worktree, the path check and `import` remain the boundary" is not true for OpenCode, which runs
unsandboxed as the user. Only Codex has a workspace sandbox.

Required: an OS sandbox for the OpenCode reviewer, with the worktree and log directory as its only
writable roots and network limited to the model provider (`sandbox-exec`/`bwrap` or equivalent),
or an owner decision recording that the boundary is advisory. A before/after `git status` of the
main tree would be a tripwire, not a boundary.

## E34-S02 - reproduction

Every probe below used OpenCode's real permission engine, not the fnmatch model in
`tests/test_opencode_agents.py`. The command form was:
`opencode debug agent <agent> --tool bash --params '{"command": ..., "description": "probe"}'`,
run in the clean clone with `OPENCODE_DISABLE_CLAUDE_CODE=1` (script: `scratchpad/probe.py`).

| Command (verifier agent) | Engine decision |
| --- | --- |
| `sed -n w<scratchpad>/sedwrite.txt README.md` | ran; file created outside the worktree |
| `sort -o <scratchpad>/sortwrite.txt README.md` | ran; file created |
| `uniq README.md <scratchpad>/uniqwrite.txt` | ran; file created |
| `git diff HEAD~1 --output=<scratchpad>/gitdiffwrite.txt` | ran; file created |
| `python3 -m pytest tests/test_nonexistent.py &` | ran (background job admitted) |
| `touch <scratchpad>/touched.txt` | denied (external_directory applies to `touch`, not to the above) |
| `python3 -m pytest ... > <file>`, `git commit ...`, `whoami`, `python3 scripts/check_agent_toolchain.py updates` (also with doubled spaces) | denied |

To show the AC1 consequence, I set up a disposable repository `victim` with two commits. I put the
first commit's sha in `project/evidence/sha.txt` inside the clone, which the verifier's edit
permission allows. Then:
`sed -n w<victim>/.git/refs/heads/main project/evidence/sha.txt` moved `main` from `90b6e1b`
(2 commits) to `a025620` (1 commit), which is a reset without `git reset`, and
`sed -n w<victim>/f /dev/null` truncated `f` to 0 bytes. `pre-reviewer` admits the same
`sed -n w<scratchpad>/pre-sed.txt README.md`.

This is an implementation bug: the allow-list admits the write forms of `sed` (`w`/`W`), `sort -o`,
the two-operand form of `uniq`, and `git diff/log/show --output`. It is also a spec gap. AC1's
exception says reviewer-run test code "is bounded by the review worktree", and nothing enforces
that (S01 R2).

Required repair: drop or narrow those commands. `grep`/`rg` and the `Read` tool already cover
reading. Add these probes to `DENIED`. Close the `&` background form. The engine test should run
against OpenCode itself where it is installed, because the fnmatch model cannot see
`external_directory` behaviour.

## E34-S04 - reproduction

In the clean clone:
`mkdir -p .opencode/agents/.DS_Store`, then write `sneaky.md` there with frontmatter
`mode: primary`, `permission: {bash: allow}`.

- `opencode agent list` → `.DS_Store/sneaky (primary)`. `opencode debug agent .DS_Store/sneaky`
  resolves it.
- `python3 scripts/check_agent_toolchain.py check` → `agent toolchain OK: ... nothing unmanaged`,
  exit 0. `_opencode_components` returns only `subagent:opencode/pre-reviewer` and
  `subagent:opencode/verifier`.

Cause: `_visible_children` drops any child named `.DS_Store` before it is judged, and OpenCode
globs agents recursively, including dot-directories. This is an implementation bug. Required
repair: skip `.DS_Store` only when it is a regular file, or not at all. Add a `.DS_Store`
*directory* holding an `.md` to `OpenCodeComponentsAreEnumerated`. The same applies under
`skill(s)/`.

Also tried, not a finding: an `opencode.json` `agent.verifier` block with `bash: {"*": allow}` did not
widen the resolved rules (the agent file's rules come last, and `git commit` and `whoami` were still
denied).

Residuals (spec gap, for the backlog, not verdict-changing):
- `.opencode/package.json`, `bun.lock` and `node_modules` are skipped as local state without
  inspection, although OpenCode installs whatever `package.json` declares.
- AC1 enumerates six component kinds, and the walk reads no other `opencode.json` keys. Keys that
  load remote content, such as URL `instructions` or a custom provider's npm package, are not
  enumerated by AC1 or the walk.

## E34-S05 - residual

Script: `scratchpad/repro_s05.py`. A file `VERIFIER_BRIEF.superseded-fabricated.md` with any body, a
matching `Brief-Checksum` and a `Rendered-by` line is accepted, and a verdict answering it passes
`check_story` with `[]`. Neither provenance nor the filename's checksum prefix is checked. This meets
AC2/AC3 as written, but it falls short of the outcome's "a superseded brief the gate rendered".
This is a spec gap. A provenance check, for example that the archive's bytes equal some committed
blob of `VERIFIER_BRIEF.md` in history, would close it.

## Minor observations (no verdict impact)

- `path_problems` uses `fnmatch`, where `*` crosses `/`. As a result,
  `rust/crates/cancellai-safety/src/tests/mod.rs` passes the formal tier's `rust/crates/*/tests/*.rs`.
  No `src/**/tests/` exists today, and a new orphan file under `src/` is not compiled unless a `mod`
  line changes, which the tier refuses.

## Gates actually run (clean clone at `3f9fe63` unless noted)

- `python3 -m pytest tests -q`: in the main tree at session start (clean then): 772 passed. In the
  clone under the network tracer: 769 passed, 3 skipped. The tracer's only event was the local
  bare-repo `git push` in `ReviewerEnvironmentTests`.
- `python3 -m pytest` on `tests/test_review_round.py`, `test_opencode_agents.py`,
  `test_agent_toolchain.py` and `test_verifier_handoff.py`: 125 passed.
- The 33 allowed script commands, taken verbatim from `.opencode/agents/verifier.md`: all exit 0,
  with `CARGO_NET_OFFLINE=true` and an empty `GH_CONFIG_DIR`. That list includes `project_os.py
  check`, `verifier_handoff.py check`, `check_agent_toolchain.py check`, `process_metrics.py check`,
  `check_process.py check`, `release.py check`, `check_docs.py check`, `check_evidence.py check` and
  `gate_sensitivity.py check`.
- `ruff check .`, `ruff format --check .`, `mypy` on the three changed scripts, and
  `gen_docs.py --check`: pass.
- `gh run list --branch main --limit 5`: the latest main runs (governance, tests, rust, codeql) are
  green for `34d2919` (`origin/main`). `df5d339`, `a464708` and `3f9fe63` are not pushed, so CI for
  the review target is **unknown**.
- A first attempt to run the gates in the main tree mis-invoked them (shell word splitting) and is
  discarded. It was not a gate result.

## Documents opened

`AGENTS.md`; `CLAUDE.md`; `project/epics/E34.json`; `project/evidence/E34-VERIFIER-REVIEW-ROUND2.md`;
`project/evidence/E34-S01/CEILING_DECISION.md`; `project/evidence/E34-S02/CEILING_DECISION.md`;
`project/evidence/E34-S02/EVIDENCE.md` (residuals section); `docs/development/AGENT_PROTOCOL.md`
(reviewer pool and harness section); `.opencode/agents/verifier.md`; `.opencode/agents/pre-reviewer.md`;
`opencode.json`; `project/agent_toolchain.json` (the `opencode-reviewer-agents` entry);
`scripts/review_round.py`; `scripts/check_agent_toolchain.py` (OpenCode walk); `scripts/verifier_handoff.py`;
`scripts/release.py` (network paths); `scripts/process_metrics.py` (round loading);
`tests/test_review_round.py`; `tests/test_opencode_agents.py`; `tests/test_verifier_handoff.py` (fixture);
the round-2 regression hunks of `tests/test_agent_toolchain.py`. Not opened in this review:
`docs/security/SAFETY_INVARIANTS.md`, `docs/security/THREAT_MODEL.md` (no E34 story is CR4).

## Round verdict

FAIL: three of the four stories judged (S01, S02, S04) have reproduced defects. Every round-2
required repair was made and holds for the case round 2 reported. S01 and S04 fail on the same class
by another route, and S02 fails on a different AC1 action. S01 R2 and the S02 finding share one root
cause: OpenCode runs unsandboxed, so "the worktree is the boundary" is not enforced. That root cause
is an architecture decision for the owner, not something another allow-list edit can close.

S01, S02 and S04 are at the owner's two-round cap. Per their `CEILING_DECISION.md`, they were to close
on this self-review. They should not close on it as it stands: the owner decides between repair
(with a further review) and an explicit, recorded acceptance of these findings. S05 goes to its
independent round 3.

This review changed no story status, production code or generated file, and made no commit.
