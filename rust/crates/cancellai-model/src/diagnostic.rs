//! Typed error categories and the stable machine-facing exit/code mapping (E02-S03).
//!
//! Generalizes the exit taxonomy `docs/architecture/AS_IS.md` documents for the Python
//! reference (0 success / 1 declined / 2 invalid usage / 3 mutation failure / 4 safety
//! block-or-defer) into a finer-grained taxonomy for the Rust target: the Python reference
//! collapsed several distinct failure modes into the same exit code because it had no typed
//! error model to keep them apart. This crate is not required to reuse Python's numeric
//! codes 1:1 - see `docs/architecture/DOMAIN_MODEL.md`'s Diagnostics section for the mapping
//! rationale.
//!
//! [`Diagnostic`] is the single source of truth for both the human-readable ([`Display`])
//! and machine-facing ([`serde::Serialize`]) renderings of an error: both are derived from
//! the same [`ErrorCategory::code`], so they cannot drift apart (AC2 of E02-S03).

use std::fmt;

/// The six error categories every diagnostic in cancellAI belongs to.
///
/// This enum is exhaustively matched by [`ErrorCategory::exit_code`] and
/// [`ErrorCategory::code`]; adding a category is a compile error everywhere those matches
/// are not also updated, which is deliberate - a category with no defined exit code or
/// string code is not a category this crate can emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Usage or configuration is invalid or ambiguous. Ambiguity never escalates privilege
    /// (C-03): this refuses rather than guesses at destructive intent.
    InvalidInput,
    /// A constitutional Safety Invariant refused the requested action.
    SafetyBlock,
    /// A scan was `PARTIAL`/`UNKNOWN` and destructive authority was withheld as a result
    /// (SI-008, SI-009): absence of evidence is never read as absence of active/protected
    /// data.
    IncompleteInventory,
    /// Provider/version/layout is unrecognized or has drifted; a capability could not be
    /// established (SI-004).
    CompatibilityFailure,
    /// A sealed, revalidated action was attempted and failed - or was stale
    /// (`STALE_PLAN`, SI-013) and safely skipped.
    MutationFailure,
    /// An unexpected internal fault: a bug, not a modeled outcome.
    InternalFault,
}

impl ErrorCategory {
    /// All categories, in the stable order used by golden tests and diagnostics listings.
    pub const ALL: [ErrorCategory; 6] = [
        ErrorCategory::InvalidInput,
        ErrorCategory::SafetyBlock,
        ErrorCategory::IncompleteInventory,
        ErrorCategory::CompatibilityFailure,
        ErrorCategory::MutationFailure,
        ErrorCategory::InternalFault,
    ];

    /// Stable, machine-facing exit code. Never renumbered once released: a new category
    /// gets a new code, an existing one is never repurposed for a different category.
    pub const fn exit_code(self) -> i32 {
        match self {
            ErrorCategory::InvalidInput => 2,
            ErrorCategory::SafetyBlock => 4,
            ErrorCategory::IncompleteInventory => 4,
            ErrorCategory::CompatibilityFailure => 4,
            ErrorCategory::MutationFailure => 3,
            ErrorCategory::InternalFault => 3,
        }
    }

    /// Stable, machine-facing string code. The single source of truth both the human
    /// ([`Display`]) and JSON ([`serde::Serialize`]) renderings of a [`Diagnostic`] read
    /// from - neither representation may compute this independently.
    pub const fn code(self) -> &'static str {
        match self {
            ErrorCategory::InvalidInput => "INVALID_INPUT",
            ErrorCategory::SafetyBlock => "SAFETY_BLOCK",
            ErrorCategory::IncompleteInventory => "INCOMPLETE_INVENTORY",
            ErrorCategory::CompatibilityFailure => "COMPATIBILITY_FAILURE",
            ErrorCategory::MutationFailure => "MUTATION_FAILURE",
            ErrorCategory::InternalFault => "INTERNAL_FAULT",
        }
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl serde::Serialize for ErrorCategory {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

/// One diagnostic: a category and a human-readable message.
///
/// `Diagnostic` never stores its own copy of the stable code - [`Diagnostic::code`] and
/// [`Diagnostic::exit_code`] always delegate to `self.category`, and the `Serialize` impl
/// serializes `category` directly, so the JSON and human renderings cannot diverge from each
/// other or from a hand-maintained duplicate field.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    pub category: ErrorCategory,
    pub message: String,
}

impl Diagnostic {
    pub fn new(category: ErrorCategory, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }

    /// Stable, machine-facing string code - see [`ErrorCategory::code`].
    pub fn code(&self) -> &'static str {
        self.category.code()
    }

    /// Stable, machine-facing exit code - see [`ErrorCategory::exit_code`].
    pub fn exit_code(&self) -> i32 {
        self.category.exit_code()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.category.code(), self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_has_a_distinct_code_and_a_defined_exit_code() {
        let mut seen_codes = std::collections::HashSet::new();
        for category in ErrorCategory::ALL {
            assert!(
                seen_codes.insert(category.code()),
                "duplicate code for {category:?}"
            );
            assert!(
                category.exit_code() != 0,
                "{category:?} must not silently mean success"
            );
        }
    }

    #[test]
    fn display_and_serialize_never_diverge() {
        for category in ErrorCategory::ALL {
            let diagnostic = Diagnostic::new(category, "example");
            let displayed = diagnostic.to_string();
            let json = serde_json::to_string(&diagnostic).expect("serializable");
            assert!(displayed.contains(category.code()));
            assert!(json.contains(category.code()));
        }
    }
}
