# Evidence Packet - E17-S08

- Commit/PR: the MSRV raise on `main`
- Executor: Claude
- Independent verifier: **required and not yet performed.** CR4; the executor may not close one or
  write its own Safety Verdict.
- Change Risk: **CR4**. Filed as CR2 and raised in flight: the story turned out to touch
  `rust/crates/cancellai-safety/src/*` (CR4 floor) and `rust/crates/cancellai-platform/src/*`
  (CR3), because clippy reads `rust-version` and the raise enabled lints that had been skipped.
- Spec version/commit: `project/epics/E17.json` at this commit
- Authorising decision: [ADR-0026](../../../docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md),
  accepted by the owner on 2026-09-13

## Outcome

IMPLEMENTED - awaiting independent verification

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - one minimum, stated everywhere | `rust/Cargo.toml` (`rust-version = "1.88.0"`), `.github/workflows/rust.yml` (`rust: ["1.88.0", "stable"]`), ADR-0015 (original text kept as a quote, because the decision to leave it was made against it), `docs/security/SUPPLY_CHAIN.md`, `AGENTS.md`, `.claude/skills/rust-kernel-guard/SKILL.md`, and ADR-0024's incidental reference. A grep for `1.85` outside the changelog, the generated backlog and the two ADRs that record history returns nothing - including two source comments that asserted the old promise, in `knowledge_bundle.rs` (E16-S07's own rationale, one hour old) and `retention.rs`. | PASS |
| AC2 - the waiver goes with the reason | `ratatui` 0.29 -> 0.30.2 takes `lru` 0.12.5 -> 0.18.4, closing GHSA-rhfx-m35p-ff5j (patched in 0.16.3), and drops `paste` from the graph entirely. `rust/deny.toml`'s `ignore` list is now empty rather than carrying a renewed waiver. | PASS |
| AC3 - enabled lints are satisfied, not suppressed | Five sites, no `#[allow]` anywhere: four `collapsible_if` (`manifest.rs` x2, `manifest_provider.rs`, `wsl.rs`) rewritten as let-chains, and one `manual_is_multiple_of` (`knowledge_bundle.rs`). These lints were always applicable; the old declared version was hiding them. | PASS |
| AC4 - one crate owns terminal state | `ratatui 0.30` uses `crossterm 0.29` while `cancellai-tui` declared 0.28, so **both were compiled** - two copies of the crate owning raw mode and the event stream in one process. `cargo deny` only warns on duplicates and would not have stopped it. `cancellai-tui` now declares 0.29; `cargo tree` shows one version. | PASS |
| AC5 - the rejected branch is recorded as not taken | The owner accepted. The advisory is closed rather than accepted as residual risk, and the Dependabot security update for `lru` should now be satisfiable rather than dismissed. | PASS |

## Safety Evidence

Every edit below is a lint-driven rewrite with no intended behaviour change. They are listed one by
one rather than summarised, because "mechanical" is the word that precedes most kernel defects.

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-022, SI-029 | A malformed signature hex string accepted, or a valid one rejected, by the rewritten length check | `decode_hex`: `text.len() % 2 != 0` -> `!text.len().is_multiple_of(2)`. For a `usize` and a constant divisor of 2 these are the same predicate; the divisor is a literal, so the `is_multiple_of(0)` edge case is unreachable. The odd-length, non-hex and valid paths are all covered by the crate's existing tests - 101 in `cancellai-safety`, unchanged and passing. | PASS |
| SI-021 | A manifest root with an empty `env_var` or an escaping `subdir` slipping through the rewritten validation | `if let Some(x) = &opt { if cond {` -> `if let Some(x) = &opt && cond {`. A let-chain evaluates the pattern first and the condition only on a match, which is what the nesting did. `ManifestError::EmptyEnvVarName` and `ManifestError::PathEscapesRoot` are both covered by existing tests. | PASS |
| SI-021 | A path outside the root matched, or one inside it missed, in the manifest provider's glob walk | Same rewrite of `if let Ok(relative) = path.strip_prefix(root_path) { if matches_pattern(..) {`. The `strip_prefix` guard still gates `matches_pattern`, so a path that is not under the root is still never matched. | PASS |
| SI-017 | A `/proc/mounts` octal escape decoded differently, changing an observed mount path | `if octal.bytes().all(is_octal_digit) { if let Ok(value) = u8::from_str_radix(&octal, 8) {` -> one let-chain. The `continue` and the `i += 4` advance stay inside the same branch; a non-octal sequence or an unparseable byte still falls through to `out.push(chars[i])`, which is the path that keeps a malformed escape literal rather than dropping it. | PASS |
| n/a | A dependency reaching the kernel ring through `ratatui 0.30` | It does not. `ratatui`, `ratatui-core`, `ratatui-widgets`, `ratatui-crossterm`, `kasuari` and `crossterm` are reachable only from `cancellai-tui` (`cargo tree -i`), which is outer ring under ADR-0019. No kernel crate's dependency set changed. | PASS |
| n/a | The graph growing more than the change warrants | 99 -> 117 compiled crates (`cargo tree --workspace --edges normal`). `Cargo.lock` grew by 115 entries, which is the number a careless reading reports: most are optional dependencies of the new crates - the `termwiz`, `termion` and `termina` backends - that are never compiled, because `ratatui`'s default features select `crossterm` only. | PASS |

## Verification Commands

```text
cargo fmt --check                                                     -> clean
cargo clippy --workspace --all-targets --all-features -- -D warnings  -> clean, no allow() added
cargo check --workspace --all-targets                                 -> clean
cargo test --workspace                                                -> 593 passed, 0 failed
cargo deny check                                                      -> advisories ok, bans ok, licenses ok, sources ok (ignore list empty)
cargo tree --workspace --edges normal                                 -> 117 crates; one crossterm; ratatui reachable only from cancellai-tui
python3 scripts/check_workflows.py check / check_docs.py check        -> clean
```

## Compatibility

- **This is the compatibility change.** Building from source now requires rustc 1.88.0, released
  mid-2025; current stable is 1.94. Nothing about the shipped binaries or the Homebrew formula
  changes - the formula installs `cancellai.py`, which is stdlib Python.
- One API break absorbed inside `cancellai-tui`: `border::Set` gained a lifetime parameter, so four
  function signatures say `border::Set<'static>`. The values were always `&'static str` literals.

## Performance / operability

- Not measured. 18 more crates to compile is a build-time cost, not a runtime one; the TUI's
  behaviour is unchanged and its tests pass unmodified.

## Residual risks

- **1.88 is not verifiable on the executor's machine.** Local `rustc` is 1.94 and no 1.88 toolchain
  is installed; installing one is not an executor decision. Every claim about 1.88 here rests on
  the MSRV legs of `rust.yml`. This is the same gap E16-S07 recorded, one version higher.
- **`ratatui 0.30` is a major upgrade adopted in one step**, on a TUI whose test coverage is
  thinner than the kernel's. It compiles, it lints clean and its tests pass; nobody has looked at
  the rendered output since the upgrade.
- **The lint edits are the part worth reviewing.** Two are in floored crates and all five are the
  kind of change whose justification is "it is obviously the same" - which is exactly the
  justification this repository does not accept without an independent reader.
- **An MSRV bump is a code change here, not a configuration change.** Clippy is MSRV-aware, so the
  next bump will enable another set of lints inside the kernel. ADR-0026 records this; nothing
  prevents it.

## Safety Verdict

**Not issued.** A CR4 Safety Verdict requires an independent verifier. What the executor can state
is what was checked and what was not, which is above.

## Verifier verdict

pending
