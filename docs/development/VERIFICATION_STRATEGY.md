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
  quietly collapse;
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
