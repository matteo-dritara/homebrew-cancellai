# Verification Strategy

cancellAI verifies behavior at multiple layers because its dominant failure mode is not merely a crash; it is incorrect authority over user data.

## Test layers

### Pure domain tests

Fast exhaustive/table/property tests for:

- authority lattice;
- lifecycle transitions;
- policy precedence;
- risk/reversibility/confidence mapping;
- deterministic explanations;
- schema/version compatibility.

### Filesystem integration tests

Synthetic temporary trees exercise:

- files/directories;
- symlinks;
- mount/volume abstractions;
- permissions;
- disappearing objects;
- identity replacement;
- huge/sparse files where platform permits;
- Windows junction/reparse behavior.

Tests never operate on the real user provider roots.

#### Rust: deterministic clock/filesystem seams (E02-S04)

`rust/crates/cancellai-platform` (`Clock`, `FsObserver`) is how the Rust target achieves the
above without weakening what a filesystem integration test can observe:

- production code takes `&dyn Clock`/`&dyn FsObserver` and is wired to `SystemClock`/
  `SystemFsObserver` - real OS-backed implementations, never abstracted away;
- tests take the same trait objects and use `FrozenClock`/`SyntheticFsObserver` instead, so a
  time- or filesystem-dependent test is reproducible without mocking away the semantics that
  matter for safety - `FsObserver::observe` keeps the absent-vs-unreadable distinction
  `docs/architecture/AS_IS.md`'s `Scan`/`observe()` established for the Python reference
  (SI-008, SI-009, SI-010) as a typed contract (`Observation::Absent` vs
  `Observation::Unreadable`), not an implementation convention that a future call site could
  quietly collapse; this extends to a single fact within otherwise-readable metadata - if
  `SystemFsObserver` cannot obtain or represent a modification time (the platform can't
  report `mtime`, or the value predates the Unix epoch and overflows `Timestamp`'s
  seconds-since-epoch encoding), it reports `Observation::Unreadable` rather than
  substituting `Timestamp::EPOCH`, so a genuinely unknown fact can never read as a credible
  1970 timestamp to a retention/planning caller (E02 verifier review round 1, E02-S04);
- a **determinism test** (`rust/crates/cancellai-platform/tests/determinism.rs`) proves two
  independent runs against the same frozen clock reading and synthetic filesystem facts
  produce byte-identical serialized output - and that changing either input changes the
  output, so the equality check is falsifiable, not vacuous. This is the pattern real plan
  generation (E03 safety kernel, E04 inventory engine) will reuse once it exists; the current
  test exercises the seam composition itself via a minimal stand-in (`Snapshot`), not a
  finished plan builder.

### Provider contract fixtures

Privacy-safe fixture corpus for every supported provider/version/layout. Each fixture declares expected capabilities and normative behavior.

### Golden tests

Used for stable machine contracts: inventory/plan/result JSON, explanation traces, CLI/TUI view models. Golden output is appropriate only when semantically reviewed; snapshots are not automatically truth.

Inventory/plan/explanation/result document shapes are specified in [`../architecture/JSON_CONTRACTS.md`](../architecture/JSON_CONTRACTS.md); worked examples live in [`../../tests/fixtures/schemas/golden/`](../../tests/fixtures/schemas/golden/) and are enforced by `scripts/check_schemas.py`.

### Differential tests

During Python->Rust migration, run both engines on the same normative fixtures and compare normalized semantic output. Known Python defects are explicitly non-normative.

#### Differential comparison contract

`scripts/diff_harness.py` compares two [`JSON_CONTRACTS.md`](../architecture/JSON_CONTRACTS.md)-shaped documents of the same `document_type` (a Python-reference output and a Rust-candidate output, in the eventual dual-engine setup) and reports every semantic divergence. Two rules bound what "divergence" means, matching E01-S05's acceptance criteria:

