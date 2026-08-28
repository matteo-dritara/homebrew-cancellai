//! OS capability interfaces and their per-platform implementations
//! (`docs/architecture/PLATFORM_MODEL.md`): path/identity, link/reparse, process, and
//! atomicity semantics for macOS, Linux, Windows, and WSL.
//!
//! `unsafe_code` is `forbid`-by-default at the workspace level (ADR-0015). If a platform
//! implementation genuinely needs an OS binding this crate cannot express safely otherwise,
//! that need is isolated in a small, separately-justified submodule/crate here - it is not
//! grounds for relaxing the default silently.
//!
//! Skeleton crate (E02-S01) - no types defined yet.

use cancellai_model as _;
