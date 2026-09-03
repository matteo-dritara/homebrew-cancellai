//! Command-line surface, parsed by `clap` (ADR-0019, E22-S03) instead of the hand-rolled
//! loop `main.rs` used before this story. Kept in its own module so `main.rs`'s
//! dispatch/execution logic - already heavily tested by `tests/cli_behavior.rs` - stays
//! untouched; this module is responsible only for turning `env::args()` into a typed
//! [`Invocation`], and for the SI-007-relevant decision of *which* subcommand a given
//! invocation resolves to before any real parsing happens.
//!
//! `clap` is an outer-ring dependency under ADR-0019: this crate is not part of the kernel
//! ring (`cancellai-model`/`cancellai-safety`/`cancellai-platform`/`cancellai-sealedfs`), and
//! a CLI parser decides what the user asked for, never what is permitted - SI-007 stays a
//! property of [`normalize_args`] and of the command dispatch `main.rs` owns, not of `clap`
//! itself. `ToolArg` exists so that boundary is structural: this is the only place
//! `clap::ValueEnum` is derived, and it converts into `cancellai_policy::ToolScope` rather
//! than that (policy-owned) type ever depending on `clap`.

use clap::{Parser, Subcommand, ValueEnum};

use cancellai_policy::ToolScope;

/// Every subcommand name this build recognizes as an explicit first token
/// ([`normalize_args`]'s own contract - this is not the general clap "possible values" list).
const COMMANDS: &[&str] = &["status", "inspect", "plan", "clean", "configure", "version"];
/// Tokens that must reach the *top-level* parser unmodified so `cancellai-cli --help`/`-h`/
/// `--version` show the overall command overview - matching the reference CLI's own
/// `cancellai --help`/`cancellai --version` - rather than being folded into the injected
/// `status` default and showing only that subcommand's help.
const TOP_LEVEL_TOKENS: &[&str] = &["--help", "-h", "--version"];

#[derive(Parser, Debug)]
#[command(
    name = "cancellai-cli",
    version = env!("CARGO_PKG_VERSION"),
    about = "Safely reclaim disk space from old Codex and Claude Code sessions (target-engine beta)."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show disk usage and cleanup candidates (default command)
    Status(ReadOnlyArgs),
    /// Always print the full inventory document (no Python CLI equivalent)
    Inspect(ReadOnlyArgs),
    /// Preview the actions a real `clean` would take - never mutates
    Plan(ReadOnlyArgs),
    /// Clean old session data - the only mutating command
    Clean(CleanArgs),
    /// Configure Claude Code's built-in retention
    Configure(ConfigureArgs),
    /// Print the engine name and version
    Version,
}