- **Only explicitly documented fields are ignored.** `generated_at`, `generator`, and every top-level opaque engine-assigned id (`inventory_id`, `plan_id`) are never compared - they are expected to differ between any two runs, let alone two engines. Everything else is compared.
- **Records are paired by natural key, never by opaque id.** Before comparing, each list of records (`artifacts`, `provider_roots`, `scan_completeness`, `actions`, `explanations`, `action_results`) is matched between the two documents using a key derived from stable content, not from an engine-assigned id:
  - `inventory.artifacts` match on `identity_token` (never `artifact_id` - see [`JSON_CONTRACTS.md`](../architecture/JSON_CONTRACTS.md#inventory-document));
  - `inventory.provider_roots` match on `provider_id`;
  - `inventory.scan_completeness` match on `scope`;
  - `plan.actions` match on `(target_artifact_ids resolved to identity_token, action_class)` - resolving `target_artifact_ids` requires the caller to supply an `artifact_id -> identity_token` index built from that side's own inventory document; `action_id` and the raw `target_artifact_ids` are then dropped before comparing the matched pair's remaining fields;
  - `explanation.explanations` and `result.action_results` match on that *same* `plan.actions` key, resolved via `action_id -> (target_artifact_ids resolved to identity_token, action_class)` built from the corresponding plan document. Comparing an explanation or result document therefore requires passing the plan document(s) that produced it (`plan_a`/`plan_b`) - there is no fallback to opaque `action_id` matching; omitting the plan context is a hard error, not a silent degradation. `action_id` itself is dropped before comparing the matched pair's remaining fields, same as `plan.actions`.
  - A record present in only one side's list is always a divergence; it is never silently dropped as "the other engine just didn't produce it."
- **Any remaining divergence fails** unless it is whitelisted by an accepted ADR/RFC recording it as `INTENTIONAL_DIVERGENCE` (the same four-value taxonomy `characterize.py` uses - see [Python reference contract](#python-reference-contract) below). A whitelist entry is a recorded decision, not a code path the comparator special-cases silently.

`scripts/diff_harness.py check` runs the module's own self-test suite: an identical document must compare clean, changing only a documented-ignored field must still compare clean, and each divergence class above (changed field, extra/missing record, mismatched `document_type`, an artifact or action matched correctly despite a renamed opaque id, explanation/result comparison refusing to run without plan context) must be caught. This is the harness self-test the story's verification contract names. E01-S05's round-one independent review found that an earlier version of this harness matched `explanation`/`result` records by opaque `action_id` directly, which reports two semantically identical records as diverged whenever only their engine-assigned ids differ - exactly the false-M6-failure case this contract exists to prevent; the plan-resolved key above, and the regression tests in `tests/test_diff_harness.py::DiffHarnessActionCorrelationRegressionTests`, close that finding.

### Adversarial tests

Required for CR3/CR4. The verifier tries to violate named invariants rather than simply increasing line coverage.

### Fault/crash tests

Inject failures between persistence/mutation stages to verify no silent partial-corruption assumptions, especially quarantine, restore, archive, metadata rewrite, and ledger updates.

### Fuzz/property tests

Particularly valuable for path normalization, policy parsing/resolution, manifest parsing, provider metadata parsing, and authority lattice. Corpus seeds include prior defects.

### Performance tests

Fast regression checks in ordinary CI; larger synthetic scans scheduled/benchmark jobs. Measure traversal count, latency, peak memory, CPU, and cancellAI state growth.

E04-S04 implements the split this section describes for `cancellai-inventory`:
`tests/performance_micro.rs` (ordinary CI, a few thousand synthetic files) and
`tests/performance_scheduled.rs` (`#[ignore]`d, 10k/100k/1M-entry datasets, run by
`.github/workflows/rust-benchmark.yml` on a schedule and via `workflow_dispatch`). See
[`RELEASE_GATES.md`](RELEASE_GATES.md#performance-budget-baseline-cancellai-inventory-e04-s04)
for which of traversal-count/latency/memory/CPU/self-footprint are actually measured today
versus recorded as forward-looking budgets.

### Installer/release tests

Fresh-machine/container/VM smoke tests for each tier-1 artifact/package channel where feasible, plus provenance/SBOM verification.

## Evidence hierarchy

Strong evidence is independent and behaviorally close to the risk:

1. property/invariant proof by construction or exhaustive model test;
2. adversarial integration test reproducing a real failure mode;
3. differential/provider fixture evidence;
4. ordinary unit/integration tests;
5. static analysis/lint;
6. manual claim.

Line coverage is diagnostic, not a release gate by itself.

## Python reference contract

Fixtures classify expected results as:

- `NORMATIVE` - must match in Rust;
- `INTENTIONAL_DIVERGENCE` - Rust differs according to accepted ADR/spec;
- `LEGACY_ONLY` - behavior retained only for reference compatibility testing;
- `KNOWN_DEFECT` - reproduction must not be copied.

## Test data rules

Never commit real transcripts, source code, auth data, API keys, or developer home paths. Synthetic fixture generators are preferred. Any captured vendor metadata must be scrubbed and reviewed.

Synthetic fixture policy and layout: [`tests/fixtures/README.md`](../../tests/fixtures/README.md).

## Corpus coverage is part of the gate (E21-S02)

The differential parity gate is worth exactly what the fixture corpus is worth. Its own
defect-injection self-test proves the *comparison* cannot silently swallow a divergence; it says
nothing about whether any fixture exercises a given invariant. The 2026-09-03 target-engine
review found the difference between those two claims expensive: the gate was green, its
self-test passed, and the engine was deleting artifacts the frozen reference withholds, because
no fixture placed an unreadable directory in a Codex tree (`CR-TE-01`/`CR-TE-03`).

Two rules follow, both enforced rather than intended:

- `scripts/check_fixtures.py` fails on an undeclared category asymmetry between the two
  reference providers, and on a declaration the corpus has outgrown.
- A fixture added to close a reproduced defect must **fail** the gate against the unrepaired
  engine before the repair lands. A fixture that passes on the unrepaired engine is not
  exercising the defect, whatever its name says. E21-S02's evidence packet records the failing
  run for both new fixtures, taken before E21-S03 existed.

## Direct coverage for a hand-translated resolver (E22-S04)

A rule ported from `cancellai.py` into Rust and verified only through the differential gate is
verified by the *corpus*, per the section above - a rule no fixture happens to exercise can
regress silently even with the gate green. `cancellai-policy/src/retention.rs`'s resolver
(the age cutoff, keep-latest applied per subagent tree rather than per file, the interaction
between pinning and protection, process liveness, tool scoping) now carries direct unit tests
in addition to the fixture corpus, each named after the specific reference behaviour it pins
(`cancellai.py::choose_old_sessions`/`choose_codex_old_sessions`) so a future divergence reads
as a named, specific failure rather than an opaque differential mismatch.

Two rules make this more than documentation intent:

- boundary cases are named explicitly rather than left to whatever the fixture corpus happens
  to cover: `keep_latest=0`, `keep_latest` above the session count, an unobservable mtime, and
  a subagent tree whose members disagree in age;
- a test's claim to pin a rule is checked, not assumed - inverting the age-cutoff comparison
  (`<` to `<=`) and dropping the tree grouping (each session becomes its own singleton "tree")
  were both run locally against the test suite before this story closed, and each broke exactly
  one named test, not only the differential gate (`project/evidence/E22-S04/EVIDENCE.md`).

`cargo llvm-cov -p cancellai-policy --lib` reported 95.58% line coverage of `retention.rs` at
the time this story closed - the figure reached, not a target this or a future story is bound
to hold exactly; new resolver logic should carry its own direct tests rather than chase a
percentage.
