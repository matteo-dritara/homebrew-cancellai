# Evidence Packet - E22-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E22 epic review round 1
- Change Risk: CR2
- Spec version/commit: `docs/adrs/0019-dependency-rings-per-crate.md`, `docs/audits/2026-09-03-CODE_REVIEW.md` (`CR-TE-07`)

## Outcome

PASS

## Scope

`cancellai-cli` parsed its own arguments by hand in `main.rs`: no `--help`/`-h`/`--version` at
all (each exited `2` with `unrecognized flag`), and every command accepted every flag
regardless of relevance (`status --dry-run` silently did nothing instead of being refused).
ADR-0019 (already `Accepted`, written ahead of this story - see its own history) defines the
dependency-ring boundary that makes fixing this with a real parser (`clap`) a reviewed
decision rather than an ad hoc exception to the workspace's zero-dependency default.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - ADR-0019 defines the kernel/outer ring boundary | Already `Accepted` in the repository (`docs/adrs/0019-dependency-rings-per-crate.md`, dated 2026-09-03) - no change needed. `AGENTS.md`'s dependency-rule paragraph was still the pre-ADR uniform text (ADR-0019's own "Neutral / follow-up" note: "`AGENTS.md` must state both rules; today it states one"); updated to state both rings explicitly. | PASS |
| AC2 - `--help`/`-h`/`--version`/per-command help, parsing no longer hand-rolled | New `cli.rs` module using `clap` (`Parser`/`Subcommand`/`Args`/`ValueEnum` derive). `main.rs`'s hand-rolled `split_command`/`parse_flags`/`parse_u32` are deleted; `run()` now calls `cli::parse`. Manually verified: `cancellai-cli --help`, `-h`, `--version`, and `<command> --help` for all six subcommands. | PASS |
| AC3 - irrelevant flags rejected, SI-007 preserved | Each subcommand has its own `clap::Args` struct (`ReadOnlyArgs` for status/inspect/plan, `CleanArgs`, `ConfigureArgs`) - a flag not in that struct is a hard parse error (exit 2), not silently accepted. `cli::normalize_args` is the *only* place a subcommand is selected: no argument or a leading flag always resolves to `"status"`; the literal token `"clean"` is the only way to reach `Commands::Clean`. Verified by a mutation spot check (below) and 6 new unit tests. | PASS |
| AC4 - every added dependency's license is already allow-listed | `clap` (MIT OR Apache-2.0) is the one dependency added, named in ADR-0019 itself as the first outer-ring application; both licenses are already in `rust/deny.toml`'s allow-list. `cargo deny check` passes with the new dependency graph (transitive deps: `clap_builder`, `clap_lex`, `anstream`, `anstyle*`, `strsim`, `colorchoice`, `utf8parse`, `is_terminal_polyfill` - all MIT/Apache-2.0/MIT-0, no allow-list widening needed). | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-007 (ambiguous CLI/configuration is non-destructive) | Mutation spot check: changed `normalize_args`'s injected default from `"status"` to `"clean"` (simulating a defect that would let a bare/ambiguous flag resolve toward the mutating command) | `cargo test -p cancellai-cli` immediately failed `cli::tests::a_bare_flag_normalizes_to_status_with_the_flag_preserved` (`left: ["clean", ...], right: ["status", ...]`) - reverted after confirming. Also: an unrecognized subcommand (new test, `an_unrecognized_subcommand_is_refused_with_exit_code_2_and_never_runs_anything`) and an ambiguous bare flag (pre-existing `an_unrecognized_flag_is_refused_with_exit_code_2...`) both still exit 2 and touch nothing. | PASS |

Structural argument, unchanged by this story: `cancellai_safety::authority::effective_authority`
still independently constrains `user_requested` alongside artifact-ceiling/confidence/
lifecycle/provider-trust/constitutional-floor, so even a CLI defect that resolved ambiguity to
a high requested authority could not by itself grant destructive authority (per this story's
own Safety Obligations note in the executor brief). Full closure of SI-007 at the CLI layer
(refusing to resolve genuinely ambiguous input to *any* non-`Observe` `user_requested` at all)
remains out of this story's scope, as stated.

## Verification Commands

Golden CLI snapshots for help/version (VC1 - macOS, this executor's platform; Linux/Windows
are exercised by the same `tests/cli_behavior.rs` suite in `rust.yml`'s `quality` job matrix
and `release.yml`'s new `verify-rust` job, both tri-platform, per E22-S01):

```text
$ ./target/debug/cancellai-cli --help
Safely reclaim disk space from old Codex and Claude Code sessions (target-engine beta).
Usage: cancellai-cli <COMMAND>
Commands: status, inspect, plan, clean, configure, version, help
Options: -h/--help, -V/--version
exit=0

$ ./target/debug/cancellai-cli --version
cancellai-cli 0.1.0
exit=0

$ ./target/debug/cancellai-cli status --help    # and inspect/plan/clean/configure/version
Usage: cancellai-cli status [OPTIONS]
--days, --keep-latest, --tool, --json, --allow-running, -h/--help
exit=0

$ ./target/debug/cancellai-cli status --dry-run
error: unexpected argument '--dry-run' found
exit=2

$ ./target/debug/cancellai-cli frobnicate
error: unrecognized subcommand 'frobnicate'
exit=2

$ ./target/debug/cancellai-cli configure --claude-retention 0
error: invalid value '0' for '--claude-retention <DAYS>': 0 is not in 1..=4294967295
exit=2
```

Committed regression tests:

- `cancellai-cli/src/cli.rs::tests` (6 unit tests) - `normalize_args`'s own contract, plus
  `clap_command_graph_is_well_formed` (`Cli::command().debug_assert()`, clap's own
  self-consistency check).
- `cancellai-cli/tests/cli_behavior.rs` (6 new integration tests, spawning the real binary):
  `top_level_help_lists_every_command_and_exits_zero`,
  `top_level_short_help_flag_behaves_like_the_long_form`,
  `top_level_version_flag_prints_the_crate_version_and_exits_zero`,
  `every_subcommand_has_its_own_help_text`,
  `an_unrecognized_subcommand_is_refused_with_exit_code_2_and_never_runs_anything`,
  `a_flag_irrelevant_to_the_chosen_command_is_refused_not_silently_accepted`.

VC2 - all 24 pre-existing `cli_behavior.rs` tests pass unchanged (30 total now, same file):

```text
$ cargo test -p cancellai-cli --test cli_behavior
test result: ok. 30 passed; 0 failed
```

Including, by name, the three the story calls out specifically: bare flags
(`an_unrecognized_flag_is_refused_with_exit_code_2_and_never_partially_runs`), `--json`
without explicit intent (`clean_json_without_yes_or_dry_run_is_refused_before_touching_anything`);
"unknown subcommands" had no dedicated pre-existing test (added in this story, see above) -
noted as a residual below.

VC3:

```text
$ cargo deny check
advisories ok, bans ok, licenses ok, sources ok
```

Full local gate set:

```text
cargo fmt --check                                                   clean
cargo clippy --workspace --all-targets --all-features -- -D warnings   clean
cargo check -p cancellai-cli --all-targets                           clean
cargo test --workspace                                               387 tests, all passed
cargo deny check                                                     advisories ok, bans ok, licenses ok, sources ok
python3 scripts/check_rust_workspace.py check                        OK
python3 scripts/check_mutation_boundary.py check                     OK (53 files scanned, +1 for cli.rs)
python3 scripts/check_provider_compatibility.py check                OK
python3 scripts/rust_python_parity.py self-test / check               OK, 12 NORMATIVE fixtures
python3 -m pytest tests -v                                            184 passed
python3 -m ruff check . / ruff format --check                        clean
python3 scripts/check_docs.py check                                  OK
python3 scripts/check_workflows.py check                             OK
python3 scripts/check_process.py check                                OK (pre-existing E00/E07 exceptions only)
python3 scripts/release.py check                                     OK
```

## Compatibility

- **Behaviour tightening, disclosed, not silent**: a flag irrelevant to a command now errors
  instead of being silently accepted-and-ignored (AC3's own requirement). Any external
  scripting against the beta CLI that relied on the old permissive behaviour (e.g. passing
  `--dry-run` to `status` out of habit) would now see exit 2 where it previously saw 0. The
  beta CLI carries no compatibility guarantee yet (`docs/RELEASING.md`'s "Beta side-by-side"
  section: "not yet the canonical CLI"), so this is judged acceptable and is the story's
  intended fix, not an oversight.
- No change to `status`/`inspect`/`plan`/`clean`/`configure`/`version`'s actual semantics,
  JSON document schemas, or exit-code taxonomy for valid invocations.

## Performance / operability

- Not applicable - CLI parsing only, no change to the discovery/mutation path.

## Documentation updated

- `docs/CLI_RUST.md` - "Known gaps" entry for `CR-TE-07` replaced with a new "Argument
  parsing" section describing the `clap`-based surface and the one remaining hand-written
  piece (subcommand selection, for SI-007); shared-flags table corrected for `--json`'s real
  scope.
- `AGENTS.md` - states both dependency-ring rules (ADR-0019's own follow-up item), replacing
  the single uniform "no dependency" rule that predated the ring boundary.
- `CHANGELOG.md` - Added/Changed entries under `[Unreleased]` for the new help surface and
  the flag-rejection tightening.
- `docs/adrs/0019-dependency-rings-per-crate.md` - already `Accepted`; no change required by
  this story beyond what AC1 already found true.

## Residual risks

- **Tri-platform golden snapshots are not executed by this evidence packet directly** - this
  executor's environment is macOS only (the same limitation E22-S01/E02-S02 already recorded
  for Windows-specific failures). The new tests are committed into `tests/cli_behavior.rs`,
  which already runs in `rust.yml`'s tri-platform `quality` matrix and will run in
  `release.yml`'s new `verify-rust` job (E22-S01); the next real CI run is the empirical
  confirmation.
- **No pre-existing "unknown subcommand" test existed** despite the story's Verification
  Contract referring to it as one of the "existing SI-007 tests" - it did not exist before
  this story; added here (`an_unrecognized_subcommand_is_refused_with_exit_code_2_and_never_
  runs_anything`) rather than left as a gap.
- SI-007's full closure at the CLI layer (never resolving genuinely ambiguous input to any
  non-`Observe` `user_requested` value) remains explicitly out of scope, per the story's own
  Safety Obligations note - unchanged by this story.
- `clap`'s auto-generated `help` subcommand (`cancellai-cli help <command>`) is additional
  surface beyond what the reference CLI or this story's AC explicitly asked for; it is
  harmless (read-only, prints and exits 0) and not separately tested, since it is clap's own
  well-tested behaviour, not code this repository wrote.

## Verifier verdict

pending
