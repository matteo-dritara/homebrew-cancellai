//! A deterministic clock seam (E02-S04).
//!
//! Production code takes `&dyn Clock` and calls [`SystemClock`]; tests take the same trait
//! and call [`FrozenClock`] to freeze time without touching `std::time` directly, which
//! would make the same test produce a different result depending on when it runs.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch. A plain, serializable integer rather than `SystemTime`
/// itself, so a frozen reading round-trips through `serde` byte-for-byte (AC of E02-S04).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub const EPOCH: Timestamp = Timestamp(0);
}

/// A source of "now". Every production call site takes `&dyn Clock`, never `SystemTime::now()`
/// directly - see [`SystemClock`] for the one place that boundary is crossed.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// The real, OS-backed clock. Production paths use this (AC2 of E02-S04) - it is not
/// replaced or abstracted away, only made injectable alongside a deterministic alternative.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Timestamp(secs)
    }
}

/// A clock frozen at a fixed reading. Tests use this to make time-dependent output
/// reproducible (AC1 of E02-S04).
#[derive(Debug, Clone, Copy)]
pub struct FrozenClock(pub Timestamp);

impl FrozenClock {
    pub fn at(seconds_since_epoch: u64) -> Self {
        Self(Timestamp(seconds_since_epoch))
    }
}

impl Clock for FrozenClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_clock_always_returns_the_same_reading() {
        let clock = FrozenClock::at(1_000);
        assert_eq!(clock.now(), clock.now());
        assert_eq!(clock.now(), Timestamp(1_000));
    }

    #[test]
    fn system_clock_reads_a_plausible_recent_timestamp() {
        // A loose sanity bound, not a determinism claim - SystemClock is production's
        // OS-backed implementation and is expected to vary run to run (AC2).
        let now = SystemClock.now();
        assert!(
            now.0 > 1_700_000_000,
            "expected a plausible post-2023 unix timestamp, got {now:?}"
        );
    }
}
