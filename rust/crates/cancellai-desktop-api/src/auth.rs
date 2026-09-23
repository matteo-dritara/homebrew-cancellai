//! The per-session token that authenticates a local client.
//!
//! A fresh 256-bit token is drawn from the operating system for every server process. It never
//! touches disk: the server prints it once, inside its [`crate::Descriptor`], to the standard
//! output its parent reads. Another local process can reach the loopback port but cannot learn
//! the token, and a browser page that rebinds a hostname to 127.0.0.1 cannot either.

use std::fmt;

/// Token length in bytes before hex encoding.
pub const TOKEN_BYTES: usize = 32;

/// A session token. `Debug` never prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    /// Draws a new token from the operating system's cryptographic random source.
    pub fn generate() -> Result<Self, getrandom::Error> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes)?;
        Ok(Self(bytes.iter().map(|b| format!("{b:02x}")).collect()))
    }

    /// Wraps a token a client received in a descriptor.
    pub fn from_descriptor(token: String) -> Self {
        Self(token)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compares without returning early on the first differing byte, so response timing does
    /// not reveal how much of a guess was right.
    pub fn matches(&self, candidate: &str) -> bool {
        let expected = self.0.as_bytes();
        let given = candidate.as_bytes();
        if expected.len() != given.len() {
            return false;
        }
        expected
            .iter()
            .zip(given)
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(<redacted>)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_long_hex_and_distinct() {
        let a = SessionToken::generate().expect("entropy");
        let b = SessionToken::generate().expect("entropy");
        assert_eq!(a.as_str().len(), TOKEN_BYTES * 2);
        assert!(a.as_str().bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn matching_requires_the_exact_token() {
        let token = SessionToken::from_descriptor("00ff".to_string());
        assert!(token.matches("00ff"));
        for wrong in ["", "00f", "00fe", "00ff0", "00FF", "10ff"] {
            assert!(!token.matches(wrong), "{wrong}");
        }
    }

    #[test]
    fn debug_output_never_contains_the_token() {
        let token = SessionToken::generate().expect("entropy");
        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
