//! Headless/scriptable CLI client (`docs/architecture/TARGET.md` - experience plane).
//!
//! Forbidden dependency direction: this crate may not access raw provider roots for
//! mutation directly - all mutation is routed through `cancellai-safety`.
//!
//! Skeleton crate (E02-S01) - no CLI surface defined yet; real command handling starts
//! once `cancellai-model`/`cancellai-safety` exist to route through (E02-S03 onward).

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
    println!("cancellai-cli: workspace skeleton (E02-S01), no command surface yet");
}
