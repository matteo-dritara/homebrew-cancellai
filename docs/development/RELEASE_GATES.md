# Release Gates

A release is eligible only when the gates required by its changes are green. The project distinguishes feature completion from safety evidence.

## Gates are structural or behavioural, and the difference matters

A process audit verifies that a step was performed; it does not verify that a property holds. The
Nimrod safety case was fully compliant and the aircraft was not safe. So every gate in
`AGENTS.md`'s check list is classified here, and `scripts/gate_sensitivity.py` refuses a gate it
does not find in this table.

**Structural** gates assert that something exists, matches, or is consistent. They are cheap, they
catch real drift, and a green one says nothing about whether the product is safe.

**Behavioural** gates assert that the system does or does not do something. A behavioural gate can
be shown to fail by planting a violation, which is what
[`GATE_SENSITIVITY.md`](../../project/generated/GATE_SENSITIVITY.md) records.

| Gate | Kind |
| --- | --- |
| `pytest` | behavioural |
| `ruff check` / `ruff format --check` / `mypy` | structural |
| `gen_docs.py --check` | structural |
| `project_os.py check` | structural |
| `check_docs.py check` | structural |
| `check_workflows.py check` | structural |
| `check_fixtures.py check` | behavioural |
| `check_schemas.py check` | structural |
| `characterize.py check` | behavioural |
| `diff_harness.py check` | behavioural |
| `check_rust_workspace.py check` | structural |
| `check_mutation_boundary.py check` | behavioural |
| `check_provider_compatibility.py check` | behavioural |
| `check_provider_trust.py check` | behavioural |
| `check_platforms.py check` | structural |
| `check_process.py check` | structural |
| `release.py check` | structural |
| `release_manifest.py check` | structural |
| `check_repository_topology.py check` | structural |
| `check_agent_skills.py check` | structural |
| `process_metrics.py check` | structural |
| `check_risk_classification.py check` | behavioural |
| `check_agent_toolchain.py check` | behavioural |
| `check_evidence.py check` | structural |
| `safety_oracle.py check` | behavioural |
| `check_ears.py check` | behavioural |
| `gate_sensitivity.py check` | behavioural |
| `rust_python_parity.py check` | behavioural |
| `cargo fmt / clippy / check` | structural |
| `cargo test` | behavioural |
| `cargo deny check` | behavioural |

Nine of thirty-one are structural-only in the sense that no planted violation could make them fail
about the product. That ratio is not a target and is recorded so that a green run is read for what
it is: roughly two thirds of this gate set asserts a property, and one third asserts a shape.

## G1 Functional

- acceptance criteria pass;
- unit/integration/contract tests pass;
- CLI/API schemas and generated docs are consistent;
- user-visible behavior has changelog/release-note coverage;
- no known regression is hidden as a warning.

## G2 Safety

- required Safety Invariants preserved;
- threat-model delta reviewed;
- CR3/CR4 adversarial tests pass;
- CR4 independent Safety Verdict is PASS or explicitly owner-accepted PASS_WITH_RESIDUALS;
- no unknown/partial condition is incorrectly promoted to destructive authority.

## G3 Compatibility

- tier-1 OS matrix appropriate to the release passes;
- provider compatibility fixtures pass for capabilities claimed;
- schema/policy/state migration compatibility is verified;
- unknown versions/platforms degrade truthfully.

## G4 Operability

- performance/self-budget regressions within thresholds;
- crash/recovery/rollback requirements pass;
- installer/update/uninstall smoke tests pass;
- documentation and troubleshooting paths exist;
- observability/audit evidence is sufficient for failures.

### Performance budget baseline

Two gates, because they measure two different things and only one of them is the product.

**The shipped discovery path (`cancellai-cli`, E21-S05) - the primary evidence.**
Measured at two scales, both against `resolve_claude`/`resolve_codex`, the exact functions
`cancellai-cli`'s `resolve_all` calls:

- `tests/performance_shipped_path.rs` runs on every `cargo test` and additionally pins two
  structural properties - planning reads the resolved inventory instead of re-walking the
  filesystem, and the protection probe runs exactly once per artifact;
- `tests/performance_scheduled_shipped.rs` carries the heavy 10k/100k datasets, `#[ignore]`d and
  run by `.github/workflows/rust-benchmark.yml`, emitting the machine-readable trend artifact
  under the same `CANCELLAI_BENCH_SIZES`/`CANCELLAI_BENCH_OUTPUT` contract and the same
  `BenchResult` schema the inventory job uses.

