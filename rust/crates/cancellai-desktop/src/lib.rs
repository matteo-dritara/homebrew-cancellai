//! The optional desktop experience (E19-S02, [ADR-0038](../../../../docs/adrs/0038-the-desktop-shell-is-a-loopback-dashboard-over-the-desktop-api.md)).
//!
//! A read-only dashboard served on loopback and opened in the system browser. It is a client of
//! the engine and nothing more: it starts `cancellai-cli desktop-api`, reads documents through
//! that versioned, token-authenticated channel (`cancellai-desktop-api`, E19-S01), and renders
//! them. It classifies nothing, plans nothing, and cannot clean - the engine stays
//! headless-capable and the CLI keeps working whether or not this crate is ever built.
//!
//! Everything it shows is the engine's own answer: the provider rows are the CLI `status`
//! summary the engine computed, and the plan section counts the plan document's actions the way
//! the CLI `plan` summary does ([`viewmodel`]).

pub mod render;
pub mod server;
pub mod source;
pub mod viewmodel;

pub use server::{Dashboard, Reply, ViewSource};
pub use source::ApiViewSource;
pub use viewmodel::{ActionRow, DashboardView, PlanView, ProviderRow, ViewError, build};
