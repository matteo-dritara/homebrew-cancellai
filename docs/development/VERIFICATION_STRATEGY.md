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

### Provider contract fixtures

Privacy-safe fixture corpus for every supported provider/version/layout. Each fixture declares expected capabilities and normative behavior.

### Golden tests

Used for stable machine contracts: inventory/plan/result JSON, explanation traces, CLI/TUI view models. Golden output is appropriate only when semantically reviewed; snapshots are not automatically truth.

### Differential tests

During Python->Rust migration, run both engines on the same normative fixtures and compare normalized semantic output. Known Python defects are explicitly non-normative.

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
