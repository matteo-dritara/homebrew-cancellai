//! Terminal (Atlas) experience client (E09 "Atlas TUI", `docs/architecture/TARGET.md` -
//! experience plane, "Atlas TUI shell (E09-S01)").
//!
//! Split into a library and a thin binary (`main.rs`) - a deliberate departure from
//! `cancellai-cli`'s no-lib, spawn-the-real-binary test style: raw-mode terminal
//! initialization cannot run headlessly in CI, so the testable surface is these pure modules
//! (exercised through `ratatui::backend::TestBackend`), not the compiled binary.
//!
//! AC1 ("No direct filesystem/provider access from the TUI crate"): this crate depends on
//! neither a provider adapter nor `cancellai-inventory`/`cancellai-platform`/`cancellai-store`
//! at all (see `Cargo.toml`'s own comment) - only `cancellai-policy`'s public view-model API,
//! read through [`data::EngineData`], plus `crossterm`/`ratatui` and this crate's own state.

pub mod app;
pub mod capability;
pub mod data;
pub mod event;
pub mod format;
pub mod ui;
