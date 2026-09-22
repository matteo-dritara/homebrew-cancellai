//! Later user-service runtime (`docs/architecture/GUARDIAN_MODEL.md`): predictive
//! pressure/anomaly signals and bounded, safety-floor-respecting remediation
//! (`SI-027`, `SI-028` - detection severity and Guardian never self-escalate authority).
//!
//! Split into a library and a thin binary (`main.rs`), matching `cancellai-tui`'s pattern: the
//! detection modules (`pressure`/`forecast`/`baseline`/`structural`) are pure logic with no
//! runtime/service surface, exercised directly by unit tests. `service` (E15-S01) is the first
//! module with a real runtime surface: the cross-platform user-service lifecycle
//! (`docs/architecture/GUARDIAN_MODEL.md` "Runtime") the compiled binary now exposes.
//! `notification` (E15-S02) is a second: OS-appropriate notification delivery with a terminal
//! fallback. `remediation` (E15-S03) is a third: the bounded remediation planner that turns
//! pressure plus already-classified artifacts into a plan, never granting authority beyond what
//! the shared policy engine already computed (SI-027, SI-028). None is wired to a live
//! orchestrator yet, matching this crate's own detection modules' "primitive delivered, no
//! orchestrator yet" precedent.

pub mod baseline;
pub mod forecast;
pub mod notification;
pub mod pressure;
pub mod remediation;
pub mod service;
// Each platform module is real production code only on its own `target_os`, but stays
// unit-testable (via its own `FakeCommandRunner`) on every host by also compiling under `test` -
// the same `#[cfg(any(test, target_os = "..."))]` split `cancellai_platform::wsl` already uses,
// applied to a whole module instead of one function.
#[cfg(any(test, target_os = "linux"))]
mod service_linux;
#[cfg(any(test, target_os = "macos"))]
mod service_macos;
#[cfg(any(test, target_os = "windows"))]
mod service_windows;
pub mod structural;
