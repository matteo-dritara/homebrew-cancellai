# E21-S03 Round-2 Repair — Independent CR4 Safety Verdict

- Epic: E21 — Target Engine Trust Remediation
- Story: E21-S03 — Scan completeness propagation in provider adapters
- Review target: `5367f615b915fdf00d6debe9b4acdda5a53dc8f4`, plus current `HEAD`
  `e9ded36` (unrelated later changes retained; this review assesses E21-S03 only)
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-21
- Change Risk: CR4
- Prior verdict: `FAIL` in `project/evidence/E21-S03-S07-INDEPENDENT-REVIEW-ROUND2.md`

## Verdict

**PASS.** The repaired Claude adapter no longer interprets a present, non-symlink,
non-directory `projects` root as a complete empty scope. It supplies a named root-unavailable
observation, the CLI reports that incompleteness, and every requested command surface withholds
with exit `4`. The frozen Python reference and the actual built Rust CLI agree on the original
regular-file counterexample.

This is an independent reconfirmation of the repair, not reliance on the executor's tests or
evidence packet. It is the CR4 Safety Verdict required for E21-S03 to move from
`ready_for_review` to `done` under the repository protocol.

## Documents and code opened

- `AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`
- `project/epics/E21.json`; generated verifier brief from
  `python3 scripts/project_os.py brief E21-S03 --role verifier`
- `docs/development/ENGINEERING_SYSTEM.md`; `docs/development/AGENT_PROTOCOL.md`;
  `docs/development/MIGRATION_PYTHON_RUST.md`; `docs/development/RELEASE_GATES.md`
- `docs/security/SAFETY_INVARIANTS.md`; `docs/security/THREAT_MODEL.md`
- `docs/architecture/TARGET.md`; `docs/architecture/PROVIDER_MODEL.md`;
  `docs/architecture/JSON_CONTRACTS.md`; `docs/CLI_RUST.md`
- The final Claude/Codex adapter sources, CLI resolution/exit handling, executor evidence
  (`project/evidence/E21-S03/EVIDENCE-ROUND2-REPAIR.md`), prior independent review, and the
  committed diff for `5367f61`.

## Independent reproductions

All filesystem probes used a newly-created synthetic directory below `/private/tmp`, assigned
as the child process's `$HOME`; no real provider data was read or written. Scratch trees were
removed after each probe.

### Original regular-file counterexample

I created `$HOME/.claude/projects` as a regular file and used the actual built binary
`rust/target/debug/cancellai-cli`, not an adapter unit-test substitute. With identical arguments:

```text
HOME=<scratch> rust/target/debug/cancellai-cli clean --yes --allow-running \
  --days 1 --keep-latest 0 --tool claude
  => exit 4; "Nothing was cleaned: safety withheld the requested work."

HOME=<scratch> python3 cancellai.py clean --yes --allow-running \
  --days 1 --keep-latest 0 --tool claude
  => exit 4; "SCAN INCOMPLETE: 1 unreadable path(s) in claude"
             ".../.claude/projects: Not a directory"
             "Nothing was cleaned: safety withheld the requested work."
```

The pre-repair branch in `5367f61^` included `!projects_meta.is_dir()` in the
`structurally_empty()` return. The reviewed repair reserves that return for absent and symlinked
roots; the real `fs::read_dir` now observes the wrong-kind root, fails with `ENOTDIR`, and enters
the existing `unobservable()` path. The observed Rust exit code is therefore no longer the
prior false-success `0` and matches the frozen reference's safety outcome.

### Command-surface checks

Against that same regular-file root, I independently obtained:

| Command | Result |
| --- | --- |
| Rust `clean --yes --allow-running --days 1 --keep-latest 0 --tool claude` | exit `4`; withheld |
| Rust `clean --dry-run --allow-running --days 1 --keep-latest 0 --tool claude` | exit `4`; withheld |
| Rust `plan --allow-running --days 1 --keep-latest 0 --tool claude` | exit `4`; no actions; `INCOMPLETE_INVENTORY` diagnostic |
| Rust `inspect --json --allow-running --tool claude` | exit `4`; Claude `scan_completeness` has `complete: false`, `error_count: 1` |

Thus `plan`, inspection, preview, and real cleanup do not reinterpret this observation
differently. The Python reference has no `plan`/`inspect` equivalent; comparison was made on its
matching `clean` surface as required.

## Adversarial counterexamples

| Case | Independent result | Assessment |
| --- | --- | --- |
| Claude `projects` regular file | Real Rust CLI and Python reference both exit `4`; Rust inspection records one incomplete Claude scope. | PASS |
| Claude `projects` FIFO | Real Rust `clean --yes` exits `4` and withholds. `read_dir` treats it as the same wrong-kind error path. | PASS |
| Claude root changes repeatedly between directory and regular-file states | A 100-run external type-flip stress probe produced only `0` (51 runs) and `4` (49 runs), no other exit. `4` is the repaired current-wrong-kind result; `0` occurs when the scan sees the empty directory as a complete snapshot. The test is deliberately not represented as a deterministic proof of the precise metadata/read-dir interleaving, which needs an injected synchronization seam. Code inspection confirms a regular file reached at `read_dir` goes to `unobservable()`, not `structurally_empty()`. | PASS for the repaired wrong-kind path; no new authority escalation found |
| Codex `sessions` regular file | Real Rust `clean --yes --tool codex` exits `4` and withholds; `inspect --json` reports Codex `complete: false`, `error_count: 1`. Codex already lets non-symlink roots reach its `walk_rollouts` `read_dir` error logging. | PASS; no equivalent defect found |
| Claude Unix-domain socket | Not executable in this sandbox: `nc -lU` is refused with `Operation not permitted`. Static path is the same non-symlink `read_dir` error route as the regular file/FIFO cases. | Environment-limited, no contrary evidence |
| Claude character device | Not executable as unprivileged user: `mknod ... c 1 3` is refused with `Operation not permitted`. Static path is the same non-symlink `read_dir` error route. | Environment-limited, no contrary evidence |

