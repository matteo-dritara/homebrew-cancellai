# E06 Independent Verifier Review - Round 2

- Review target: `4024ab8..057578d`
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-01
- Epic: E06 - Rust CLI Parity and Cutover

All in-scope stories (`E06-S01` through `E06-S03`) were `ready_for_review` before
this round began. `E06-S04` is out of scope and remains `blocked`; its round-1 CR4
Safety Verdict remains `REJECT` and nothing here supersedes it.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S01 | FAIL | Round-1 custom-root, configuration-symlink, malformed-settings, partial-scan, and process-precondition cases are closed: a custom `CLAUDE_CONFIG_DIR` root is reported `custom`/not mutation-eligible and `clean --yes --json` exits 4 without deleting; custom-root `configure` exits 4; malformed default `settings.json` is unchanged and exits 4. But a default-named provider root that is itself a symlink remains mutation-eligible: with `$HOME/.claude -> <outside>` and a stale `<outside>/projects/proj/11111111-1111-4111-8111-111111111111.jsonl`, `clean --tool claude --days 7 --keep-latest 0 --allow-running --yes --json` exited 0 and deleted the outside file. |
| E06-S02 | FAIL | The round-1 uncited-text counterexample is closed (`INTENTIONAL_DIVERGENCES={"fx": "uncited free text"}` leaves one comparison error), and default/custom corpus runs pass. However, `INTENTIONAL_DIVERGENCES={"fx": "unrelated accepted ADR-0014"}` suppresses a deliberately divergent candidate/root/completeness projection (zero errors). The script only projects `candidates`, `withheld`, `root_origin`, `root_confidence`, `mutation_eligible`, and `scan_complete`; it cannot observe protected/unknown coverage, non-delete discovered identity records, or non-delete proposed actions. Thus an omission/divergence in those normative semantics still passes. |
| E06-S03 | PASS_WITH_RESIDUALS | The source-built side-by-side smoke contract still passes: `cargo test -p cancellai-cli` covers engine/version identification, separate command names, read-only no-trace behavior, narrowly scoped real clean, and CWD-independent invocation. No cancellAI-owned state exists in either engine to migrate. It remains blocked operationally by the failed E06-S02 dependency. Full installer/upgrade/uninstall validation remains E17 scope. |
| E06-S04 | OUT_OF_SCOPE | Remains `blocked`, with the existing `project/evidence/E06-S04/SAFETY_VERDICT.md` verdict unchanged (`REJECT`). |

## Required repairs and carry-forward work

### E06-S01: provider-root symlink authority bypass

The reproduction was performed on a synthetic tree only:

```text
HOME=<tmp>/home
<tmp>/home/.claude -> <tmp>/outside
<tmp>/outside/projects/proj/11111111-1111-4111-8111-111111111111.jsonl  (stale)
cancellai-cli clean --tool claude --days 7 --keep-latest 0 --allow-running --yes --json
```

The command returned 0 and removed the session in `<tmp>/outside`. The lexical default-root
comparison treats the symlink as default; discovery, `ApprovedRoot::establish`, and the
eventual deletion then follow it. The same root form is also eligible for `configure`.

Required repair: establish a root capability that rejects a root object/path traversal through a
symlink, junction, or reparse point before planning and immediately before every mutation or
configuration rewrite. Do not infer default-root authority from the lexical `$HOME/.claude` or
`$HOME/.codex` name alone. Add Unix symlink and Windows junction/reparse regressions for both
`clean` and `configure`, including root drift between observation and mutation.

This violates E06-S01 AC2/AC3, SI-002, SI-003, SI-013, C-02, C-03, C-05, C-06, and C-07.
Per the two-round ceiling, it is recorded as new CR4 backlog item **E07-S07** rather than
opening an E06 review round 3.

### E06-S02: allow-list authorization and semantic blind spots

The allow-list checks that *some* accepted ADR/RFC ID occurs in free text, not that the cited
decision approves this fixture and the exact difference. `ADR-0014` concerns release cadence,
not fixture `fx` or a parity exception, but it suppresses a fully divergent comparison.
Additionally, the only projected keys are the six listed in the evidence table, so removing a
protected/unknown artifact or changing a non-delete action cannot change the comparison result.

Required repair: replace free-text suppression with a structured divergence record that binds an
accepted ADR/RFC section to fixture ID and allowed semantic fields, and validate that binding.
Project all discovered identity records, protection/unknown coverage, scan state, root authority,
and every proposed action; add injected failures for every field and for an unrelated accepted
citation.

This violates E06-S02 AC1/AC2 and M6's requirement that every unexplained semantic divergence
blocks cutover. Per the two-round ceiling, it is recorded as new CR2 backlog item **E07-S08**
rather than opening an E06 review round 3.

## Gate status

| Command | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS, with three existing unmatched-license-allowance warnings (BSD-2-Clause, BSD-3-Clause, ISC) |
| `.venv/bin/python -m pytest tests -v` | PASS after control-plane generation: 179 passed, 22 subtests passed. An earlier run's sole failure was expected generated-doc drift while this review had stories in `verification`; it was re-run after generation. |
| `.venv/bin/python -m ruff check .` / `ruff format --check .` | PASS |
| `.venv/bin/python -m mypy` over every AGENTS.md target | PASS |
| Generated docs, project OS, docs, workflows, fixtures, schemas, characterization, differential harness, Rust-workspace, mutation-boundary, provider-compatibility, process, and release checks | PASS after control-plane generation |
| `.venv/bin/python scripts/rust_python_parity.py self-test` / `check` | PASS: self-test and all 10 NORMATIVE fixtures in default and custom-root scenarios |
| Round-1 custom-root deletion, configuration symlink, malformed settings, partial scan, process guard, and uncited allow-list probes | PASS (closed) |
| Adversarial default-root symlink and unrelated-accepted-ADR / semantic-coverage probes | FAIL as reproduced above |

## Overall verdict

**FAIL — round 2 of at most 2.** E06-S01 returns to `in_progress`; E06-S02 is `blocked` by
E06-S01; E06-S03 is `blocked` by E06-S02; E06-S04 remains `blocked` and out of scope. No third E06 review is
authorized. E07-S07 and E07-S08 carry the surviving findings forward explicitly; neither is an
acceptance of cutover or a replacement for E06-S04's owner-visible CR4 Safety Verdict.
