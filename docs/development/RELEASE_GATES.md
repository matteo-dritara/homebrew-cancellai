# Release Gates

A release is eligible only when the gates required by its changes are green. The project distinguishes feature completion from safety evidence.

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

### Performance budget baseline (`cancellai-inventory`, E04-S04)

`rust/crates/cancellai-inventory/tests/performance_micro.rs` is a CI-friendly regression
ceiling (a few thousand synthetic files, runs on every `cargo test`) that catches a gross
traversal regression without being a tight SLA. The heavy 10k/100k(/1M-on-demand)-entry
benchmarks live in `tests/performance_scheduled.rs`, `#[ignore]`d out of the default test run
and executed weekly (plus on-demand) by `.github/workflows/rust-benchmark.yml`, which uploads
a machine-readable JSON trend artifact (`CANCELLAI_BENCH_OUTPUT`) - the "benchmark summary"
release-evidence item below, once a release references it. Latency/throughput thresholds are
recorded in that file's `THRESHOLDS` table, generously bounded (regression detection, not a
tight SLA) given shared-runner variance.

Only latency and throughput are actually measured today. Peak memory, CPU, and cancellAI's
own runtime self-footprint are recorded here as forward-looking budgets, not yet measured:
- **Peak memory**: target is O(one `FileFacts` per observed path) for a single scope scan - no
  profiling/memory-accounting dependency exists in this workspace yet to verify this
  automatically (AGENTS.md: do not add a dependency merely to reduce implementation effort).
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
rewritten from scratch each time. As of E06-S03 (E06-S01/S02/S03 committed, `ready_for_review`,
not yet independently verified):

**G1 Functional - not ready.** Core `status`/`inspect`/`plan`/`clean`/`configure`/`version`
surface exists with matching JSON schemas and exit-code taxonomy (E06-S01), and a differential
gate confirms parity with the Python reference on the full `NORMATIVE` fixture corpus
(E06-S02). Disclosed gaps, tracked in `docs/CLI_RUST.md`'s "Known gaps" section: no
`--aggressive` (legacy/cache category widening), no `status --paths/--coverage/--top`, no
`clean --keep-claude-history`/`--verbose`, no deletion of a session's companion payload
directory (only the session file itself - `mutation_executor` has no directory-tree deletion
path yet), no Codex-side incomplete-scan detection (a directory-read failure during rollout
discovery is silently skipped rather than surfaced, unlike the Claude-side fix E06-S02 made -
`cancellai-provider-codex`, E05-S04, pre-existing gap outside E06's own crates).

**G2 Safety - not ready.** SI-007/SI-008/SI-009/SI-019/SI-020/SI-021/SI-022 are exercised by
targeted unit/integration tests (E06-S01/S02 evidence packets). Missing for CR4: the
independent verifier's own adversarial pass (this section, and every claim in it, is executor
self-assessment - `AGENT_PROTOCOL.md` is explicit that a verifier does not treat executor
tests as proof), and the owner-visible Safety Verdict itself, which this document cannot
substitute for.

**G3 Compatibility - not ready.** The tier-1 CI matrix (`rust.yml`) reached this crate's
mutation-path integration tests on Windows for the first time on 2026-09-01 (an unrelated
pre-existing clippy failure had aborted the quality job before them on every prior run, the
same pattern E07-S05/E20-S04 already document) and found a real, concrete blocker, not merely
missing confirmation: `cancellai-platform::identity::SystemIdentityObserver` reports
`IdentityObservation::Unsupported` unconditionally on non-Unix platforms (E03-S01's own
disclosed residual risk), so `ApprovedRoot::establish`/`bind` fails closed and a real deletion
can never succeed on Windows today - correctly, not silently, but it does mean `clean --yes`
cannot pass tier-1 Windows CI until E20-S01 ("Windows native backend", moved from E07 into a
dedicated Windows/WSL epic pending real environment access) lands. The affected tests are now
`#[cfg(unix)]`-scoped rather than left red, and `docs/PROVIDERS.md`'s generated compatibility
matrix covers the provider adapters' own capabilities, not the CLI command surface E06-S01-S03
added. **This is a concrete addition to E06-S04's own blocker list, beyond the G4/E17
prerequisite cycle already recorded below**: tier-1-clean cutover requires E20-S01, not only
E17's packaging work.

**G4 Operability - not ready.** No packaged installer exists (Epic E17 scope, `docs/RELEASING.md`
"Target Rust release factory"); no performance/self-budget measurement exists for the CLI's
own command paths (only `cancellai-inventory`'s scan performance is benchmarked,
`RELEASE_GATES.md`'s own "Performance budget baseline" section above); no crash/recovery
testing beyond what unit tests exercise.

**Conclusion**: cutover is not recommended at this time. Closing E06-S04 (and E06 as a whole)
requires this checklist to read "ready" against real evidence, an independent CR4 verifier
pass, and the owner's own Safety Verdict acceptance - none of which the executor grants itself
(`AGENT_PROTOCOL.md`: "an executor's work is finished at `ready_for_review`... it does not
write its own Safety Verdict").

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
