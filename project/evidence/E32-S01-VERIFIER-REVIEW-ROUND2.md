# E32-S01 Verifier Review — Round 2

Review-Scope: story
Review target: `96c1b9a` (`96c1b9a^..96c1b9a`)
Verifier: Codex
Brief-Checksum: 8f0da877e84bb0416b80425b88f7fb753164ad81ccef6aea819c0da83d954a86
Date: 2026-09-20

## Verdict

`FAIL`

The repair closes the round-1 reproduction for a balanced triple-backtick fence, but it still
reads verdict-shaped content from code in two malformed/standard-Markdown cases. A CR4 closure
gate must not turn a real current rejection into a pass merely because a later evidence example is
not perfectly formatted.

## Independent reproductions

The requested round-1 reproduction now behaves correctly:

````text
`REJECT`

```text
`PASS`
```
````

returns `False`. Its reverse, a real `PASS` followed by a balanced language-tagged fence
containing `` `REJECT` ``, returns `True`; the real prose verdict controls. A real `REJECT`
immediately before a fence boundary also remains controlling, and a real `PASS` after a balanced
fence remains controlling.

However, each following file returns `True`:

````text
`REJECT`

```text
`PASS`
````

The missing closing backticks cause `FENCED_CODE_RE` not to match anything, so the illustrative
`PASS` is scanned as a real verdict. This is malformed Markdown, but it is untrusted/freeform
evidence input and must fail closed.

````text
`REJECT`

~~~text
`PASS`
~~~
````

`~~~` is a standard CommonMark fenced-code delimiter. No current evidence file uses it (the
repository's existing `FENCED` convention also recognises only backticks), but that convention
does not make its content prose. The parser must exclude both supported delimiter forms, or reject
an unclosed fence before it can supply a later verdict.

`project/evidence/E12-S04/SAFETY_VERDICT.md` remains correctly refused at its current Round-6
state (`False`); this confirms the repair does not spuriously flip that currently open CR4 story.

## Acceptance criteria

| AC | Independent result | Status |
| --- | --- | --- |
| Latest genuine pass closes append-only history | Balanced triple-backtick fence handling preserves the existing fail-then-pass behavior. | PASS |
| Latest genuine failure/rejection blocks closure | Defeated: both an unclosed triple-backtick fence and a balanced `~~~` fence let a later illustrative `PASS` override a real `REJECT`. | FAIL |
| No standalone verdict blocks closure | Existing regression remains passing. | PASS |

## Required disposition

E32-S01 returns to `in_progress`. The executor must make the safety-verdict parser fail closed
for all Markdown fenced-code input it accepts: at minimum, exclude balanced `~~~` fences and
treat an unclosed backtick/tilde fence as code through end-of-file (or refuse the Safety Verdict
file). Add regressions for real `REJECT` followed by each form of fenced `PASS`, then repeat
independent verification. E12-S04 must remain `in_progress`; its otherwise-confirmed store result
cannot close until this CR4 closing gate reads only an attributable, eligible final verdict.

## Gate status

- PASS: `python3 -m pytest tests -v` (667 passed, 578 subtests); `python3 scripts/project_os.py
  check`; `scripts/check_process.py check`; `scripts/check_evidence.py check`;
  `scripts/verifier_handoff.py check`; `scripts/check_docs.py check`; `scripts/check_ears.py
  check`; `scripts/gate_sensitivity.py check`; `cargo fmt --check`; `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-targets`; `cargo
  test --workspace`; and `cargo deny check` (only its pre-existing allow-list/duplicate warnings).
- UNAVAILABLE: `python3 -m ruff check .`, `python3 -m ruff format --check .`, and `python3 -m
  mypy scripts/project_os.py`. The active Python 3.13 environment has neither module. As required,
  `python3 -m pip install -r requirements-dev.txt` was attempted first; Homebrew's externally
  managed environment refused the system-wide install.

## Scope isolation

No `rust/` file has changed since the E12-S04 Round-6 review. The Rust store conclusions remain:
AC1/ADR-0033 is `PASS_WITH_ACCEPTED_RESIDUAL`, AC2/SI-020 is `PASS`, and the E13-S02
mutation-reference contract is `PASS`. This rejection concerns only the E32-S01 CR2 gate parser
that would otherwise be used to close E12-S04.
