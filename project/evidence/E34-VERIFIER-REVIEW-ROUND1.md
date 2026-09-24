Review-Scope: epic
Round: 1
Verifier: Codex
Date: 2026-09-24
Review-Target: ccd56ff6649532a190e636a074dda5ab4abccdd2..dc50a95020831e9dde6b960a27669710eadfe72f

# E34 independent verifier review, round 1

All four stories were ready_for_review at the start. I derived the checks from project/epics/E34.json and the committed briefs, inspected the final diff, and used disposable repositories and synthetic configurations for counterexamples. No CR4 story is in this epic, so no Safety Verdict is due.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E34-S01 | FAIL | A staged rename from scripts/protected.py to tests/test_new.py produces only tests/test_new.py from changed_paths; path_problems returns no issue. A passing run record can later import over an existing review record. Brief-Checksum: bdd4184cfe0cb67b89f90f7dd271af87928b2c7075da6d0dc15d6de36a559daa |
| E34-S02 | FAIL | Both agents allow the shell pattern python3 *, which admits python3 -m pip install, Python file removal, and Python network requests despite direct-command denies. Brief-Checksum: f67a2ee4c768ed1a25d97575c01930ad7fc2778279284d1d5ceded8363618012 |
| E34-S03 | PASS | A disposable evidence tree containing a pre-review and three formal records yielded only the three formal rounds; reviewer rows were Codex, OpenCode/google/y, and unnamed, while the pre-review appeared only in ADVISORY. Brief-Checksum: bd7b2ae65c48a06e3c3a8221bc71a721d401ed05a5f2f3409b8cc5fed654f18a |
| E34-S04 | FAIL | In a synthetic .opencode tree, agents/known.md is reported but agents/unknown/ and skills/rogue.txt vanish entirely; no unrecognised component is reported for either entry. Brief-Checksum: 4eef8d625dfc4c40905296b5434dc66a63a729ff4896e09fe7f6741c716d3d8a |

## E34-S01: failed boundary checks

In a disposable Git repository, I committed scripts/protected.py, then staged a rename with git mv to tests/test_new.py. Git status --short returned:

    R  scripts/protected.py -> tests/test_new.py

The harness changed_paths returned only tests/test_new.py, and path_problems returned an empty list for a formal round. The production-side source deletion is therefore invisible to the tier check. This violates AC4, which requires refusal when any changed path is outside the tier's allowed set. The test suite checks additions to scripts and tests separately but has no rename-source case.

I also created a disposable reviewed worktree with an allowed new round record and a passing .review-run.json. After the run check, I created the same record path in the import target. cmd_import returned success and replaced the target's existing text with the worktree text. This violates AC2's no-overwrite rule and AC5's exact, safe import claim under a run/import race. cmd_import trusts the worktree's writable run metadata and does not revalidate target existence before copying.

Required repair: parse Git status without losing rename sources, and validate both sides of every rename, deletion, and addition. At import, revalidate the approved run metadata and current paths, refuse an already existing record or conflicting target before copying anything, and test both staged renames and an intervening record creation. A pre-review and a formal round must have the same refusal semantics.

The prompt does include committed briefs and refuses a missing one. I did not complete a live OpenCode pre-review: this environment has no OpenRouter API key, and OpenCode's local debug invocation could not write its user log inside the filesystem sandbox. That verification-contract item remains unproven here, independently of the reproduced AC failures.

## E34-S02: failed permission boundary

The two committed agents put a broad python3 * allow after a default shell deny. Matching the agent's own shell patterns accepts all three commands below:

    python3 -m pip install example
    python3 -c "import os; os.remove('file')"
    python3 -c "import urllib.request; urllib.request.urlopen('https://example.com')"

The direct pip *, rm *, curl *, webfetch, and websearch denies do not constrain code executed inside an allowed Python process. cargo * likewise permits build scripts. This violates AC1's denial of package installation, file removal, and web access. The agents do pin the same model as opencode.json's small_model, but that does not repair the permission boundary. The existing smoke evidence only tests a direct commit attempt and does not exercise the allowed interpreter route.