Every timing assertion is paired with an assertion on what the resolution actually produced,
because `CR-TE-02`'s lesson is that a benchmark silently measuring an empty tree is
indistinguishable from a fast one. E21 round-1 independent review found the first version of this
story running only the small per-PR test while the heavy datasets still went through
`cancellai-inventory`, which proved the old traversal met its budget rather than the shipped
one.

**The inventory traversal (`cancellai-inventory`, E04-S04) - a crate-contract guard.**

This measures `scan_scope`, which ADR-0018 keeps as the reference implementation of scan
completeness but which the shipped binary does not call - so it is a guard on that crate's own
contract, not on user-visible latency.
`rust/crates/cancellai-inventory/tests/performance_micro.rs` is a CI-friendly regression
ceiling (a few thousand synthetic files, runs on every `cargo test`) that catches a gross
traversal regression without being a tight SLA. The heavy 10k/100k(/1M-on-demand)-entry
benchmarks live in `tests/performance_scheduled.rs`, `#[ignore]`d out of the default test run
and executed weekly (plus on-demand) by `.github/workflows/rust-benchmark.yml`, which uploads
a machine-readable JSON trend artifact (`CANCELLAI_BENCH_OUTPUT`) - the "benchmark summary"
release-evidence item below, once a release references it. Latency/throughput thresholds are
recorded in that file's `THRESHOLDS` table, generously bounded (regression detection, not a
tight SLA) given shared-runner variance.