/// Flags shared by `status`/`inspect`/`plan` - every read-only command. A flag another
/// command needs (`--dry-run`, `--yes` on `clean`; `--claude-retention` on `configure`) is
/// deliberately absent here rather than accepted-and-ignored: E22-S03's own acceptance
/// criterion is that a flag irrelevant to a command is refused, not silently accepted the way
/// the pre-`clap` parser accepted `--dry-run status`.
#[derive(clap::Args, Debug, Clone)]
pub struct ReadOnlyArgs {
    /// Retention cutoff in days
    #[arg(long, default_value_t = 7)]
    pub days: u32,
    /// Always protect the N most-recently-modified sessions per tool, independent of age
    #[arg(long = "keep-latest", default_value_t = 2)]
    pub keep_latest: u32,
    /// Restrict to one provider
    #[arg(long, value_enum, default_value_t = ToolArg::All)]
    pub tool: ToolArg,
    /// Machine-readable output
    #[arg(long)]
    pub json: bool,
    /// Proceed even though a Codex/Claude process appears to be running, or liveness could
    /// not be determined
    #[arg(long = "allow-running")]
    pub allow_running: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct CleanArgs {
    #[command(flatten)]
    pub common: ReadOnlyArgs,
    /// Preview only; never mutates
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    /// Skip interactive confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ConfigureArgs {
    /// Claude Code's cleanupPeriodDays value to set - must be at least 1
    #[arg(long = "claude-retention", value_name = "DAYS", value_parser = clap::value_parser!(u32).range(1..))]
    pub claude_retention: u32,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolArg {
    All,
    Codex,
    Claude,
}

impl From<ToolArg> for ToolScope {
    fn from(value: ToolArg) -> Self {
        match value {
            ToolArg::All => ToolScope::All,
            ToolArg::Codex => ToolScope::Codex,
            ToolArg::Claude => ToolScope::Claude,
        }
    }
}

/// What a validated invocation resolved to - `main.rs`'s dispatch matches on this rather than
/// touching `clap` types directly, so the rest of the binary stays agnostic to which parsing
/// library produced it.
pub enum Invocation {
    Status(ReadOnlyArgs),
    Inspect(ReadOnlyArgs),
    Plan(ReadOnlyArgs),
    Clean(CleanArgs),
    Configure(ConfigureArgs),
    Version,
}

/// No subcommand, or a leading flag with no subcommand, always means `status` - the read-only
/// default (SI-007: ambiguity never escalates toward mutation). The *only* token that can
/// select `clean` is the literal string `"clean"` appearing as the first argument; nothing
/// this function does can inject or infer it. An unrecognized non-flag first token is passed
/// through unmodified so `clap` itself refuses it as an unknown subcommand (exit 2), matching
/// this crate's `INVALID_INPUT` taxonomy without a second, parallel error path to keep in
/// sync.
fn normalize_args(args: &[String]) -> Vec<String> {
    match args.first() {
        None => vec!["status".to_string()],
        Some(first) if TOP_LEVEL_TOKENS.contains(&first.as_str()) => args.to_vec(),
        Some(first) if COMMANDS.contains(&first.as_str()) => args.to_vec(),
        Some(first) if first.starts_with('-') => {
            let mut normalized = vec!["status".to_string()];
            normalized.extend_from_slice(args);
            normalized
        }
        Some(_unrecognized) => args.to_vec(),
    }
}

/// Parse `argv[1..]` into an [`Invocation`]. On `--help`/`-h`/`--version`, an unrecognized
/// flag, an unrecognized subcommand, or any other usage error, this follows `clap`'s own
/// `Parser::parse_from` contract: it prints to stdout/stderr and calls `std::process::exit`
/// itself (0 for help/version, 2 for a usage error) and therefore never returns in those
/// cases - matching this crate's pre-existing `INVALID_INPUT` exit code exactly, since 2 is
/// `clap`'s own default usage-error exit code.
pub fn parse(args: &[String]) -> Invocation {
    let mut full = vec!["cancellai-cli".to_string()];
    full.extend(normalize_args(args));
    match Cli::parse_from(full).command {
        Commands::Status(a) => Invocation::Status(a),
        Commands::Inspect(a) => Invocation::Inspect(a),
        Commands::Plan(a) => Invocation::Plan(a),
        Commands::Clean(a) => Invocation::Clean(a),
        Commands::Configure(a) => Invocation::Configure(a),
        Commands::Version => Invocation::Version,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_arguments_normalizes_to_status() {
        assert_eq!(normalize_args(&[]), vec!["status".to_string()]);
    }

    #[test]
    fn a_bare_flag_normalizes_to_status_with_the_flag_preserved() {
        let args = vec!["--json".to_string()];
        assert_eq!(
            normalize_args(&args),
            vec!["status".to_string(), "--json".to_string()]
        );
    }

    #[test]
    fn a_known_subcommand_passes_through_unmodified() {
        let args = vec!["clean".to_string(), "--yes".to_string()];
        assert_eq!(normalize_args(&args), args);
    }

    #[test]
    fn top_level_help_and_version_tokens_bypass_the_status_default() {
        for token in TOP_LEVEL_TOKENS {
            let args = vec![token.to_string()];
            assert_eq!(normalize_args(&args), args, "token: {token}");
        }
    }

    #[test]
    fn an_unrecognized_word_passes_through_for_clap_to_refuse() {
        let args = vec!["frobnicate".to_string()];
        assert_eq!(normalize_args(&args), args);
    }

    #[test]
    fn clap_command_graph_is_well_formed() {
        // `clap`'s own recommended self-check (catches conflicting arg names, invalid
        // defaults, etc. that would otherwise only surface the first time a user hits them).
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
