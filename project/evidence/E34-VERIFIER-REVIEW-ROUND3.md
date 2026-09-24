Review-Scope: epic
Round: 3
Verifier: Codex
Date: 2026-09-24
Review-Target: 34d2919575a731e4dd17489b050e7327fa6640c3..398f1401dcf6d8d2e2902f312af10d45cc5e03ca
Round-3-Repair-Range: 7e35a2aa2c9eaaa1a7402a21368646195f9fb732..398f1401dcf6d8d2e2902f312af10d45cc5e03ca

# E34 independent verifier review, round 3

All five stories were `ready_for_review` at the start. I reviewed their committed briefs, contracts and final diff, then used disposable repositories and synthetic evidence/configuration trees to seek counterexamples. No E34 story is CR4, so no Safety Verdict is due.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E34-S01 | FAIL | In a disposable Git worktree, an OpenCode log naming `anthropic/claude-opus-5` was rejected by `stream_problems`, but a rewritten `.review-run.json` claiming `reviewer: codex` and matching digests made `import_problems` return `[]`; `cmd_import` returned 0 and imported the forged record. Brief-Checksum: bdd4184cfe0cb67b89f90f7dd271af87928b2c7075da6d0dc15d6de36a559daa |
| E34-S02 | FAIL | `opencode debug agent verifier` and `pre-reviewer` each parsed a global `{permission: "*", action: "allow", pattern: "*"}` rule. An unnamed permission category resolves to `allow` for both; an explicit `git commit` resolves to `deny`. The committed bash-only test does not examine the global rule. Brief-Checksum: 5e918fad438f54e004dbca7c33dedb69e3e699fddccb9db97126c70add803454 |
| E34-S03 | PASS | A synthetic evidence tree with one advisory `FAIL` and three formal records produced three rounds and one advisory record. Reviewer families were `Codex`, `OpenCode/y`, and `unnamed`; the advisory finding entered no round. Brief-Checksum: bd7b2ae65c48a06e3c3a8221bc71a721d401ed05a5f2f3409b8cc5fed654f18a |
| E34-S04 | PASS | A synthetic `.opencode/agents` tree reported a known agent, an unrecognised hidden file and nested directory; omitted `lsp` and `formatter` produced gate components. The committed repository's toolchain gate passed. Brief-Checksum: 4eef8d625dfc4c40905296b5434dc66a63a729ff4896e09fe7f6741c716d3d8a |
| E34-S05 | PASS | Re-rendering A, B, A, B retained three distinct superseded files, including both byte-distinct A headers. An old verdict passed against its archive; an unknown checksum and a tampered archive were each rejected. The committed handoff gate passed. Brief-Checksum: 4b9fbf7d73e21b96b246696bccd4e73eeb6b623cd3fec902aee66adc2391387e |

## E34-S01: the import receipt can replace model attribution

I committed a base in a disposable repository, added a review worktree, and put a formal review record and an OpenCode stream log in it. The log contained `message=stream providerID=anthropic modelID=claude-opus-5`; `stream_problems` called it a self-review. I then wrote the worktree's `.review-run.json` with `reviewer: codex`, `label: Codex`, `problems: []`, and recomputed digests. `import_problems` returned no errors. With that file, `cmd_import` returned 0 and copied the record into the main tree. The receipt is in the review worktree, which the harness itself says is writable by the reviewer, and `import` trusts its reviewer identity. The new digest check protects file bytes only against an unchanged receipt.

This violates AC3: an Anthropic-streamed session's record remains importable after its attribution is replaced. It also violates AC5's requirement that only a round that passed its checks be imported. Required repair: keep the run identity and pass/fail result outside reviewer-writable state, or authenticate the receipt against a trusted run invocation; at import, derive the reviewer and declared model from that trusted state before checking the stream log. Add a regression that changes `reviewer` and `label` in the receipt after a failed OpenCode run. E34-S01 remains the carry-forward work item; this finding is not accepted as residual risk.

## E34-S02: an unlisted permission is allowed

With OpenCode's data/configuration directories redirected into `/tmp`, `opencode debug agent verifier` and `opencode debug agent pre-reviewer` loaded the committed agent definitions. Both parsed a global `*` permission rule whose action is `allow`. Applying the parsed rule list in order produced `allow` for an unlisted permission category and `deny` for `bash: git commit -m x` and `bash: python3 scripts/check_agent_toolchain.py updates`. The specific shell repair works for these commands, but the default permission rule is permissive. `tests/test_opencode_agents.py` starts its simulator from `ask` and evaluates only bash rules, so it cannot detect this runtime default.

This violates AC3: a permission the agents do not explicitly allow is allowed by the runtime's global rule. Required repair: configure an effective global default-deny rule for both agents, confirm the parsed OpenCode permission lists end with that default for unnamed categories without blocking intentional grants, and add a regression against the parsed configuration rather than only the YAML bash map. E34-S02 remains the carry-forward work item; this finding is not accepted as residual risk. I could inspect the live configuration but could not run a credentialed OpenRouter model smoke test here.

## Gates and limits

- `python3 scripts/project_os.py check`, `status`, `next`, `review`, and `brief <ID> --role verifier` for all five stories: passed at session start.
- `python3 scripts/check_agent_toolchain.py report`: passed with no expired decision.
- `python3 -m pytest tests -q`: first run had 776 passed, 3 skipped and one documentation-link failure caused by the harness's untracked `.opencode-run/prompt.md`. With that directory temporarily parked and restored afterward, the full suite passed: 777 passed, 3 skipped, 827 subtests.
- `pre-commit run --all-files`: passed twice with the same untracked harness directory temporarily parked, including after the verdict/status files were written; this ran ruff, mypy, documentation, governance, handoff, process, Python parity, release and related Python gates.
- `opencode debug agent verifier` and `opencode debug agent pre-reviewer`: parsed both committed agent files with temporary XDG directories; their output showed the global allow rule described above.
- `gh run list --branch main --limit 5`: unavailable because GitHub CLI has no authentication. Main CI is unknown, not green.
- No E34 change touched Rust production paths, so the Rust workspace quality matrix is outside this story scope. No live OpenRouter smoke was available in this session.

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`; `docs/development/RELEASE_GATES.md`; `docs/development/AGENT_TOOLCHAIN.md`; `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`; `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`; `project/epics/E34.json`; `project/decisions.json`; `project/evidence/E34-VERIFIER-REVIEW-ROUND1.md`; `project/evidence/E34-VERIFIER-REVIEW-ROUND2.md`; `project/evidence/E34-S01/VERIFIER_BRIEF.md`; `project/evidence/E34-S02/VERIFIER_BRIEF.md`; `project/evidence/E34-S03/VERIFIER_BRIEF.md`; `project/evidence/E34-S04/VERIFIER_BRIEF.md`; `project/evidence/E34-S05/VERIFIER_BRIEF.md`.

## Overall verdict

FAIL: two of five stories have reproduced defects, a 40% finding yield. S01 and S02 overlap both earlier rounds' findings, so the zero-overlap escalation does not apply. S01 and S02 return to `in_progress`; passing S03 is `blocked` by S01 and passing S04 is `blocked` by S02; S05 is `done`. The epic remains `in_progress`. Round 3 reaches ADR-0025's cost ceiling while yield remains above 10%, so closure requires an owner decision after the two carry-forward repairs are made. Earlier ceiling decisions for S01 and S02 did not accept these new findings. This verifier changed no production code and made no commit.