Latency and throughput were, until E10-S02, the only dimensions actually measured. Peak memory
is now measured too, on one tier-1 platform; CPU and cancellAI's own runtime self-footprint
remain forward-looking budgets, not yet measured, for the reasons recorded below:
- **Peak memory - measured on Linux, disclosed-unmeasured elsewhere (E10-S02).**
  `tests/performance_memory.rs`'s `the_shipped_discovery_path_stays_within_a_peak_memory_budget`
  runs on every `cargo test` on Linux and asserts real peak RSS (`/proc/self/status`'s `VmHWM`,
  a plain file read - no profiling/memory-accounting dependency added, per AGENTS.md's "do not
  add a dependency merely to reduce implementation effort") against a 128 MiB regression budget
  for the same `SESSIONS_PER_PROVIDER = 2_000` synthetic tree the latency gate uses. macOS and
  Windows have no equivalent without a new dependency; the gate's own module is
  `#[cfg(target_os = "linux")]` and simply does not exist on those platforms rather than
  reporting a fabricated or skipped-silently result. `tests/performance_scheduled_shipped.rs`'s
  `BenchResult` gained a `peak_rss_bytes: Option<u64>` field (`None` off Linux) published to the
  same trend artifact for the heavy 10k/100k datasets - informational only, not gated, matching
  this story's own AC2 ("scheduled deep benchmarks publish trends without blocking ordinary PRs
  on noisy metrics"). Cross-compile-`clippy`-verified for `x86_64-unknown-linux-gnu` by this
  story's own executor (no Linux runtime available in that session to execute it for real);
  first real execution is on Linux tier-1 CI.
- **CPU**: target is single-threaded, I/O-bound traversal dominated by syscall latency, not
  CPU-bound work - no concurrency exists yet to budget separately.
- **Self-footprint**: cancellAI's own on-disk/runtime footprint budget (C-11) is a Guardian
  (long-running service) concern; Guardian does not exist yet (a later epic), so there is
  nothing running continuously to measure this against today.

Closing these three gaps is scope for the epic/story that first has something able to
produce the measurement (a profiling dependency review, or the Guardian runtime), not
fabricated here.

## Gate matrix by Change Risk

| Risk | G1 | G2 | G3 | G4 | Independent verifier | Owner Safety Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| CR0 | required | docs consistency | as relevant | as relevant | optional | no |
| CR1 | required | basic | required where exposed | as relevant | optional | no |
| CR2 | required | targeted | required | targeted | safety-relevant changes | no |
| CR3 | required | required | required | required | required | residual-risk summary |
| CR4 | required | required + adversarial | required | required | required | required |

## Rust cutover gate status (E06-S04)

E06-S04's own outcome is "promote Rust to stable only after functional, safety, compatibility,
and operability gates pass" - a gate, not a feature to implement. This section is the living
checklist that gate is evaluated against; it is updated as later work closes a gap, not
rewritten from scratch each time. As of 2026-09-12, since the previous update E06-S01/S02/S03
have closed `done` and E20, E21, E22, and E23 have all closed as well:

**G1 Functional - substantially ready, with disclosed permanent gaps.** Core
`status`/`inspect`/`plan`/`clean`/`configure`/`version` surface exists with matching JSON
schemas and exit-code taxonomy (E06-S01), and a differential gate confirms parity with the
Python reference on the full `NORMATIVE` fixture corpus (E06-S02). `cancellai-cli --help`/`-h`/
`--version` now work at both the top level and per-subcommand (`CR-TE-07`, closing the gap this
section used to list), matching the reference CLI's own surface and the Homebrew formula's
smoke test. The detected Codex native delete backend is no longer an open gap to close: E22-S05
evaluated wiring it and explicitly declined, because doing so safely would require a second
production mutation primitive in the kernel's own boundary (`cancellai-safety::mutation_executor`)
rather than a CR3 side effect of closing a documentation gap - this is now a permanent, disclosed
divergence (`docs/CLI_RUST.md`'s "Known gaps"), not a pending item. Remaining disclosed gaps,
tracked in the same section: no `--aggressive` (legacy/cache category widening - fail-closed,
never a superset of the reference's candidates), no `status --paths/--coverage/--top`, no
`clean --keep-claude-history`/`--verbose`. Whether a session's companion payload directory
itself (versus the session file) has a deletion path was not re-verified for this update.

The incomplete-scan gap that used to be listed here moved to G2 in the prior update. It was
recorded as a missing feature; the 2026-09-03 target-engine review reproduced it deleting an
artifact the frozen reference withholds, which makes it a safety-invariant violation rather than
a functional shortfall. See G2 below.

**G2 Safety - the blocking defect is repaired; the gate still awaits its independent pass.**
`docs/audits/2026-09-03-CODE_REVIEW.md` (`CR-TE-01`) reproduced, end to end on a synthetic
tree, that a directory the scan could not list was silently skipped by both Rust provider
adapters without making the scope incomplete: on the same tree, `cancellai.py` withheld every
destructive action and exited `4` while `cancellai-cli` deleted the eligible artifact and
exited `0`, reporting `scan_complete: true` and `knowledge_confidence: verified`. That violated
SI-008, SI-009, SI-010 and constitutional C-02.

E21 repaired it (`E21-S03`), and did so behind fixtures that fail against the unrepaired engine
(`E21-S02`) so the class cannot silently return: `codex-partial-tree` and
`claude-partial-project` are `NORMATIVE` and run through the differential gate in both
root-origin scenarios. Both providers were affected - the Claude branch more broadly than
previously recorded, since E06-S02 had repaired only the *companion payload* case and an
unreadable **project** directory still passed silently, disclosed nowhere until that review.
`E21-S04` moved the verdict onto `cancellai-inventory`'s own `ScopeCompleteness` (ADR-0018) and
made planning candidates unobtainable without it, so the invariant is now a type obligation
rather than a rule each adapter is trusted to remember.

`E21-S07` additionally replaced detection with prevention on the delete path: the unlink is
issued through `cancellai-sealedfs`'s handle-relative `unlinkat`, so a path-level swap after
validation cannot redirect it, and the two unconfirmed `MutationOperation` variants no caller
requested were removed rather than left armed for E12 to inherit.

**What still blocks this gate is independent re-verification of the repair, not an open
defect**: E21 did have an independent review round - round 1 (`project/evidence/
E21-VERIFIER-REVIEW.md`, Codex, 2026-09-03) returned `FAIL` on five of its seven stories,
including both CR4 stories (`E21-S03`, `E21-S07` was `PASS_WITH_RESIDUALS`). Every round-1
finding was repaired and pinned by regressions written against the verifier's own
reproductions, and the owner then directed epic closure **without spending the second review
round** on those repairs (`project/evidence/E21-CLOSURE.md`, 2026-09-03) - ADR-0014 permits
this as an owner decision. That closure file is explicit about what it does and does not mean:
"one independent adversarial round, all of whose findings were reproduced, repaired, and pinned
by regression tests written against the verifier's own reproductions - and no independent
confirmation of those repairs," and states plainly, "Closing E21 does not make E06-S04 ready."
So this gate's remaining blocker is narrower than before but still real: the repairs to
`E21-S03`/`E21-S07` (the two CR4 stories, both now carrying an owner-visible Safety Verdict at
the *story* level in `project/evidence/E21-S0{3,7}/SAFETY_VERDICT.md`) have not had their own
independent verifier confirmation, and E21-CLOSURE.md's own residual-risk list (the
`fstatat`/`unlinkat` TOCTOU window, bounded reason retention, and a self-audit written by the
same agent that implemented the fix) stands as accepted risk, not closed risk.

Separately, SI-007/SI-008/SI-009/SI-019/SI-020/SI-021/SI-022 are exercised by
targeted unit/integration tests (E06-S01/S02/E21 evidence packets). Missing for CR4: an
independent verifier's own adversarial pass over the E21 repairs specifically (as opposed to the
round-1 pass that found the original defects), and the cutover-level owner-visible Safety
Verdict itself, which this document cannot substitute for.

**G3 Compatibility - ready.** The Windows blocker this section used to record -
`cancellai-platform::identity::SystemIdentityObserver` reporting `IdentityObservation::
Unsupported` unconditionally on non-Unix platforms, so `ApprovedRoot::establish`/`bind` failed
closed and no deletion could ever succeed on Windows - is repaired. E20-S01 (ADR-0020) gave
Windows a real `#[cfg(windows)]` identity observer backed by `cancellai_sealedfs::
observe_identity` (volume serial/file index/reparse classification); E20-S05 completed the rest
of the mutation path natively - process observation (`CreateToolhelp32Snapshot`), allocated size
(`GetFileInformationByHandleEx(FileStandardInfo)`), and handle-relative deletion
(`NtCreateFile` with a retained `RootDirectory` handle, rejecting target swaps and reparse
points). Both closed with an independent `PASS` verdict
(`project/evidence/E20-VERIFIER-REVIEW-ROUND2.md`, Codex, 2026-09-08) against real native
Windows CI runs (`rust.yml` runs 33904582262 and 33905293698) exercising the full quality
matrix - format, clippy, workspace tests, `cargo deny` - plus native reparse/symlink/junction,
identity-mismatch, and target-swap fixtures, and a CR4 Safety Verdict at
`project/evidence/E20-S05/SAFETY_VERDICT.md`. Only a genuinely unsupported non-Unix,
non-Windows target still reports `Unsupported`; that is outside the tier-1 platform contract.
The prior E06-S04 dependency on E20 as a whole was also corrected to remove a stale cycle (E20's
own round-2 review, same evidence file): E06-S04 depends on E20's work, but E20 does not need
E06 to close.

**G4 Operability - not ready, narrower than before.** Packaged installers for macOS/Linux/
Windows now exist (E17-S02, `done`, `docs/RELEASING.md` "Target Rust release factory"), with
provenance/SBOM/signing (E17-S03) and installation-source-aware upgrade guidance (E17-S04) on
top. What remains open in E17 is **E17-S07** (safety incident containment and capability
downgrade), `blocked` on `E16-S05`, which is itself blocked because epic E16 depends on `E15`
(Guardian runtime, phase P4, not yet started) - so this leg of G4 will not close until Guardian
work begins, unless the owner instead accepts a scoped cutover perimeter by ADR per E06-S04's
own acceptance criteria. Independent of E17, no performance/self-budget measurement exists for
the CLI's own command paths, and no crash/recovery testing beyond unit tests. The benchmark
that does exist measures `cancellai-inventory`'s `scan_scope`,
which the 2026-09-03 review found (`CR-TE-02`) is not reachable from the shipped binary at all -
so the performance gate is not merely narrow, it is pointed at code the CLI never executes.
`E21-S05` retargeted it: `cancellai-cli/tests/performance_shipped_path.rs` now measures
`resolve_claude`/`resolve_codex` on every `cargo test`, with every timing assertion paired to an
assertion on what the resolution actually produced, so a benchmark measuring an empty tree fails
instead of reporting an excellent number. The same review measured `CR-TE-04`: peak RSS of
303 MB against the reference's 27.8 MB on a single 287 MB rollout, because rollout metadata
reading buffered the whole file despite documenting a 512 KiB bound. `E21-S06` made the read
streaming and bounded; the same measurement now reads **2.9 MB**, an order of magnitude below
the reference itself.

The release workflow used to be weaker than it stated (`CR-TE-06`): it claimed to re-run every
gate at the tagged commit and ran fewer than half, omitting the differential parity gate and
every Rust check. `E22-S01` closed that: `release.yml`'s `verify` job now runs the full Python
checker set AGENTS.md lists, and a new `verify-rust` job runs the Rust quality set (`fmt
--check`, `clippy -D warnings`, `cargo test`, `cargo deny check`) on all three tier-1
platforms, so the Windows-only clippy failure that slipped through at v1.8.0
(`project/evidence/RELEASE-v1.8.0.md`) would now fail the tag. `scripts/check_workflows.py`
derives the required gate set from `.pre-commit-config.yaml`, AGENTS.md's own "Current Python
checks" list, and `rust.yml`'s `quality` job rather than a hand-copied list, and also rejects
`continue-on-error`/`if:`-guarded gates, a `verify-rust` matrix narrower than `quality`'s, and
a `publish` job not depending on both `verify` and `verify-rust` - closing the round-1
independent verifier finding that the original version of this check still passed against six
independently reproduced regressions of exactly this kind - so this cannot silently regress
the way it did the first time.
`E06-S04` no longer carries this as a blocker.

The `v1.10.0` tag then reproduced a narrower variant of the same class of defect: the `verify`
job's checkout was shallow (GitHub Actions' default), so `check_platforms.py check`'s ancestor
check found each platform's `verified_commit` object entirely absent from the tagged commit's
local history rather than merely unreachable, and the tag's release workflow failed
(`gh run 34252829459`) after `v1.10.0` had already been committed, tagged, and pushed - the
GitHub release itself was never published. `E23-S01` gives the `verify` job's checkout
`fetch-depth: 0` and adds `scripts/check_workflows.py::release_history_gate_errors()` so a
reversion to a shallow checkout on that job fails statically instead of only at the next tag
push.

