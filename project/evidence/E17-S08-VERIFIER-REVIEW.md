# E17-S08 independent verifier review

- Verifier: Codex (OpenAI); original executor: Claude. No private executor reasoning was used.
- Date: 2026-09-13.
- Original target: proposal `05defd4`, implementation `31206c2`, Windows follow-up `4c2801b`;
  selected diffs within `05defd4^..4c2801b`, not attribution of every interleaved commit.
- Repaired source target: `ba30b42`. Repairs: `28c29a7` strict verification, `5c36b3b`
  mutation-sensitive tests, `003d00e` advisory scope/evidence corrections, `ba30b42` test lint.
- Authorization: the owner explicitly requested standalone review, repair, per-story verdicts
  and closure on passing verdicts. E17 remains planned, E16 remains blocked, and E17-S07 is
  untouched. This is not release or epic approval.

## Story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E17-S08 | PASS_WITH_RESIDUALS | Six lint rewrites preserve behavior. Independent review repaired a pre-existing strict-signature contract violation, transitive-advisory omission, mutation coverage gaps and inaccurate measurements. Final gates and scoped CI evidence are below; native terminal/process-probe limitations remain explicit. |

The initial target was not acceptable as a completed evidence-backed CR4 story: the findings
below were reproduced, not inferred from failing executor tests. The final verdict follows
owner-authorized repairs in this one requested review round. The verifier authored those repairs;
the original independent assessment and subsequent repair verification are not misrepresented
as two independent reviewers or an independent organization.

## Per-AC assessment

| AC | Verdict | Concrete evidence |
| --- | --- | --- |
| AC1 - same minimum on every specified surface | PASS | Cargo, CI, ADR-0015, ADR-0024, SUPPLY_CHAIN, AGENTS, kernel-guard skill and source comments use 1.88.0 as the current minimum. Historical 1.85 references remain in earlier story/ADR evidence. The executor's claim that its broad grep found no matches was false and is corrected. |
| AC2 - adopt held dependency and retire waiver | PASS after repair | ratatui 0.30.2 selects lru 0.18.4; paste is absent from the full lockfile. Empty ignore list verified. `unsound = "all"` now covers the transitive advisory the default skipped; old graph fails and new graph passes. |
| AC3 - enabled lints satisfied without suppression | PASS after coverage repair | Five let-chain edits plus one parity edit inspected separately below. No added suppression. Existing suites caught most mutants; added odd-hex and malformed-octal regressions catch two genuine survivors. |
| AC4 - one terminal-state crate | PASS | One crossterm 0.29.0 under each requested target's normal dependency graph. No new package reaches kernel consumers; reverse trees and kernel forward trees were independently compared. |
| AC5 - owner rejection branch recorded as not taken | PASS | ADR-0026 is accepted. No Dependabot dismissal or advisory-risk acceptance was performed or invented. |
| AC6 - weak local key/signature refuses without losing current | PASS after safety repair | Added to the contract before repair under the owner's repair authorization. Permissive verification violated ADR-0024; `verify_strict` rejects the independently reproduced input, valid signed fixtures continue to pass, and current state is unchanged on refusal. |

## Reproduced findings and repairs

### F1 - strict signature requirement was not implemented (safety implementation defect)

ADR-0024 specifies strict Ed25519 verification; the module used the permissive `Verifier::verify`.
A synthetic weak key already present in local policy allowed forged input to authenticate.
This requires a weak local-policy key: it is not a claim of forging signatures under arbitrary
properly generated trusted keys. The mismatch predates the MSRV raise, but was found in its
edited safety module while checking SI-022/SI-029.

Reproduction: run `verifier_local_policy_weak_key_is_rejected` with the original permissive
call. It fails on `4c2801b` and passes with `28c29a7`; reverting only the strict call on the
repaired source fails again. The test also proves a refused apply preserves the complete
current bundle. The authorizing strict requirement already existed; AC6 makes this additional
repair scope explicit instead of calling it a behavior-neutral lint fix. SI-022 documentation,
supply-chain text and Unreleased changelog were updated with it.

### F2 - a passing advisory gate omitted the transitive unsoundness case (gate defect)

