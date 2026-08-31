# Spec-First Python -> Rust Migration

## Why not rewrite immediately

The Python implementation contains valuable behavior and hard-won safety cases. A clean-slate rewrite without an oracle risks losing them. Conversely, continuing to place new product capability into the monolith would harden the wrong architecture.

The migration therefore treats Python as a temporary executable reference.

## Sequence

### M0 - P0 safety repair

Implement E00 only. No new product capability.

Exit: owner-visible Safety Verdict for the P0 trust floor.

### M1 - Extract contract

Implement E01:

- canonical vocabulary ([`DOMAIN_MODEL.md`](../architecture/DOMAIN_MODEL.md));
- synthetic provider/adversarial fixtures ([`tests/fixtures/`](../../tests/fixtures/));
- versioned plan/result contracts ([`JSON_CONTRACTS.md`](../architecture/JSON_CONTRACTS.md));
- Python characterization - `scripts/characterize.py` records what `cancellai.py` actually does on every fixture in the corpus and classifies it `NORMATIVE` / `INTENTIONAL_DIVERGENCE` / `LEGACY_ONLY` / `KNOWN_DEFECT` (see [Python reference contract](VERIFICATION_STRATEGY.md#python-reference-contract)); committed records live under [`tests/fixtures/characterization/`](../../tests/fixtures/characterization/) and `scripts/characterize.py check` proves they are still reproducible on a clean checkout;
- differential comparison rules.

Only fixtures classified `NORMATIVE` are binding on the Rust candidate at M6. A
`KNOWN_DEFECT` record exists precisely so that behavior is never copied forward as a
requirement merely because Python happens to do it; `INTENTIONAL_DIVERGENCE` and
`LEGACY_ONLY` need their own accepted spec/ADR before Rust may differ or drop the behavior,
per [Story changes during implementation](WORK_ITEM_MODEL.md#story-changes-during-implementation).

### M2 - Freeze Python

Python becomes maintenance-only (E01-S06). `AGENTS.md`'s "Python reference freeze" section is the visible marker; `scripts/check_process.py check` fails if it goes missing. Only parity fixes (matching the committed characterization), safety/security fixes, and migration-support tooling are accepted from here forward - not merely until this epic closes. New features target Rust.

### M3 - Bootstrap Rust

Create workspace, quality gates, typed model, filesystem seams, safety kernel, inventory, and provider contract.

### M4 - Reference-provider parity

Claude and Codex adapters must satisfy normative fixtures and compatibility evidence.

### M5 - CLI parity

Rust status/inspect/plan/clean semantics meet versioned CLI/JSON contracts. No TUI is required for cutover.

### M6 - Differential gate

Every normative fixture runs Python and Rust. An unexplained semantic divergence blocks cutover. Intentional differences require accepted architecture/spec documentation.

**E06-S02 implements the gate itself**: `scripts/rust_python_parity.py check` materializes
every `NORMATIVE`-classified fixture (`scripts/characterize.py`'s `CLASSIFICATIONS`) once and
runs `cancellai.py`'s `build_plan` and the built `cancellai-cli`'s `plan`/`inspect --json`
against the same synthetic tree, then compares the set of session UUIDs each engine would
delete plus whether the tool's scan was withheld. This compares at the semantic level both
engines can actually express - `docs/architecture/JSON_CONTRACTS.md` documents are a
target-engine-only contract `cancellai.py` was never changed to emit (see that document's own
"Compatibility policy"), so `scripts/diff_harness.py`'s JSON_CONTRACTS-vs-JSON_CONTRACTS
comparator is not the mechanism here; `rust_python_parity.py`'s own module doc explains why in
full, including the one-day timing margin it applies to avoid a whole-second-vs-float
timestamp-precision flake at an exact cutoff boundary (`cancellai-platform::Timestamp`'s
documented whole-second granularity versus `cancellai.py`'s float `time.time()`). Wired into
`.pre-commit-config.yaml` (`rust-python-parity-gate`) and `AGENTS.md`'s "Current Python checks"
list, run via the `governance.yml` `pre-commit` CI job. `rust_python_parity.py self-test`
proves the comparator itself can fail (an injected divergence in each of: extra candidate,
missing candidate, withheld-flag mismatch) before trusting it to pass. An
`INTENTIONAL_DIVERGENCES` allow-list exists in the script for a future accepted, cited
divergence; it is empty today - all ten current `NORMATIVE` fixtures match exactly.

Finding this gate immediately useful: running it against the just-implemented E06-S01 CLI
surfaced two real defects before any review round, both fixed in the same change - an
incomplete companion-payload scan only downgraded the one degraded session instead of
withholding the whole tool (SI-008/SI-009), and a `claude_home` with no `projects/` directory
was incorrectly treated as an incomplete scan rather than a legitimately empty one. Exactly the
kind of cross-engine divergence this gate exists to catch before cutover, not after.

### M7 - Beta side-by-side

Release candidate identifies engine/version clearly and preserves rollback. Local state migrations are reversible/rebuildable.

**E06-S03 implements this milestone's contract** for the current beta period - before Epic
E17's canonical cross-platform release factory exists (`docs/RELEASING.md` "Target Rust
release factory" remains E17 scope; this story does not build packaging/installers):

- **Engine/version identification**: `cancellai-cli version` prints the engine name and a
  concrete version (`rust/crates/cancellai-cli/tests/install_rollback.rs::
  version_output_identifies_this_as_the_rust_engine_with_a_concrete_version`).
- **No irreversible local state migration**: there is no cancellAI-owned local state to
  migrate in either engine today - `cancellai-store` remains an empty skeleton (C-10's
  "disposable and rebuildable" is trivially true of nothing), and `cancellai.py` itself is a
  stateless scan-on-demand script. Proven, not merely asserted: every read-only Rust command
  (including `clean --dry-run`) leaves the entire `$HOME` tree byte-for-byte unchanged, and a
  real `clean` touches exactly the one artifact it deletes and nothing else anywhere
  (`install_rollback.rs::every_read_only_command_leaves_no_trace_anywhere_under_home`,
  `::a_real_clean_touches_only_the_provider_artifact_it_deletes_nothing_else_anywhere`).
- **Rollback mechanism**: `cancellai` (Python, the installed Homebrew command,
  `pyproject.toml`) and `cancellai-cli` (Rust, this crate's package name) are different binary
  names that share no install path and no state file
  (`install_rollback.rs::the_rust_and_python_commands_never_collide_on_path`) - during beta,
  "rollback" is simply not invoking `cancellai-cli`, never a migration to undo.
- **Install smoke test**: the built binary behaves correctly regardless of its invocation
  working directory (`install_rollback.rs::
  the_built_binary_runs_correctly_regardless_of_its_invocation_directory`) - the property a
  real installed binary needs; full packaged-install verification (checksums, SBOM, signed
  provenance, `dist`/cargo-dist) remains E17.

### M8 - Canonical switch

Rust becomes stable only after G1 Functional, G2 Safety, G3 Compatibility, and G4 Operability gates are green and owner accepts the migration Safety Verdict.

## What not to preserve

The migration does not preserve accidental implementation constraints:

- one-file architecture;
- macOS-only assumptions;
- repeated filesystem traversals;
- path-only identity;
- implicit scanner-based protection;
- legacy ambiguous CLI normalization;
- any known defect reproduced by the P0 audit.

## Rollback

During transition, release artifacts/tags make the last Python release available. Because cancellAI current-state DB is not provider truth, a failed Rust beta can be removed/reset without migrating provider data backward.

## Completion

After a defined transition window, Python may remain in repository history or a `reference/` tag/branch rather than the active source tree. Do not maintain two production engines indefinitely.
