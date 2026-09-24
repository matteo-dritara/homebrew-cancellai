//! Named pause points on `clean`'s mutation path, for the crash/recovery harness (E06-S09).
//!
//! The harness kills a running `clean` at each point and checks what the kill left behind. To
//! kill at a point it must first know the process reached it, so a point that is armed writes a
//! marker file and then waits to be killed; the harness never guesses with a timer.
//!
//! This exists only in a build with the `kill-points` feature. Without it, [`reached`] is an
//! empty inline function and the environment variables below are never read, so the released
//! binary carries neither the pause nor a way to request one. `tests/kill_harness.rs` is the
//! only intended consumer.
//!
//! The points, in the order `clean` reaches them: `roots-established`, `before-delete#n`,
//! `after-delete#n`, `before-report`. `tests/kill_harness.rs` enumerates the same list.
//! `containment install|refresh|lift` has one more, `containment-before-commit`, between the
//! ledger row's insert and its commit (E33-S03, `tests/containment.rs`).
//!
//! - `CANCELLAI_KILL_POINT` names the point to arm, as `<point>` or `<point>#<n>` for the n-th
//!   (zero-based) visit of a point reached once per action.
//! - `CANCELLAI_KILL_MARKER` is the file written when the armed point is reached.
//! - `CANCELLAI_KILL_PLANT_PARTIAL`, when set to a path, makes the armed point first write half
//!   of a file there - a planted defect the harness must detect, never used outside its own test.

#[cfg(feature = "kill-points")]
pub fn reached(point: &str, visit: usize) {
    let Ok(armed) = std::env::var("CANCELLAI_KILL_POINT") else {
        return;
    };
    let matches = armed == point || armed == format!("{point}#{visit}");
    if !matches {
        return;
    }
    if let Ok(partial) = std::env::var("CANCELLAI_KILL_PLANT_PARTIAL") {
        std::fs::write(partial, b"{\"schema_version\":1,\"records\":[").ok();
    }
    if let Ok(marker) = std::env::var("CANCELLAI_KILL_MARKER") {
        std::fs::write(marker, point.as_bytes()).ok();
    }
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(feature = "kill-points"))]
#[inline(always)]
pub fn reached(_point: &str, _visit: usize) {}