The ADR attributed the omission to a GitHub-only advisory. That is false:
[GHSA-rhfx-m35p-ff5j](https://github.com/advisories/GHSA-rhfx-m35p-ff5j) aliases
[RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002.html), whose patched range
starts at lru 0.16.3. The local fetched RustSec database also contains that record.

Reproduction against a disposable `31206c2^` checkout:

1. `cargo deny check advisories` with the old configuration returns exit 0.
2. Add `unsound = "all"` under `[advisories]`, leaving the old paste waiver intact.
3. The same command returns exit 1 and specifically reports RUSTSEC-2026-0002 on lru 0.12.5.
4. With the repaired graph, `cargo deny check` returns exit 0 for advisories, bans, licenses
   and sources, with `ignore = []` and the stronger scope.

Cargo-deny 0.20.2's default limits unsoundness checks to direct workspace dependencies; lru
is transitive. The explicit wider scope is now committed. This demonstrates a live gate,
not an empty scan that happens to pass. See the project's corrected ADR-0026 and
[cargo-deny configuration reference](https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html#the-unsound-field-optional).

### F3 - mutation coverage claims exceeded the tests (verification defect)

The original safety suite survives removing `!text.len().is_multiple_of(2) ||`. An odd ASCII
signature then reaches out-of-range slicing. It also survives removal of the octal-digit guard:
Rust's byte parser accepts a leading plus sign, unlike the original digit-only rule. Added
regressions pin malformed public signatures, all byte-pair encodings and 512 octal values plus
signed/truncated/Unicode escapes. Both previously surviving mutants now fail. Production hex
and mount parsing are unchanged by the test repair.

### F4 - package, lint and lifetime counts were wrong (evidence defect)

Independent locked snapshots contradict 99 -> 117 packages and lockfile growth of 115. The
correct measurements are below. Eight TUI signatures gained the lifetime, not four. There are
five collapsible-if sites, not four, plus parity; three sites are in floored crates. The
sixth-site follow-up left stale counts in the ADR, packet and changelog. All are corrected.
The alleged two-crossterm state was an intermediate upgrade resolution, not the committed
pre-upgrade baseline, which selected one crossterm 0.28.1.

### F5 - unrelated toolchain report false positive (deferred observational defect)

`check_agent_toolchain.py report` labels all eight explicitly approved pack members unmanaged,
while `check` correctly passes. Its report subtracts only component ids, omitting member ids.
The manifest already records owner approval; no tooling decision or installation was missing.
Filed as planned E17-S11 without changing the enforcing gate or implementing unrelated repairs.

## Six lint rewrites, individually

| Site | Semantic analysis and counterexample evidence |
| --- | --- |
| manifest.rs empty env_var | `Some` match still precedes trimmed-empty check; None/nonempty continue and empty/whitespace refuse with EmptyEnvVarName. Independent probe covers None, normal name, empty, ASCII whitespace and Unicode whitespace. Disabled guard is caught. |
| manifest.rs subdir | Some match still precedes relative-path validation; None/safe paths continue and empty/traversal/absolute paths refuse with PathEscapesRoot. Independent probe covers seven such cases; disabled guard is caught. |
| manifest_provider.rs file matching | strip_prefix success still gates matches_pattern; both must succeed to push. Failure falls through without a match. Existing synthetic layout and symlink-descent tests ran; disabled matching fails three tests. The walk constructs paths below root, so a strip_prefix error is defensive and not claimed as directly injected through that API. |
| wsl.rs octal decoding | Digit-only predicate remains first, byte parse second. Only successful decoding pushes a decoded character, advances by four and continues; overflow/non-octal/truncated fields copy the original character and advance by one. All 512 three-octal-digit values, `\\+10`, overflow and mixed invalid/valid escapes were exercised. |
| process.rs Windows dedup | Case-insensitive name lookup precedes the no-duplicate guard; unmatched/already-recorded names add nothing. Successful matches preserve order. The exact function extracted from source was run with synthetic enumeration including duplicates, unmatched names, empty requested names and enumeration failure. Both disabled matching and disabled dedup are caught by that harness. It is not a Windows OS test. |
| knowledge_bundle.rs decode_hex | For unsigned n, `n = 2q + r`, r in {0,1}; `n % 2 != 0` equals `!n.is_multiple_of(2)`. The literal divisor cannot be zero. A native Rust probe checks 65,536 consecutive values plus usize::MAX-1/MAX. Byte parity still short-circuits before ASCII validation/slicing. Odd/non-ASCII rejection and all 256 lower/upper hex pairs now have regressions. |

### Mutation results

Mutants were run alone against disposable source copies using `cargo test --locked --offline
-p <affected-crate>`. Original baseline and repaired results, with actual failing test names,
are in [verifier-mutations.json](E17-S08/verifier-mutations.json).

| Mutation | Original suite | Repaired suite / independent probe |
| --- | --- | --- |
| Delete empty-env guard | Caught | Caught |
| Delete subdir validation | Caught | Caught |
| Replace matches_pattern guard with true | Caught | Caught |
| WSL advance 4 -> 3 | Caught | Caught |
| WSL remove continue | Caught | Caught |
| WSL delete digit-only guard | Survives | Caught by new malformed-octal test |
| Delete hex parity guard | Survives | Caught by both new decoder/public-signature tests |
| Invert hex parity | Caught | Caught |
| Windows delete dedup guard | Survives macOS cargo test; branch excluded | Exact-function synthetic harness catches it; no native Windows mutant run claimed |
| Windows disable name matching | Not run as native Windows mutant | Exact-function synthetic harness catches it |
| Revert strict signature call | Original weak-key probe fails | Repaired-source mutation caught by weak-key regression |

A macOS survivor in a `cfg(windows)` block is a coverage limitation, not evidence of semantic
correctness. Cross-target clippy passes the real Windows code but does not execute mutants.
The original E16 publisher/sequence guard-swap survivor is equivalent, unlike guard deletion;
its separate review explains why no artificial test is added to kill it.

## Dependency audit

Method: `cargo tree --locked --offline --workspace --edges normal --prefix none --format '{p}'`,
with explicit target; remove blank lines and duplicate package identities. Includes 13 workspace
packages and excludes build/dev edges. This is a selected normal graph, not every compiled unit.

| Target | Before (31206c2^) | After (4c2801b) |
| --- | --- | --- |
| aarch64-apple-darwin | 98 | 116 |
| x86_64-unknown-linux-gnu | 100 | 118 |
| x86_64-pc-windows-gnu | 98 | 115 |

Cargo.lock: 123 -> 225 entries, net +102. Set difference by (name, version): 127 additions and
25 removals; 103 wholly new names and 11 wholly removed names. Text diff name-line counts are
122 additions/20 removals and are not a substitute for parsing package identities.

Every one of the 127 introduced pairs was queried with `cargo tree --locked --offline
--workspace --edges normal,build --target all -i NAME@VERSION`. All commands succeeded;
33 pairs are selected and 94 have no selected reverse graph. Every selected new pair reaches
workspace consumers only through cancellai-tui. Kernel forward graphs for model, safety,
platform and sealedfs are identical before/after with normal/build edges over all targets.
[Full audit data](E17-S08/verifier-dependencies.json) preserves each query's result.

There is exactly one crossterm 0.29.0 on all three normal graphs. Selected duplicate hashbrown
0.16.1/0.17.1 serves rendering/cache internals; syn 2.0.119/3.0.4 serves distinct proc-macro
paths, with the latter pre-existing via curve25519-dalek-derive. Neither splits terminal state.
Cargo-deny still warns about duplicates under the existing policy; no duplicate ban is claimed.
No dependency was added by the verifier's repairs.

## TUI rendering and lifetimes

All eight affected functions are private and receive static border constants or literal strings
from draw. Their `border::Set<'static>` arguments do not over-constrain any existing caller or
public API; no signature change is needed for a hypothetical dynamic border feature.

The attached [render probe](E17-S08/verifier-render-probe.rs) was appended to ui.rs's test
module in disposable pre/post checkouts, using the existing synthetic sample view models.
`CANCELLAI_REVIEW_RENDER_DIR` selected an output directory, then
`cargo test --locked --offline -p cancellai-tui verifier_dump_rendered_screens` ran on each.
Four screens × five dimensions × two Unicode modes × two help states = 80 character buffers;
all matched byte-for-byte. [Per-case hashes](E17-S08/verifier-renders.json) record the comparison.

I visually inspected the four populated 100x30 screens in
[the monochrome character-buffer montage](E17-S08/verifier-render.png). Borders, incomplete-scan
warning, explanation and irreversible-action prompt are intact and readable. This image
reconstructs actual buffer characters, not a screenshot of an interactive terminal; colors,
escape transport, real keyboard delivery and raw-mode restoration were not newly verified.
Navigation/render tests ran independently as part of cargo test. The smaller cases render
identically, including the existing clipping/fallback behavior; this review does not redesign it.

## Other adversarial axes and probe reproduction

| Axis | Actual work and limits |
| --- | --- |
| Path/identity and links/mounts | Manifest traversal/absolute-path probes, existing symlink-descent tests and malformed mount escape corpus. No live WSL mount manipulation claimed. |
| Partial reads/permissions | Existing workspace unknown/partial and process-enumeration failure checks ran; changed predicates add no I/O. |
| Provider/layout drift | Existing realistic/unknown manifest layouts and capability ceilings exercised; disabled glob matching fails. |
| Concurrency | No new synchronization or mutation path. Windows duplicate enumeration tested through a deterministic function seam. |
| Crash/failure/retry | Bundle refusal preserves complete current state. No new persistent transaction; native terminal crash restoration not exercised here. |
| Boundary values | Hex parity, u64 expiry extremes (E16), optional root values and 512 octal cases. |
| Policy/trust conflicts | Weak local-policy key reproduction, unknown/wrong signers and authority tests. No data field grants trust. |
| Platform | Real original-tier-1 CI, fresh repaired CI below, and native-host plus both cross-target clippy commands. Synthetic Windows logic probe remains explicitly narrower than native execution. |
| Malformed/untrusted input | Signed/truncated/Unicode escapes and malformed signature strings; all reject without panic in unmutated code. |
| Performance/large data | Workspace performance guards passed. Lint edits keep branch/loop complexity; no upgrade-specific runtime or memory benchmark was measured. |

Standalone exact-function and arithmetic probes are reproducible with `rustc --edition=2024
--test <probe.rs> -o <temporary-binary>` followed by that binary. Files:
[Windows](E17-S08/verifier-windows-probe.rs), [parity](E17-S08/verifier-parity-probe.rs).
[Manifest probe](E17-S08/verifier-manifest-probe.rs) is appended to manifest.rs's tests in a
disposable checkout and run using its named test filter. All probe data are synthetic.

## CI evidence

Original `4c2801b`: tests, governance, CodeQL and all nine jobs in
[Rust run 34753145038](https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/34753145038)
passed. The MSRV checks ran at 1.88.0 on macOS, Linux and Windows.

Repaired source `ba30b42`: [Rust run 34774901899](https://github.com/matteo-dritara/homebrew-cancellai/actions/runs/34774901899)
passed all nine jobs, including 1.88.0 checks on macOS (103771115709), Linux (103771115708),
and Windows (103771115698), plus stable checks and each native quality job. Job steps were
inspected, including the real workspace check, tests and cargo-deny commands. This is evidence
about the repaired source revision, not just the executor's original commit.

Only CI supplies execution of the 1.88.0 toolchain; local rustc is 1.94.0 and no 1.88 toolchain
was installed. Cross-target clippy is not mislabeled as MSRV or native runtime testing.

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E16.json`
- `project/epics/E17.json`
- `project/agent_toolchain.json`
- `docs/architecture/TARGET.md`
- `docs/architecture/PROVIDER_MODEL.md`
- `docs/architecture/PLATFORM_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/security/SUPPLY_CHAIN.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/development/AGENT_TOOLCHAIN.md`
- `docs/adrs/0015-rust-workspace-toolchain-and-repository-layout.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/adrs/0024-ed25519-dalek-for-knowledge-bundle-signatures.md`
- `docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md`
- `docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md`
- `project/evidence/E16-S07/EVIDENCE.md`
- `project/evidence/E17-S08/EVIDENCE.md`
- `project/templates/SAFETY_VERDICT.md`
- `.github/SECURITY.md`
- `.claude/skills/orient/SKILL.md`
- `.claude/skills/toolchain/SKILL.md`
- `.claude/skills/epic-verifier/SKILL.md`
- `.claude/skills/adversarial-cases/SKILL.md`
- `.claude/skills/risk-gate/SKILL.md`
- `.claude/skills/rust-kernel-guard/SKILL.md`

Also inspected `.github/workflows/rust.yml`, `.pre-commit-config.yaml`, Cargo manifests/lock,
`rust/deny.toml`, every changed source hunk, checker implementations for evidence/reporting,
and the installed cryptographic verifier and RustSec advisory source. External primary sources
used for factual verification are linked with the relevant findings above.

## Residual risks and disposition

No unresolved high/critical safety finding remains in the reviewed/repaired scope. Residuals:
no local 1.88 execution; no native Windows mutant execution; no new live Windows/Linux TUI
check; no independent second model reviewing the verifier-authored repairs; default duplicate
warnings remain policy; E17-S11 tracks the unrelated report-only defect. The last limitation is
explicit: original independent falsification plus owner-authorized repair and mutation-sensitive
verification is not a claim that the repairs themselves received a second independent review.

The owner explicitly asked for two stories reviewed once, with authorized fixes and final
verdicts. Findings were repaired within this round and followed by the full gate rerun; this
record is not an epic-wide stopping decision. E17-S07 and both epics' blockers remain outside
scope. Final PASS_WITH_RESIDUALS authorizes the requested story-state transition, not a release.
## Gates run independently

Full rerun after the last repair at `ba30b42`, using the existing `.venv` for Python and local
`rustc 1.94.0`. No toolchain component was installed. Commands below returned exit 0; Rust
commands ran from `rust/`, all others from the repository root.

| Command | Result |
| --- | --- |
| `python3 -m pytest tests -v` | PASS |
| `python3 -m ruff check .` | PASS |
| `python3 -m ruff format --check .` | PASS |
| `python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py scripts/check_docs.py scripts/check_workflows.py scripts/check_fixtures.py scripts/check_schemas.py scripts/characterize.py scripts/diff_harness.py scripts/check_rust_workspace.py scripts/check_mutation_boundary.py scripts/check_provider_compatibility.py scripts/check_provider_trust.py scripts/check_platforms.py scripts/rust_python_parity.py scripts/release_manifest.py scripts/check_repository_topology.py scripts/check_agent_skills.py scripts/process_metrics.py scripts/check_risk_classification.py scripts/check_agent_toolchain.py scripts/check_evidence.py scripts/check_ears.py scripts/safety_oracle.py scripts/gate_sensitivity.py` | PASS |
| `python3 scripts/gen_docs.py --check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `python3 scripts/check_docs.py check` | PASS |
| `python3 scripts/check_workflows.py check` | PASS |
| `python3 scripts/check_fixtures.py check` | PASS |
| `python3 scripts/check_schemas.py check` | PASS |
| `python3 scripts/characterize.py check` | PASS |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/check_provider_compatibility.py check` | PASS |
| `python3 scripts/check_provider_trust.py check` | PASS |
| `python3 scripts/check_platforms.py check` | PASS |
| `python3 scripts/rust_python_parity.py self-test` | PASS |
| `python3 scripts/rust_python_parity.py check` | PASS |
| `python3 scripts/check_process.py check` | PASS |
| `python3 scripts/release.py check` | PASS |
| `python3 scripts/release_manifest.py check` | PASS |
| `python3 scripts/check_repository_topology.py check` | PASS |
| `python3 scripts/check_agent_skills.py check` | PASS |
| `python3 scripts/process_metrics.py check` | PASS |
| `python3 scripts/check_risk_classification.py check` | PASS |
| `python3 scripts/check_agent_toolchain.py check` | PASS |
| `python3 scripts/check_evidence.py check` | PASS |
| `python3 scripts/safety_oracle.py check` | PASS |
| `python3 scripts/check_ears.py check` | PASS |
| `python3 scripts/gate_sensitivity.py check` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS |
| `cargo clippy --workspace --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings` | PASS |
| `cargo clippy --workspace --all-targets --all-features --target x86_64-unknown-linux-gnu -- -D warnings` | PASS |

Pytest: 483 passed and 444 subtests. Rust: 598 passed, two existing scheduled benchmarks ignored.
System Python initially lacked ruff/mypy; the existing virtual environment passed both, including
mypy over all 25 named modules. Cargo-deny initially refused its sandboxed database lock; the
explicitly permitted rerun passed all four checks. An earlier final-gate attempt caught
`repeat(1)` in the verifier's test; `ba30b42` fixes it, and this entire table was rerun afterward.
Pre-commit hooks also ran on each repair commit. No skipped hook is counted as a full test run.
The final evidence/status-only changes are followed by regenerated-project, documentation,
process, evidence and release checks before commit.
