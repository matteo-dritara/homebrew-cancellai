//! Codex CLI provider adapter: implements `cancellai_provider_api`'s capability contract
//! for Codex's on-disk layout, including the subagent/rollout graph
//! (`docs/architecture/AS_IS.md` - Codex subagent graph). Provider-specific knowledge lives
//! here, not in `cancellai-model`/`cancellai-safety` (`docs/architecture/TARGET.md`).
//!
//! Skeleton crate (E02-S01) - no types defined yet.

use cancellai_inventory as _;
use cancellai_model as _;
use cancellai_provider_api as _;
