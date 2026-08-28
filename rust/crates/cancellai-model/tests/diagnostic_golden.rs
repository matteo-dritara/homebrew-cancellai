//! Golden diagnostic tests across all six error categories (E02-S03 verification contract).
//!
//! Each category gets a fixed, deterministic `Diagnostic` and a committed golden JSON file
//! under `tests/golden/`. The test fails if the pretty-printed JSON drifts from the golden
//! file (catching an accidental field rename/reorder) and separately proves the `Display`
//! rendering embeds the same stable code the JSON does - the two representations sharing one
//! source of truth is AC2, not merely both happening to look right today.

use cancellai_model::{Diagnostic, ErrorCategory};

struct Case {
    category: ErrorCategory,
    message: &'static str,
    golden_file: &'static str,
}

const CASES: [Case; 6] = [
    Case {
        category: ErrorCategory::InvalidInput,
        message: "requested provider root escapes the operator's home directory; refusing",
        golden_file: "invalid_input.json",
    },
    Case {
        category: ErrorCategory::SafetyBlock,
        message: "candidate is covered by a protected name; the barrier refused it",
        golden_file: "safety_block.json",
    },
    Case {
        category: ErrorCategory::IncompleteInventory,
        message: "scan was partial; destructive authority is withheld for this scope",
        golden_file: "incomplete_inventory.json",
    },
    Case {
        category: ErrorCategory::CompatibilityFailure,
        message: "provider layout is unrecognized by this build",
        golden_file: "compatibility_failure.json",
    },
    Case {
        category: ErrorCategory::MutationFailure,
        message: "revalidation found the plan stale; the action was safely skipped",
        golden_file: "mutation_failure.json",
    },
    Case {
        category: ErrorCategory::InternalFault,
        message: "unexpected internal error",
        golden_file: "internal_fault.json",
    },
];

fn golden_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

#[test]
fn every_category_has_a_committed_golden_document() {
    assert_eq!(
        CASES.len(),
        ErrorCategory::ALL.len(),
        "a category is missing a golden case"
    );
    for category in ErrorCategory::ALL {
        assert!(
            CASES.iter().any(|case| case.category == category),
            "{category:?} has no golden case in this test file"
        );
    }
}

#[test]
fn json_diagnostics_match_their_golden_document() {
    for case in &CASES {
        let diagnostic = Diagnostic::new(case.category, case.message);
        let actual = serde_json::to_string_pretty(&diagnostic).expect("serializable") + "\n";
        let path = golden_path(case.golden_file);
        let expected =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path:?}: {e}"));
        assert_eq!(
            actual, expected,
            "regenerate {path:?} if this change is intentional and reviewed"
        );
    }
}

#[test]
fn human_and_json_diagnostics_share_the_same_stable_code() {
    for case in &CASES {
        let diagnostic = Diagnostic::new(case.category, case.message);
        let human = diagnostic.to_string();
        let json = serde_json::to_string(&diagnostic).expect("serializable");
        assert!(human.starts_with(&format!("[{}]", diagnostic.code())));
        assert!(json.contains(&format!("\"category\":\"{}\"", diagnostic.code())));
        assert_eq!(diagnostic.code(), case.category.code());
    }
}

#[test]
fn exit_codes_are_stable_and_distinguish_the_documented_severity_bands() {
    // AC1 groups these into three exit bands (usage / safety-or-incomplete-or-compat /
    // mutation-or-internal), mirroring the Python reference's coarser taxonomy
    // (docs/architecture/AS_IS.md) while keeping the six categories distinct string codes.
    assert_eq!(ErrorCategory::InvalidInput.exit_code(), 2);
    for category in [
        ErrorCategory::SafetyBlock,
        ErrorCategory::IncompleteInventory,
        ErrorCategory::CompatibilityFailure,
    ] {
        assert_eq!(
            category.exit_code(),
            4,
            "{category:?} should share the safety-withheld exit band"
        );
    }
    for category in [ErrorCategory::MutationFailure, ErrorCategory::InternalFault] {
        assert_eq!(
            category.exit_code(),
            3,
            "{category:?} should share the failure exit band"
        );
    }
}
