Review-Scope: epic
Verifier: Claude (self-review, not independent)

# E34 self-review round 2 (SELF-REVIEW, not independent)

- Review target: `main` at HEAD `57b1a1b` (`feat(process): sandbox the OpenCode reviewer and repair the E34 self-review and round-3 findings`).
- Reviewer: Claude (Opus 5.5), forked context. This is the executor's own family; it is a pre-handover self-attack, **not** an independent verifier round, and does not count as one or move any status.
- Date: 2026-09-24.
- Scope: E34-S06 (new OS sandbox) and the post-round-3 repairs carried by the same commit to E34-S01 (run record beside the worktree; process-group kill before the check) and E34-S02 (permission map denies every unnamed category first; write-capable and background-job commands removed). E34-S03/S04/S05 passed independent round 3 and are out of scope. Owner decided 2026-09-24 to close E34 on these repairs plus this self-review, residuals accepted.

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E34-S06 | PASS | Independent `sandbox-exec` probes with the harness's own `rr.sandboxed()` against a throwaway `git worktree` (below) confirm every AC: writes land only in the worktree, its per-worktree git admin dir and OpenCode's data/state/cache; the main tree, the shared object store/refs/hooks, the run record, OpenCode's config dir, `~/.cargo`, `/tmp`, `/var/tmp` and home dotfiles all fail with `Operation not permitted`. Symlink and setsid-escape write-through are both blocked. `sandboxed()` raises off macOS. `tests/test_review_round.py` 32 passed; `tests/test_opencode_agents.py` 8 passed / 170 subtests. |
| E34-S01 | PASS_WITH_RESIDUALS | `run_file()` places the record at `<worktree>.review-run.json` in the worktree's parent, outside the sandbox's writable set, so the reviewer cannot forge it during the run (probe: `RUNFILE` write denied). `import_problems` re-runs every check and re-digests the live worktree, so a record edited or a tree scribbled after the check is refused (`test_a_record_edited_after_the_check_is_refused`, `test_a_tampered_run_file_is_rechecked`). `run_in_own_group` kills the leader's process group. Residual: a child that calls `setsid` survives the group kill (reproduced below) — but it stays inside the sandbox (its post-exit write to the main tree was blocked) and any worktree change it makes trips the import re-digest, so no bad bytes can be imported. Best-effort cleanup layered under two hard gates; accepted. |
| E34-S02 | PASS_WITH_RESIDUALS | Both agents now begin `"*": deny` (verified structurally by `test_every_unnamed_permission_category_is_denied_first`, and against OpenCode's own engine by `test_opencode_itself_denies_what_the_file_does_not_name`: `some_future_tool`/`webfetch`/`task` → deny, `read` → allow). Write-capable commands (`touch`, `mkdir`, `mktemp`, `chmod`, `sed -n`, `sort`, `uniq`) removed; `*&*`/`*--output*`/`*-fls*`/`*-fprint*`/`*-ok *` denied. Residual by design (and by the AC's own words): a command-pattern denylist is not the boundary — an allowed `pytest` runs arbitrary reviewer code — so containment rests on E34-S06's sandbox + the E34-S01 path check + import, all exercised here. Accepted. |

## Attack reproductions

Throwaway `main` repo + linked `review` worktree under the session scratchpad; every command run through `scripts/review_round.py`'s own `sandboxed()`.

### E34-S06 write confinement (each `sh -c` under `rr.sandboxed(...)`)

| Target | Result |
| --- | --- |
| `<worktree>/inside.txt` | WROTE (allowed) |
| `<worktree>` git admin dir (`.git/worktrees/review/…`) | WROTE (allowed — the GITDIR param) |
| `~/.local/share/opencode/probe` | WROTE (allowed — data dir) |
| `<main>/README.md` (main tree) | Operation not permitted |
| `<main>/.git/objects`, `/.git/refs`, `/.git/hooks/post-checkout` | Operation not permitted (shared store and hook-plant blocked) |
| `<worktree>.review-run.json` (run record) | Operation not permitted |
| `~/.config/opencode/probe` (config dir) | Operation not permitted |
| `~/.cargo/probe`, `/tmp/…`, `/var/tmp/…`, `~/.gitconfig.sbprobe` | Operation not permitted |
| symlink `worktree/link_out -> main`, then write through it | Operation not permitted (target canonicalised) |
| symlink to `main/.git/hooks`, then write `pre-commit` | Operation not permitted |

Mutation confirmation: `git commit` inside the sandboxed worktree fails (`impossibile creare il file temporaneo: Operation not permitted`) because the shared object store is not writable — benign, since import copies files and never commits from the worktree.

### E34-S01 process-group escape

`run_in_own_group` on a reviewer that `os.fork()`s a child which calls `os.setsid()`: the child **survived** the leader's `killpg` (`escaped grandchild survived killpg: True`). Its attempt to append to `<main>/README.md` after the leader exited returned `BLOCKED: Operation not permitted`, and `README.md` was intact. Conclusion: the group kill is not a hard guarantee against a detached child, but the sandbox contains such a child to the worktree/git-admin/oc-dirs and the import re-digest rejects any worktree bytes it changes — so the layered defence holds; recorded as an accepted residual.

## Additional observations (not findings)

- The reviewer can edit `.opencode/agents/*.md` inside its own worktree, but `.opencode/**` is absent from `FORMAL_ALLOWED`, so any such change is refused at import as "outside what this tier may change"; and the main tree's `.opencode/` is not sandbox-writable, so no later session is affected.
- OpenCode's data dir is writable and holds the OpenRouter credential; a reviewer could overwrite it (breaks the next review — fail-closed) but cannot read anything it could not already read. The config dir, where a global agent/plugin/`opencode.json` would persist into a later session, is not writable — the persistence vector that matters is closed. Both already listed in `project/evidence/E34-S06/EVIDENCE.md` residuals.

## Gate commands actually run

- `python3 -m pytest tests/test_review_round.py -q` → 32 passed
- `python3 -m pytest tests/test_opencode_agents.py -q` → 8 passed, 170 subtests passed
- Independent `sandbox-exec` probes via `rr.sandboxed()` against a throwaway worktree (tables above)

## Documents opened (by path)

- `AGENTS.md`, `CLAUDE.md`
- `.claude/skills/epic-verifier/SKILL.md` (skill body, this run)
- `scripts/review_round.py`
- `tests/test_review_round.py`
- `tests/test_opencode_agents.py`
- `.opencode/agents/verifier.md`, `.opencode/agents/pre-reviewer.md`
- `project/epics/E34.json`
- `project/evidence/E34-S06/EVIDENCE.md`
- `docs/development/AGENT_PROTOCOL.md` (diff at HEAD)
- Verifier briefs E34-S06 / E34-S01 / E34-S02 (via `project_os.py brief`)

## Round verdict

All three stories in scope: PASS / PASS_WITH_RESIDUALS, no FAIL. Residuals are the accepted ones (setsid survival mitigated by sandbox + import re-digest; permission denylist is defence-in-depth, not the boundary; network open; macOS-only; OpenCode data dir writable). This self-review is not an independent verifier round and is recorded as `E34-SELF-REVIEW-ROUND2.md`; the owner's 2026-09-24 decision to close E34 on these repairs plus this self-review stands on that basis.
