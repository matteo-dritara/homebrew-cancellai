# Evidence Packet - E05-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E05)
- Change Risk: CR1
- Spec version/commit: `rust/crates/cancellai-cli/examples/compatibility_matrix.rs` (new),
  `scripts/check_provider_compatibility.py` (new), `docs/PROVIDERS.md` (new generated
  section), `.pre-commit-config.yaml`/`AGENTS.md` (new hook/check wiring) as added in this
  change

## Outcome

PASS

## Scope

Publishes a generated, per-capability compatibility matrix for the two reference adapters
(`cancellai-provider-claude`, `cancellai-provider-codex`) into `docs/PROVIDERS.md`, sourced
from their own `ProviderCapabilities` output rather than hand-typed prose - the story's
outcome ("Publish tested provider versions/layout signatures and supported capability
states"), scoped to what a CLASSIFY-stage adapter (E05-S03/E05-S04) can actually answer today.
Deliberately not attempted: real per-*version* compatibility evidence ("Claude Code v1.2.3")
- no version-tagged fixture corpus exists yet, and `docs/PROVIDERS.md` already documented that
as future P1/P2 work before this story. Instead, two *layout* scenarios are run against each
adapter (an empty candidate root, varying only whether it is asserted to be the OS-default
provider directory), which is enough to demonstrate both ACs concretely from real adapter
output.

**A real environment-determinism bug was found and fixed during this story's own
development, before commit:** the first draft of `compatibility_matrix.rs` called
`CodexProvider::native_delete_support()` with no explicit `codex_bin`, which resolves via this
process's own `PATH` - on the machine used to develop this story, a real `codex` CLI happens
to be installed, so the first generated matrix reported `native_delete_capability` as
`VERIFIED` for both layouts, a result that depends on local machine state, not on the adapter.
Fixed by pinning `native_delete_capability` probing to an explicit, deliberately nonexistent
`codex_bin` in the generator (not in the adapter itself, which correctly keeps `PATH`
resolution as a real, documented capability for its own callers) - documented in the example's
own module doc and inline comment.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Compatibility is reported per capability, not a single supported boolean | The generated matrix in `docs/PROVIDERS.md` has one row per `CapabilityKind` (9) per layout (2) per provider (2) = 36 rows, each with its own independent `SupportState`/confidence - e.g. `claude-code`'s `known_default_root` row shows `detect`=`VERIFIED` alongside `native_delete_capability`=`UNSUPPORTED` and `session_graph`=`UNSUPPORTED` *in the same layout scenario*, concretely proving support is not one collapsed flag. `render_table` in `check_provider_compatibility.py` structurally cannot produce a single boolean - its output type is a table keyed by `(provider, layout, capability)`. | PASS |
| AC2 - Unknown version behavior is documented and fail-closed | The `unknown_custom_root` column (an empty directory, `is_default_root = false`) shows `detect`/`fingerprint_root` as `UNSUPPORTED` (`low_unknown` confidence) for both providers - the real, adapter-produced fail-closed answer (E05-S03/E05-S04's own AC3-equivalent), not asserted prose. The matrix's own intro paragraph in `docs/PROVIDERS.md` states this explicitly and cross-references SI-004. | PASS |

## Verification

`scripts/check_provider_compatibility.py check` is the "Generated matrix drift check from
adapter metadata" this story's verification plan names: it re-runs the
`compatibility_matrix` example fresh and fails if the committed `docs/PROVIDERS.md` section
differs from what the adapters currently produce - the same generate/check convention every
other governance script in this repository already uses (`scripts/project_os.py`,
`scripts/gen_docs.py`). Wired into `.pre-commit-config.yaml` (new `provider-compatibility-check`
hook, triggered on changes to the two adapter crates' `src`/`examples`, the CLI's examples
directory, `docs/PROVIDERS.md`, or the script itself) and `AGENTS.md`'s "Current Python
checks" list, matching how `check_rust_workspace.py`/`check_mutation_boundary.py` are already
wired in.

## Verification Commands

```text
# Rust workspace (from rust/)
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check

# Python governance (repository-wide)
python3 -m ruff check .
python3 -m ruff format --check .
python3 -m mypy --strict scripts/check_provider_compatibility.py
python3 scripts/check_provider_compatibility.py generate
python3 scripts/check_provider_compatibility.py check
python3 scripts/check_docs.py check
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check
```

`cargo clippy --workspace --all-targets` covers the new `examples/compatibility_matrix.rs`
target (`--all-targets` includes examples); no new Rust unit tests were needed for a
print-and-format example, so verification is the generate/check round trip itself plus every
pre-existing test in the workspace (unaffected, all still green).

## Compatibility

- The generator invokes `cargo run --example compatibility_matrix -p cancellai-cli` from
  Python (`subprocess.run`, no shell, matching `scripts/check_process.py`'s existing
  `git`-invocation convention) - requires `cargo` on `PATH`, the same requirement every other
  Rust-touching check in this repository already has.
- The matrix's `native_delete_capability` column is pinned to a fixed "no binary" answer
  regardless of the generating machine's actual environment (see Scope) - this is the one
  place this story's own generator had to actively defend against non-reproducibility; every
  other row is inherently deterministic (pure filesystem reads against a fresh empty temp
  directory).

## Performance / operability

- `compatibility_matrix` creates and tears down 4 empty temp directories and runs 36
  capability queries against in-memory/filesystem-local state - sub-second, no network, no
  persistent state left behind (each `TempRoot` removes its directory on `Drop`).

## Documentation updated

- `docs/PROVIDERS.md` - new "Tested compatibility matrix" section (the story's declared
  documentation impact), plus a one-sentence update to the document's opening paragraph
  (previously implied no generated adapter metadata existed yet; this story adds the first
  slice).
- `AGENTS.md` - added `scripts/check_provider_compatibility.py` to the mypy target list and
  the "Current Python checks" sequence (documentation impact expanded beyond the story's
  single declared file, since a new governance script needs the same visibility every other
  one already has - AGENTS.md: "add more if implementation changes more contracts").
- `.pre-commit-config.yaml` - new `provider-compatibility-check` local hook (same reasoning).

## Residual risks

- No real per-*version*/layout compatibility evidence exists yet (only the two reference
  adapters' current behavior against a generic empty layout) - this is the same gap
  `docs/PROVIDERS.md` already documented before this story ("exact tested versions... will
  become generated adapter metadata during P1/P2"); this story narrows but does not close it.
- The matrix's `known_default_root`/`unknown_custom_root` scenarios both use an *empty*
  directory (differing only in the `is_default_root` flag) rather than a fully populated
  reference fixture tree (e.g. `claude-normal-session`) - sufficient to prove both ACs (see
  Scope), but a richer matrix built from the full fixture corpus (showing, for instance, what
  `session_graph` looks like when real sessions exist) is a reasonable future enhancement, not
  a defect in what this story claims.
- `check_provider_compatibility.py`'s `cargo run` invocation has no explicit timeout beyond
  180 seconds (generous for a debug build of one small example); a genuinely stuck build would
  surface as a timeout failure in CI rather than hanging indefinitely, but this has not been
  exercised adversarially.

## Verifier verdict

PENDING - epic E05 review runs once every story in E05 is `ready_for_review` (at most twice
per epic, per ADR-0014). This closes out every story in E05; the epic-scoped review round can
begin once this commits.