Required repair: remove arbitrary interpreter/build-command grants or execute them within an enforced sandbox that blocks network access, installation destinations, and file removal. Add denied-operation smoke cases through an otherwise allowed command, as well as the direct git commit case. If the intended guarantee is only that prohibited changes cannot be imported, the owner must narrow the story contract and documentation before re-review.

## E34-S03: passed with dependency held

In a disposable evidence tree, a synthetic pre-review file contained a FAIL-shaped table row but load_rounds did not count it. Three formal records were counted and _reviewer_lines reported their named reviewer families separately; a formal record with no Verifier line appeared as unnamed. The pre-review appeared in ADVISORY only. This exercises all three ACs independently of the executor's tests.

S03 is blocked in project state because its dependency S01 failed this round. Its own implementation verdict is PASS; it may be closed after the dependency passes.

## E34-S04: failed unknown-entry handling

I patched the checker's ROOT only for a disposable tree containing .opencode/agents/known.md, .opencode/agents/unknown/ and .opencode/skills/rogue.txt. _opencode_components returned only subagent:opencode/known. It silently ignored the unrecognized nested directory and file inside recognized component directories. This violates AC2's requirement to report an unrecognized .opencode entry rather than ignore it, and undermines AC1's complete census. The existing synthetic test plants an unknown top-level directory, which does get reported, but never tests unknown children of recognized directories.

Required repair: walk each recognized .opencode directory with explicit allowed child shapes and report every other entry as unrecognized. Add synthetic tests for unknown nested files and directories, including an empty directory and a file in skills/.

S04 also depends on failed S02. The project control plane requires a dependency-held story to be blocked, so its status is blocked even though its own verdict is FAIL.

## Gates and limits

- python3 scripts/project_os.py check, status, next, review and each of the four verifier briefs: passed at session start.
- python3 scripts/check_agent_toolchain.py report: passed; no manifest decision was overdue.
- python3 -m pytest tests -q: 738 passed, 3 skipped, 681 subtests passed after temporarily moving the pre-existing, untracked .opencode-run directory out of the documentation scan. The first run failed one docs-link test on .opencode-run/prompt.md's relative link.
- pre-commit run --all-files: all hooks passed with that same temporary log-directory move. The first run failed only docs-check on the same untracked prompt link. The directory was restored after each run and remains unmodified.
- python3 scripts/check_docs.py check: passed with the temporary log-directory move.
- python3 scripts/check_platforms.py check: passed.
- python3 scripts/rust_python_parity.py self-test: passed.
- python3 scripts/gate_sensitivity.py check: passed, 11 of 11 planted mutants killed.
- gh run list --branch main --limit 5: unavailable (exit 4, GitHub CLI not authenticated). Main CI is unknown, not green by inference.
- No Rust production files changed in this review target; the Rust workspace quality matrix is not a gate for these process-only stories.

## Documents opened

AGENTS.md; docs/INDEX.md; docs/CONSTITUTION.md; docs/BACKLOG.md; docs/development/ENGINEERING_SYSTEM.md; docs/development/AGENT_PROTOCOL.md; docs/development/AGENT_TOOLCHAIN.md; docs/development/WORK_ITEM_MODEL.md; docs/development/RELEASE_GATES.md; docs/security/SAFETY_INVARIANTS.md; docs/security/THREAT_MODEL.md; docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md; project/epics/E34.json; project/decisions.json; project/templates/VERIFIER_PROMPT.md; project/evidence/E34-S04/EVIDENCE.md.

## Overall verdict

FAIL: three of four judged stories were rejected, for a 75% round yield. ADR-0025 requires another independent round after repair. S01 and S02 return to in_progress; S03 and S04 are blocked by their failed dependencies. This record is a round-1 finding, not epic closure. No production code or committed history was changed by this verifier.