No production mutation occurred in any probe. The type-flip probe used an empty directory, so an
exit `0` never authorized or performed a deletion; it is not evidence that an unobservable root
was accepted as complete.

## Regression-test audit

The new tests are substantive, not tautological:

| Test | Assertions audited | Result |
| --- | --- | --- |
| `a_regular_file_projects_root_is_unobservable_not_a_clean_empty_scope` | Builds a real regular-file root, calls discovery, requires `SessionDiscoveryScope::Unobservable`, `ScopeCompleteness::Unknown`, and unobserved count `1`. It would fail under the prior `Unavailable`/complete implementation. | Meaningful; PASS |
| `a_regular_file_claude_projects_root_withholds_and_exits_four` | Spawns the real CLI and requires exactly safety exit `4`, rather than prior successful exit `0`. A root-file case cannot contain a session to assert survives, but the exit-code distinction is the safety contract under SI-014. | Meaningful; PASS |
| `a_regular_file_claude_projects_root_is_reported_incomplete_with_a_real_count` | Parses native JSON, finds the Claude scope rather than relying on array position, and requires `complete == false` and `error_count == 1`. | Meaningful; PASS |
| `a_regular_file_claude_projects_root_withholds_dry_run_too` | Spawns the real preview surface and requires exit `4`; it would fail if preview silently treated the scope as complete. | Meaningful; PASS |

I also ran these tests directly before the workspace suite:

```text
cargo test -p cancellai-provider-claude \
  a_regular_file_projects_root_is_unobservable_not_a_clean_empty_scope  => PASS
cargo test -p cancellai-cli --test cli_behavior \
  a_regular_file_claude_projects_root                              => PASS (3 tests)
```

## Invariant and acceptance evidence

| Requirement | Independent evidence | Result |
| --- | --- | --- |
| E21-S03 AC1; SI-008 | A regular/FIFO scope root becomes unobservable and `clean --yes` exits `4`; no deletion is authorized. Codex's equivalent regular root is also incomplete. | PASS |
| E21-S03 AC2; SI-009 | Wrong-kind roots are not folded into absent-layout completeness. `inspect --json` exposes `complete:false` and the scope's error count. | PASS |
| E21-S03 AC3; SI-010 | The inspected Claude and Codex documents report the real count `1`, not a Boolean disguised as a count. | PASS |
| E21-S03 AC4; SI-014 | `clean --yes`, `clean --dry-run`, and `plan` each exit `4`; inspection emits the incomplete result and also exits `4`. | PASS |
| C-02 | A present, malformed provider root lowers authority to withholding; it does not become a clean empty installation. | PASS |

## Gate results

```text
cd rust && cargo fmt --check
  => PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings
  => PASS
cd rust && cargo test --workspace
  => PASS (full workspace; no failures; scheduled heavy benchmarks remain intentionally ignored)
python3 scripts/rust_python_parity.py check
  => PASS (13 NORMATIVE fixtures, both root-origin scenarios)
```

Session-start governance checks also passed. The CI history query showed the four most recent
push workflows on `main` successful (`codeql`, `rust`, `tests`, `governance`); it also showed a
separate, later scheduled `rust-benchmark` workflow as failed. That scheduled failure is outside
this repair's diff and remains CI state to investigate; it is recorded here rather than silently
calling the branch fully green.

## Residual risks

- The sandbox prevented live creation of a Unix-domain socket and device node. The verified
  regular-file and FIFO cases, and code path inspection, cover the shared non-symlink
  `read_dir` failure route; no behavioral claim is made for platforms whose filesystem semantics
  differ from this Unix probe.
- A precise metadata/read-dir TOCTOU interleaving is not deterministically injectable through
  the public CLI. The stress result and direct control-flow audit cover the repaired outcome when
  `read_dir` sees the changed wrong-kind root. This repair does not claim to make provider
  discovery an atomic filesystem snapshot; that broader pre-existing filesystem race is not
  introduced by this one-clause repair.
- The scheduled `rust-benchmark` CI failure reported at session start is external to this
  E21-S03 verification and leaves repository-wide scheduled CI not fully green.

## Method defects

- none for this verification. The prior round's missed present-non-directory root, its absence
  from a deterministic root-type matrix, and the executor's proposed follow-up are already
  recorded in `project/evidence/E21-S03/EVIDENCE-ROUND2-REPAIR.md`; this review did not create a
  new method defect.

## CR4 Safety Verdict

**PASS — E21-S03's round-2 regular-file-root repair is independently reconfirmed.**
