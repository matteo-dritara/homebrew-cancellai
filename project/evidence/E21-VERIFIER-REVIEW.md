# E21 Verifier Review - Round 1

- Epic: E21 - Target Engine Trust Remediation
- Verifier: Codex (`/root`), independent reviewer
- Date: 2026-09-03
- Requested target: `c00f16f..HEAD`
- Actual target: uncommitted working tree based on `c00f16f56534651e304c12c5040303984317ac3d`.
  `git log c00f16f..HEAD` is empty and the 36-file implementation is unstaged; this is a
  traceability failure and means this review cannot be reproduced from the requested range.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E21-S01 | FAIL | The required disclosures were added, but now state that the G2 blocking defect is repaired. The native unreadable-Claude-`projects/` reproduction below shows the target still reports a clean empty scan and exits 0. The disclosure is therefore again stronger than implementation. |
| E21-S02 | PASS_WITH_RESIDUALS | Both NORMATIVE fixtures exist and current parity covers both root origins. A baseline engine plus the new fixtures produced all four recorded divergences. It is blocked only by failed disclosure dependency E21-S01. |
| E21-S03 | FAIL | A real mode-000 Claude `projects/` directory exits 0 rather than 4 because `resolve_claude` throws away `ScopeCompleteness::Unknown` when scope is `Unavailable`. Nested Claude observation failures are also collapsed/silently ignored. See the CR4 Safety Verdict. |
| E21-S04 | FAIL | The construction-level planning interface is sound in current production call sites, and no destructive path uses public `observed()`. But it faithfully carries a falsely `Complete` value from E21-S03, so it does not satisfy SI-008/SI-009 for the root-unreadable counterexample. |
| E21-S05 | FAIL | The new 2k shipped-path test is useful and rejects an empty result, but the CI microbenchmark and scheduled 10k/100k jobs still run `cancellai-inventory::scan_scope`, not CLI discovery. This violates AC1. |
| E21-S06 | FAIL | `read_parent_from` sets `take(remaining + 1)` and its own enormous-line test accepts `MAX_PARENT_SCAN_BYTES + 1`. It can read 524,289 bytes, violating the explicit 512 KiB maximum. |
| E21-S07 | PASS_WITH_RESIDUALS | `unlinkat` is handle-relative, intermediate links fail closed, unconfirmed variants are removed, and the remaining fstatat/unlinkat race is accurately documented. See the CR4 Safety Verdict. It is blocked by E21-S01. |

## Reproductions and required repairs

### E21-S01 / E21-S03 / E21-S04: Claude root completeness escape

Created a synthetic `$HOME/.claude/projects/project-a/<uuid>.jsonl`, set `projects` to mode
`000`, then ran:

```text
HOME=<synthetic-home> rust/target/debug/cancellai-cli clean --yes --days 1 --keep-latest 0 --tool claude
Nothing to clean: no artifact is both stale and unblocked.
exit 0
```

`discover_claude_sessions` constructs `ScopeCompleteness::Unknown` for its unreadable root,
but `resolve_claude` returns `empty()` whenever `scope == Unavailable`, thereby changing it to
`Complete`. Required repair: distinguish a structurally absent/symlinked `projects/` root from
an unobservable one in `resolve_claude`, preserve the supplied completeness, and add the native
CLI regression asserting exit 4 and no deletion. Replace Claude's boolean companion walker with
per-failure `CompletenessReason`s; record `modified()` and companion `symlink_metadata` errors
instead of `.ok()`/`if let Ok`. Bound reason retention while retaining a truthful total count.
Until repaired, the G2/CHANGELOG repair claims must say the defect remains open.

This violates E21-S03 AC1/AC2/AC4, SI-008, SI-009, SI-010, SI-014, constitutional C-02, and
consequently E21-S01's truthfulness contract and E21-S04's SI-008/SI-009 obligation.

### E21-S02 counterfactual

Reverting only the two `session.rs` files in a temporary copy of the current tree fails to
compile because E21-S04 changed their result types. To test the required pre-S03 state without
changing production files, I built `c00f16f` with the current fixture corpus and updated
characterization map. Parity failed as required with four divergences: `codex-partial-tree` and
`claude-partial-project`, each in default and custom root scenarios. Default scenarios showed
Python `withheld=True`/`scan_complete=False` against Rust `False`/`True` and delete candidates.

### E21-S05: scheduled benchmark remains unreachable from the binary

`.github/workflows/rust-benchmark.yml` runs the new CLI test, then runs the 10k/100k scheduled
datasets only through `cancellai-inventory/tests/performance_scheduled.rs`. I ran that command:

```text
10,000: 0.07s; 100,000: 0.93s; both passed
```

It proves the old inventory traversal meets its budget, not the shipped provider discovery
path. Required repair: retarget the CI microbenchmark and scheduled 10k/100k datasets to
`resolve_claude`/`resolve_codex`, preserving the trend artifact schema and non-degenerate output
assertions. Violates E21-S05 AC1 and verification contract.

### E21-S06: byte bound is one byte too large

`rust/crates/cancellai-provider-codex/src/session.rs` computes
`MAX_PARENT_SCAN_BYTES.saturating_sub(consumed) + 1`; the test explicitly permits one extra
byte. Required repair: never request/read more than the remaining budget and update the
counting-reader regression to assert `<= MAX_PARENT_SCAN_BYTES`. Preserve the existing
lossy/non-UTF-8 selection semantics. Violates E21-S06 AC1.

## Additional counterexamples checked

- Missing provider root, empty readable tree, and symlinked Claude `projects/` remain known-empty
  / non-destructive rather than making all scopes partial.
- `ProviderResolution::observed()` is used in production only for status/inspect rendering and
  mapping already-planned artifact IDs back to provider roots; `build_actions` takes
  `ProviderPlanningView`, so no current destructive route plans from `observed()`.
- `Vec<CompletenessReason>` is unbounded on a hostile failing tree; recorded as a required
  fail-closed operability repair with E21-S03.
- The S07 intermediate-link refusal is deliberate and documented. A symlinked home is already
  non-destructive under E07-S09; relocated non-symlink homes and Unix bind mounts are not
  silently broken. The fstatat/unlinkat residual is described accurately.

## Gates executed

| Command | Result |
| --- | --- |
| `python3 -m pytest tests -q` | PASS: 179 passed, 26 subtests |
| `python3 -m ruff check .`; `python3 -m ruff format --check .` | NOT RUN: ruff unavailable (`No module named ruff`) |
| `python3 -m mypy cancellai.py scripts/*.py` | NOT RUN: mypy unavailable (`No module named mypy`) |
| Project/docs/process/workflow/fixture/schema/characterize/diff/workspace/mutation/provider/release/doc generation checks | PASS (all requested commands run individually after unavailable tools) |
| `python3 scripts/rust_python_parity.py self-test`; `check` | PASS: self-test and 12 NORMATIVE fixtures/both origins |
| `cargo fmt --check`; clippy `-D warnings`; `cargo check`; `cargo test --workspace` | PASS |
| `cargo deny check` | PASS after sandbox-approved advisory DB lock; only unmatched-license warnings |
| Scheduled 10k/100k benchmark | PASS, but only for unreachable inventory path (E21-S05 FAIL) |

## Overall verdict

`FAIL` - E21 cannot close. E21-S01, E21-S03, E21-S04, E21-S05, and E21-S06 require repair;
E21-S02 and E21-S07 are blocked by their failed dependencies. This is review round 1 of 2.
