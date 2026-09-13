# Evidence Packet - E27-S01

- Commit/PR: the lint policy on `rework/codebase-health`
- Executor: Claude
- Independent verifier: **required and not yet performed.** CR4: the change touches
  `rust/crates/cancellai-sealedfs/src/*`, and the executor may not write its own Safety Verdict.
- Change Risk: **CR4**, the floor `project/risk_floors.json` sets for the sealed filesystem crate
- Spec version/commit: `project/epics/E27.json` at this commit
- Authorising decision: [ADR-0028](../../../docs/adrs/0028-lint-policy-states-the-safety-thesis-and-differs-by-ring.md)

## Outcome

IMPLEMENTED - awaiting independent verification

## What was measured before anything changed

No number in this repository described the code until this story. All five are new.

| Measurement | Before |
| --- | --- |
| Rust region coverage (`cargo llvm-cov`, first run in the project's history) | 94.54% overall; worst kernel-ring file `cancellai-sealedfs/src/lib.rs` at 92.57% |
| `unsafe` blocks vs `SAFETY:` comments | 41 vs 30 - **eleven undocumented**, seven of them only reachable under `cfg(windows)` |
| Panic sites in production code | 13, six of them indexing or slicing strings that arrive from third-party provider manifests |
| Clippy lints configured workspace-wide | **one** (`unsafe_code = "forbid"`) |
| Gate wall-clock cost | 26 gates, **13 seconds total** - which is why nothing was removed for speed |

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every unsafe block carries a written argument | 41 of 41, up from 30. Enforced by `clippy::undocumented_unsafe_blocks = "deny"`, and asserted independently of clippy by `test_every_unsafe_block_in_the_workspace_carries_a_safety_comment`, so the property survives the lint being relaxed. That test also asserts no `unsafe` exists outside the ADR-0017 crate. | PASS |
| AC2 - no production panic through unwrap/indexing/slicing, on any tier-1 platform | Thirteen sites closed. Clippy with `-D warnings` is clean on native, `x86_64-pc-windows-gnu` and `x86_64-unknown-linux-gnu`. The cross-target runs are not decoration: **seven of the eleven undocumented unsafe blocks and one slicing panic exist only behind `cfg(windows)`** and no single-platform run could see them. | PASS |
| AC3 - a dropped workspace lint is refused | `lint_policy_errors()` in `scripts/check_rust_workspace.py`. Shown to fail, not merely to pass: removing `undocumented_unsafe_blocks` from the sealedfs table produces `local lint table drops clippy.undocumented_unsafe_blocks`, and restoring it clears. | PASS |
| AC4 - an exemption is declared in source | `cancellai-tui/src/lib.rs` carries `#![allow(clippy::indexing_slicing)]` with the ring argument written above it. The one exemption that cannot work this way is `unsafe_code`, because `forbid` is unliftable from an attribute - which is exactly why sealedfs needs a manifest table, and why AC3 exists. | PASS |
| AC5 - the test relaxation does not reach production | `rust/clippy.toml`'s four `allow-*-in-tests` options apply only inside `#[cfg(test)]`. Integration tests are separate crates those options do not reach, so each of the 13 files carries the equivalent inner attribute with the reason above it. Production code has no such attribute. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | A panic inside the mutation boundary aborting a run part-way through | Two of the thirteen were real rather than theoretical. `cancellai-cli`'s execution loop indexed `action.target_artifact_ids[0]`, so a plan action naming no target aborted mid-run; it now records an `INTERNAL_FAULT` result for that action and continues, which is what every other unexpected shape in that loop already did. And the Windows rename path in `windows_sealed.rs` sliced a buffer whose size was computed a few lines earlier; it now returns a refusal. | PASS |
| SI-019 | The rewrite changing what the mutation boundary does | It does not. Every other sealedfs edit is a `SAFETY:` comment - no statement executes differently - and `cargo test --workspace` is 600 passed, 0 failed, unchanged in count except for the two tests this story added. | PASS |
| SI-021 | A provider manifest pattern reaching a panic through a byte index | `segment_matches` indexed `&str` by byte offset in five places. Each was sound because a preceding `starts_with`/`ends_with` had put the offset on a character boundary - four interacting facts held in a reader's head, in a function whose input is written by a third-party manifest author. Every index now goes through `get`, and an impossible one returns `false`. | PASS |
| SI-021 | The rewrite silently changing which paths match | `exhaustive_glob_agrees_with_a_naive_reference` compares it against a naive recursive matcher over **every** pattern and input up to length four across `{a, *, é}` and `{a, b, é}` - 10,000+ pairs, exhaustive rather than sampled, with a guard asserting the corpus is non-empty so the test cannot pass vacuously. `multibyte_boundaries_do_not_abort` adds the specific shapes that break a byte-indexed matcher, including `𝄞` (four bytes). This is the differential method `scripts/rust_python_parity.py` already uses, applied to a parser that had never been tested that way. | PASS |

## Verification Commands

```text
cargo llvm-cov --workspace --summary-only                             -> 94.54% regions (baseline)
cargo fmt --check                                                     -> clean
cargo clippy --workspace --all-targets --all-features -- -D warnings  -> 0 errors
  ... --target x86_64-pc-windows-gnu                                  -> 0 errors
  ... --target x86_64-unknown-linux-gnu                                -> 0 errors
cargo test --workspace                                                -> 600 passed, 0 failed
cargo deny check                                                      -> advisories ok, bans ok, licenses ok, sources ok
python3 -m pytest tests -q                                            -> 505 passed
python3 scripts/check_rust_workspace.py check                         -> 13 crates, acyclic, lint policy intact
```

## Compatibility

- No public API, schema or wire-format change. Two error paths are new where a panic was:
  a plan action with no target is now a recorded failure, and an undersized rename buffer is now a
  refusal. Both were previously aborts.

## Performance / operability

- Not measured, not material: the rewrites replace an index with a checked index. Build time is
  unchanged; no dependency was added.

## Residual risks

- **This is a CR4 change with no independent verdict.** The sealedfs diff is overwhelmingly
  comments, but "overwhelmingly comments" is the executor's own summary of its own diff, which is
  the thing this repository does not accept.
- **`indexing_slicing` is relaxed for the whole TUI crate**, not per-site. The argument is
  ring-based and written down, but a future TUI change that did decide something would inherit the
  relaxation silently.
- **Coverage was measured, not gated.** 94.54% is a baseline with no floor under it, and the worst
  kernel file is below the average. A gate belongs in its own story with its own threshold argument.
- **Miri never ran.** It needs the nightly toolchain, which is not installed, and installing a
  toolchain is an owner decision. Every claim here about the unsafe blocks is a reading of the
  code and its `SAFETY:` argument, not a dynamic check.
- **The lint set is a judgement.** `pedantic` and `restriction` were rejected as noise, and that
  line was drawn by the executor.

## Safety Verdict

**Not issued.** A CR4 Safety Verdict requires an independent verifier.

## Verifier verdict

pending
