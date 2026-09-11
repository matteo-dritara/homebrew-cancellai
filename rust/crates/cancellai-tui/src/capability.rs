//! Terminal capability detection (E09-S01, AC3 "graceful capability fallback").
//!
//! There is no existing "tier-1 terminal" definition in this repository -
//! `docs/architecture/PLATFORMS.md`'s "tier-1" is about OS platforms (macOS/Linux/Windows
//! native), not terminal emulators. This module defines the axis this story actually needs:
//! how much color a terminal accepts, and whether it can render Unicode box-drawing glyphs -
//! and degrades to the safest choice whenever a signal is missing or contradictory, mirroring
//! `cancellai-platform`'s own "unknown/unverified never claims a stronger capability" posture.
//!
//! Environment is read through [`EnvSource`], not `std::env::var` directly, so tests can
//! exercise every branch with a synthetic map instead of mutating real process environment
//! (the same dependency-injection shape `cancellai-platform`'s `Clock`/`FrozenClock` already
//! uses for time).

use std::collections::HashMap;
use std::env;

/// A source of environment variables. [`ProcessEnv`] reads the real process environment;
/// [`SyntheticEnv`] (test-only) reads a fixed map.
pub trait EnvSource {
    fn get(&self, key: &str) -> Option<String>;
}

/// Reads the real process environment - used by `main.rs`, never by this module's own tests.
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn get(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

/// A fixed, synthetic environment for tests - see module docs.
#[derive(Debug, Default, Clone)]
pub struct SyntheticEnv(HashMap<String, String>);

impl SyntheticEnv {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn set(mut self, key: &str, value: &str) -> Self {
        self.0.insert(key.to_string(), value.to_string());
        self
    }
}

impl EnvSource for SyntheticEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
}

/// How much color styling a terminal is safe to receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSupport {
    /// No color escape codes at all - `NO_COLOR` is set, or `TERM` is absent/`dumb`.
    None,
    /// The universally-safe 16-color ANSI palette.
    Basic,
    /// 256-color or truecolor - only claimed on an explicit, positive signal.
    Extended,
}

/// What this run should assume about the terminal it draws into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapability {
    pub color: ColorSupport,
    /// Whether Unicode box-drawing glyphs are safe to render; `false` falls back to ASCII
    /// (`+`/`-`/`|`) borders.
    pub unicode: bool,
}

impl TerminalCapability {
    /// The most conservative capability: no color, ASCII only. Used when detection itself is
    /// impossible to trust (never reached today, but keeps this type's safest state explicit
    /// and reachable rather than only implied by `detect`'s branches).
    pub const MINIMAL: TerminalCapability = TerminalCapability {
        color: ColorSupport::None,
        unicode: false,
    };
}

/// Detect terminal capability from `env` (AC3). Fail-safe by construction: every branch that
/// is not a positive, explicit signal for a *stronger* capability falls back to the weaker one,
/// so a missing or unrecognized variable degrades gracefully instead of assuming the best case.
pub fn detect(env: &impl EnvSource) -> TerminalCapability {
    TerminalCapability {
        color: detect_color(env),
        unicode: detect_unicode(env),
    }
}

fn detect_color(env: &impl EnvSource) -> ColorSupport {
    // https://no-color.org - any value, including empty, means "no color" and overrides
    // every other signal.
    if env.get("NO_COLOR").is_some() {
        return ColorSupport::None;
    }
    let term = env.get("TERM").unwrap_or_default();
    if term.is_empty() || term == "dumb" {
        return ColorSupport::None;
    }
    let colorterm = env.get("COLORTERM").unwrap_or_default().to_lowercase();
    if colorterm == "truecolor" || colorterm == "24bit" || term.contains("256color") {
        return ColorSupport::Extended;
    }
    ColorSupport::Basic
}

fn detect_unicode(env: &impl EnvSource) -> bool {
    if env.get("CANCELLAI_TUI_ASCII").is_some() {
        return false;
    }
    for key in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Some(value) = env.get(key) {
            return value.to_uppercase().contains("UTF-8") || value.to_uppercase().contains("UTF8");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_color_wins_over_an_otherwise_truecolor_terminal() {
        let env = SyntheticEnv::new()
            .set("NO_COLOR", "1")
            .set("TERM", "xterm-256color")
            .set("COLORTERM", "truecolor");
        assert_eq!(detect(&env).color, ColorSupport::None);
    }

    #[test]
    fn missing_term_falls_back_to_no_color() {
        let env = SyntheticEnv::new();
        assert_eq!(detect(&env).color, ColorSupport::None);
    }

    #[test]
    fn dumb_term_falls_back_to_no_color() {
        let env = SyntheticEnv::new().set("TERM", "dumb");
        assert_eq!(detect(&env).color, ColorSupport::None);
    }

    #[test]
    fn colorterm_truecolor_is_extended() {
        let env = SyntheticEnv::new()
            .set("TERM", "xterm")
            .set("COLORTERM", "truecolor");
        assert_eq!(detect(&env).color, ColorSupport::Extended);
    }

    #[test]
    fn term_256color_is_extended_even_without_colorterm() {
        let env = SyntheticEnv::new().set("TERM", "screen-256color");
        assert_eq!(detect(&env).color, ColorSupport::Extended);
    }

    #[test]
    fn a_plain_recognized_term_is_basic() {
        let env = SyntheticEnv::new().set("TERM", "xterm");
        assert_eq!(detect(&env).color, ColorSupport::Basic);
    }

    #[test]
    fn utf8_lang_enables_unicode() {
        let env = SyntheticEnv::new().set("LANG", "en_US.UTF-8");
        assert!(detect(&env).unicode);
    }

    #[test]
    fn lc_all_takes_precedence_over_lang() {
        let env = SyntheticEnv::new()
            .set("LC_ALL", "C")
            .set("LANG", "en_US.UTF-8");
        assert!(!detect(&env).unicode);
    }

    #[test]
    fn missing_locale_vars_fall_back_to_ascii() {
        let env = SyntheticEnv::new();
        assert!(!detect(&env).unicode);
    }

    #[test]
    fn ascii_escape_hatch_overrides_a_utf8_locale() {
        let env = SyntheticEnv::new()
            .set("LANG", "en_US.UTF-8")
            .set("CANCELLAI_TUI_ASCII", "1");
        assert!(!detect(&env).unicode);
    }

    #[test]
    fn minimal_capability_is_the_safest_state() {
        let minimal = std::hint::black_box(TerminalCapability::MINIMAL);
        assert_eq!(minimal.color, ColorSupport::None);
        assert!(!minimal.unicode);
    }
}
