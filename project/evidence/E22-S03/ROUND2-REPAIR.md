# E22-S03 - Round 2 repair (independent verifier review round 1 findings)

- Story: E22-S03
- Round: repair after `project/evidence/E22-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04

## Verdict this repairs

Round 1 verdict: FAIL, two distinct findings:

1. `cancellai-cli status --help --dry-run` exits 0 and prints help, never validating
   `--dry-run` - clap's help action short-circuits the moment it is matched, regardless of
   what else is on the command line.
2. The "golden" CLI tests only asserted a usage-prefix/command-name substring, so many real
   output regressions would still pass; this does not meet the Verification Contract's own
   "golden CLI snapshot" requirement.

## What changed

**Finding 1 - resolved by amending the story contract, per the review's own stated remedy**
("or amend the story contract explicitly if help is intended to be an exception"), not by
fighting clap's parser:

- `project/epics/E22.json` AC3 now states the exception explicitly: `--help`/`-h`/`--version`
  always short-circuit remaining validation, matching clap's own precedence and the same
  convention `git`/`cargo` follow (`git commit --help --bogus-flag` shows help too). This is
  safe by construction, not merely by convention: `cli::parse` only returns an `Invocation`
  when clap neither printed help/version nor errored, so no code path from `--help`/`-h`/
  `--version` can ever reach `main.rs`'s dispatch, in particular never `Invocation::Clean`.
  Disabling clap's built-in help/version handling and hand-rolling strict pre-validation was
  considered and rejected: it would reintroduce exactly the kind of hand-written,
  can-drift-from-what's-actually-accepted parsing this story replaced clap to get away from,
  for a UX property (help always available) every reference CLI already treats as correct.
- `docs/CLI_RUST.md`'s "Argument parsing" section states the exception and its safety
  argument in the same terms.
- `rust/crates/cancellai-cli/tests/cli_behavior.rs` gained
  `help_short_circuits_remaining_argument_validation_by_design` (locks the exception in as
  intentional, not a latent bug) and `an_irrelevant_flag_before_help_is_still_refused` (locks
  in the mirror case - an irrelevant flag *before* `--help` is still a usage error, since clap
  parses left to right).

**Finding 2 - resolved with real exact golden snapshots**:

- `rust/crates/cancellai-cli/tests/golden/{top_level,status,inspect,plan,clean,configure,
  version}_help.txt` are committed, reviewed, byte-exact captures of the real built binary's
  help output.
- `top_level_help_matches_the_committed_golden_snapshot` and
  `every_subcommand_help_matches_its_committed_golden_snapshot` now assert full-string
  equality (`assert_eq!(stdout(&output), golden(...))`) against these files, replacing the
  substring checks. `top_level_version_flag_prints_the_crate_version_and_exits_zero` now
  asserts the version line's exact shape (`cancellai-cli {version}\n`), not merely that the
  version substring appears somewhere in the output.

## Verification

`cargo test -p cancellai-cli --test cli_behavior`: 32/32 pass, including the four new/changed
tests above and the four pre-existing SI-007 tests unchanged. `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `cargo fmt --check`, and `cargo deny check` all
pass with no change to the dependency graph.

## Residual risk

The golden snapshot files must be regenerated deliberately (not silently) if help text ever
changes - that is the point of an exact snapshot, not a gap. `docs/CLI_RUST.md` documents the
`--help`/`-h`/`--version` precedence exception so a future contributor does not mistake it for
an unnoticed regression.
