# Evidence Packet - E17-S04

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR1
- Spec version/commit: `project/epics/E17.json` (E17-S04), as of this change

## Outcome

PASS

New product capability targets the Rust implementation per AGENTS.md's Python reference
freeze, so this lands in `cancellai-cli` (`rust/crates/cancellai-cli`), not `cancellai.py`.

`rust/crates/cancellai-cli/src/install_source.rs` (new module) detects installation source from
the running binary's own resolved executable path - `Homebrew` (`Cellar` path, checked first,
platform-independent since Linuxbrew uses the same layout), `WindowsPackage`, `LinuxPackage`,
`DirectDownload`, or `Unknown` (fails closed to `Unknown` rather than guessing, C-02) - and
carries each variant's own upgrade guidance as a pure function of the variant alone (so nothing
can route one source's guidance through another's mechanism).

Two CLI surfaces expose it, both purely observational (CR1, no mutation path added or touched):

- `cancellai-cli update --check` (new command) - prints version, detected source, and its
  guidance. A bare `update` with no `--check` is refused with exit 2 (SI-007: ambiguity never
  escalates - a bare invocation does not silently mean "check", it means "refused").
- `cancellai-cli version --source` (new flag) - appends the same information to `version`'s
  output. The bare `version` invocation's exact stdout is an existing, committed golden
  contract (`tests/cli_behavior.rs::top_level_version_flag_prints_the_crate_version_and_exits_
  zero` and the renamed/added exact-match tests below); `--source` is additive-only and does
  not touch that first line, proven by a new test that reuses the exact same format string.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - `cancellai update --check` never silently switches package channels | `install_source::InstallSource::upgrade_guidance` is a pure match on `self` with no other input, so there is no code path that could substitute a different source's text; `install_source::tests::each_sources_guidance_names_no_other_sources_own_mechanism` asserts each source's guidance text contains none of the other sources' mechanism keywords ("brew", "Windows package", "Linux system package", "direct download"); `cli_behavior.rs::bare_update_without_check_is_refused_not_silently_a_no_op` proves a bare `update` is refused (exit 2, stderr names `--check`), not silently treated as a check | PASS |
| AC2 - Source is inspectable in version/status output | `cancellai-cli version --source` (new); `cli_behavior.rs::version_source_appends_install_source_without_changing_the_first_line` proves it prints `install_source:`/`upgrade:` lines while the first line stays exactly `cancellai-cli {VERSION}\n`; `cli_behavior.rs::version_bare_output_is_byte_identical_whether_or_not_source_support_exists` re-proves the pre-existing golden contract for the bare invocation was not disturbed | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - `safety_obligations: []`, CR1. The one relevant
constitutional property is SI-007 (ambiguity never escalates toward mutation), addressed by
requiring `--check` explicitly rather than accepting a bare `update` as an implicit no-op or,
worse, a future silent default; `bare_update_without_check_is_refused_not_silently_a_no_op`
covers it. `update --check` performs no filesystem write of any kind - confirmed by
`update_check_reports_version_and_install_source_and_never_mutates`, which plants a synthetic
stale Claude session before the call and asserts it still exists afterward.

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_provider_compatibility.py check
python3 scripts/rust_python_parity.py self-test
python3 scripts/rust_python_parity.py check
python3 scripts/check_platforms.py check
python3 scripts/check_docs.py check
python3 -m pytest tests -v
```

All PASS locally. `cargo test -p cancellai-cli --bin cancellai-cli` (unit tests, includes
`install_source`'s own module): 24/24. `cargo test -p cancellai-cli --test cli_behavior`
(integration, real built binary): 38/38, including 7 new tests
(`version_bare_output_is_byte_identical_whether_or_not_source_support_exists`,
`version_source_appends_install_source_without_changing_the_first_line`,
`update_check_reports_version_and_install_source_and_never_mutates`,
`bare_update_without_check_is_refused_not_silently_a_no_op`,
`update_rejects_unrecognized_arguments`, plus `update` added to the table-driven
`every_subcommand_help_matches_its_committed_golden_snapshot`) and two updated golden snapshots
(`version_help.txt`, `top_level_help.txt`) plus one new one (`update_help.txt`), all captured
from the real built binary's own `--help` output, not hand-transcribed.
`scripts/check_mutation_boundary.py` confirms this story added a source file
(`install_source.rs`) without adding any new deletion capability - the reported deleting-file
set is unchanged from before this story.

## Compatibility

- Platform detection branches on `cfg!(windows)`/`cfg!(target_os = "linux")`/
  `cfg!(target_os = "macos")` at compile time, matching this crate's existing cross-platform
  pattern; `#[cfg(target_os = "...")]`-gated unit tests exercise the Linux/Windows branches on
  their respective tier-1 CI legs (this session's host is macOS, so those specific tests do not
  compile/run here - the Homebrew and macOS-`DirectDownload` tests do, and pass).

## Performance / operability

- `detect_from_path` is a handful of string comparisons on an already-resolved path; no I/O
  beyond the pre-existing `std::env::current_exe()` call.

## Documentation updated

- `docs/CLI_RUST.md` - new `update` command section, `version --source` documented under the
  existing `version` section.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- Detection is a path heuristic, not a query to a real package database - correct today because
  no package channel exists yet for `cancellai-cli` (E17-S02's pipeline produces archives, not
  packages). `DirectDownload`/`Unknown` are the only reachable results in practice until a real
  package channel exists; the `Homebrew`/`WindowsPackage`/`LinuxPackage` branches are exercised
  by unit tests against synthetic paths, not yet against a real installed package (there is
  none to test against).
- `status`'s JSON document (the `JSON_CONTRACTS.md` inventory schema) does not carry an
  `install_source` field - deliberately: extending a versioned cross-engine document schema for
  this was judged higher blast radius than adding it, and `version --source` already satisfies
  the AC's literal "version/status output" wording without touching that contract.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
