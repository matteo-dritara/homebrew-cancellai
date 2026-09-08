//! Release-channel authority binding (E17-S05, SI-030, `docs/security/SUPPLY_CHAIN.md`
//! "Release channels": "Beta/nightly builds and unverified knowledge bundles do not inherit
//! stable-release destructive authority by default.").
//!
//! [`BuildChannel`] is the only type [`crate::authority::AuthorityInputs::release_channel`]
//! accepts, and [`BuildChannel::from_compiled_env`] is the only production way anywhere in
//! this workspace to obtain one - mirroring [`crate::TrustedTier`]'s split from
//! [`cancellai_model::ProviderTrust`] for the identical reason (see `trust_promotion.rs`'s
//! module doc, and E05 verifier review round 1, which is exactly the class of defect this
//! guards against here too): a bare [`ReleaseChannel`] is freely constructible pure
//! vocabulary, so if `AuthorityInputs::release_channel` accepted that type directly, any
//! caller could write `AuthorityInputs { release_channel: ReleaseChannel::Stable, .. }` and
//! reach stable-level authority from a nightly binary with no relationship to what that binary
//! was actually built as.
//!
//! **Why a compile-time env var, not a runtime one:** SI-030 exists specifically so a user
//! cannot unlock stable-level default authority "merely because user configuration exists" -
//! a *runtime* environment variable is exactly that: something the user running the binary
//! controls. [`option_env!`] reads `CANCELLAI_CHANNEL` at the moment `cancellai-safety` itself
//! is compiled (baked into the resulting binary, immutable afterward), not at the moment the
//! binary runs. `.github/workflows/release.yml`'s `build-artifacts` job sets it to `stable`
//! for every canonical release build; a local `cargo build`/`cargo test`/CI check build (no
//! variable set) gets [`ReleaseChannel::Nightly`] - the most restrictive default, per SI-030's
//! own fail-closed requirement, not an unlabeled/unknown state treated as more permissive than
//! naming a channel explicitly would be.
//!
//! **Known limitation, not a security gap in the shipped binary:** `option_env!` does not
//! register a `cargo:rerun-if-env-changed` dependency the way a build script would, so a
//! developer who changes `CANCELLAI_CHANNEL` between two `cargo build` invocations sharing the
//! same `target/` directory without touching this file may see a stale cached value. CI's
//! `build-artifacts` job always starts from a fresh checkout and a fresh `target/` per matrix
//! leg, so this does not affect what any actual canonical release binary bakes in - only local
//! iteration on this file's own env-var handling.

use cancellai_model::ReleaseChannel;

/// The only [`ReleaseChannel`] value [`crate::authority::effective_authority`] accepts as a
/// build's release channel. See the module doc for why this exists and what it closes.
///
/// This doctest is the regression proving an external caller cannot construct a `BuildChannel`
/// at an arbitrary level, the same way `TrustedTier`'s own doctest proves it for provider trust:
///
/// ```compile_fail
/// # use cancellai_model::ReleaseChannel;
/// # use cancellai_safety::BuildChannel;
/// // BuildChannel's field is private: no tuple-struct construction from outside this crate.
/// let forged = BuildChannel(ReleaseChannel::Stable);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildChannel(ReleaseChannel);

impl BuildChannel {
    /// Reads the channel this specific compiled binary was built as, from the `CANCELLAI_CHANNEL`
    /// value present when `cancellai-safety` itself was compiled. Recognizes `"stable"` and
    /// `"beta"` case-insensitively; anything else (including unset, and including a typo'd or
    /// unrecognized value - deliberately, so a malformed override cannot silently claim more
    /// authority than an absent one) resolves to [`ReleaseChannel::Nightly`], the fail-closed
    /// floor.
    pub fn from_compiled_env() -> Self {
        BuildChannel(match option_env!("CANCELLAI_CHANNEL") {
            Some(value) if value.eq_ignore_ascii_case("stable") => ReleaseChannel::Stable,
            Some(value) if value.eq_ignore_ascii_case("beta") => ReleaseChannel::Beta,
            _ => ReleaseChannel::Nightly,
        })
    }

    /// The channel this value carries.
    pub fn level(self) -> ReleaseChannel {
        self.0
    }

    /// Constructs a `BuildChannel` at an arbitrary level with no compiled-env check at all -
    /// visible only inside this crate (`pub(crate)`) and only compiled for tests, mirroring
    /// `TrustedTier::for_tests` for the identical reason: this crate's own tests need to set up
    /// fixture state (e.g. "assume a stable build, then assert some *other* input still
    /// collapses authority") without depending on the process environment at compile time.
    #[cfg(test)]
    pub(crate) fn for_tests(level: ReleaseChannel) -> Self {
        BuildChannel(level)
    }
}

impl Default for BuildChannel {
    /// [`ReleaseChannel::Nightly`] - the same fail-closed floor an absent/unrecognized
    /// `CANCELLAI_CHANNEL` resolves to, so a `BuildChannel` obtained via `Default::default()`
    /// (e.g. `..Default::default()` in a struct-update fixture) can never accidentally start
    /// out more privileged than SI-030 allows.
    fn default() -> Self {
        BuildChannel(ReleaseChannel::Nightly)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_build_channel_defaults_to_the_most_restrictive_level() {
        assert_eq!(BuildChannel::default().level(), ReleaseChannel::Nightly);
    }

    #[test]
    fn for_tests_round_trips_every_level() {
        for level in [
            ReleaseChannel::Nightly,
            ReleaseChannel::Beta,
            ReleaseChannel::Stable,
        ] {
            assert_eq!(BuildChannel::for_tests(level).level(), level);
        }
    }

    // `from_compiled_env`'s actual `option_env!` branch cannot be exercised by a unit test in
    // this crate's own build (the workspace's rust.yml/release.yml never set CANCELLAI_CHANNEL
    // for `cargo test`, and a test cannot retroactively change what was compiled in) - this is
    // the same class of "verified for real in CI, not locally" residual this repository already
    // discloses for platform-specific behavior (`docs/PLATFORMS.md`). What *is* testable here,
    // and matters more: an absent/unset compiled value is never treated as anything but the
    // floor, which `a_fresh_build_channel_defaults_to_the_most_restrictive_level` above proves
    // for the `Default` path `from_compiled_env`'s fallback arm shares the same fail-closed
    // outcome with.
    #[test]
    fn from_compiled_env_in_this_test_build_is_the_fail_closed_floor() {
        // This workspace's own test/check builds never set CANCELLAI_CHANNEL, so this is a
        // real (not synthetic) proof that an unset compiled value resolves to Nightly.
        assert_eq!(
            BuildChannel::from_compiled_env().level(),
            ReleaseChannel::Nightly
        );
    }
}
