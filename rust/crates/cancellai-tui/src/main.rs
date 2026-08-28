//! Terminal experience client (E09 Atlas TUI; `docs/architecture/TARGET.md` - experience
//! plane). Shares the engine/query API with the CLI rather than duplicating engine logic.
//!
//! Skeleton crate (E02-S01) - no TUI surface defined yet.

use cancellai_inventory as _;
use cancellai_model as _;
use cancellai_platform as _;
use cancellai_policy as _;
use cancellai_provider_api as _;
use cancellai_provider_claude as _;
use cancellai_provider_codex as _;
use cancellai_safety as _;
use cancellai_store as _;

fn main() {
    println!("cancellai-tui: workspace skeleton (E02-S01), no interface yet");
}