**Conclusion**: cutover is still not recommended, but the shape of the blocker has changed since
the 2026-09-03 update. G1 is substantially ready with permanently disclosed gaps, and G3 is now
ready with independent verification behind it. What remains is: (a) G2 - the reproduced
authority defect is repaired and pinned by regressions, but repaired-by-executor is not the same
claim as independently-confirmed-repaired, and no verifier has taken a second adversarial pass
at `E21-S03`/`E21-S07` specifically; and (b) G4 - `E17-S07` sits behind a real dependency chain
into Guardian (`E16-S05` → `E16` → `E15`, phase P4, not yet started), plus the still-unaddressed
performance self-budget and crash/recovery gaps. Closing E06-S04 (and E06 as a whole) requires
either this checklist to read "ready" against real evidence with an independent CR4 verifier
pass and the owner's own Safety Verdict acceptance, or an explicit ADR narrowing the cutover
perimeter per E06-S04's own acceptance criteria ("a scoped cutover perimeter decided by ADR") so
that the Guardian-dependent leg of G4 is not required for an initial cutover. Neither path is
something the executor grants itself (`AGENT_PROTOCOL.md`: "an executor's work is finished at
`ready_for_review`... it does not write its own Safety Verdict").

## Epic closure

Closing an epic is what triggers a release (ADR-0014, PD-021). An epic may close when:

- every story is `done`;
- CR4 stories carry a Safety Verdict recording `PASS` or `PASS_WITH_RESIDUALS`;
- at most two independent review rounds were run, and anything surviving round two exists as
  a new backlog work item rather than as an unresolved finding;
- the gates required by the highest Change Risk Level in the epic are green.

`scripts/project_os.py` enforces the first two, `scripts/check_process.py` the third, and
`scripts/release.py check` refuses to let a closed epic sit unreleased.

## Release evidence packet

A canonical release records:

- source/tag/commit;
- included story/ADR/RFC IDs;
- risk distribution;
- G1-G4 results;
- CR4 Safety Verdict links;
- compatibility matrix;
- benchmark summary;
- dependency/security scan summary (for the Rust workspace: `cargo deny check` - licenses, sources, bans, RustSec advisories - `rust/deny.toml`, ADR-0015);
- SBOM/provenance/signature/attestation references;
- installer smoke results;
- known residual risks and rollback instructions.

## Emergency security fixes

Emergency does not mean bypass safety. It may reduce ceremony by using a narrowly scoped CR4 patch and expedited independent verification, but the invariant-specific regression test and Safety Verdict remain required before public release when the fix changes destructive behavior.
