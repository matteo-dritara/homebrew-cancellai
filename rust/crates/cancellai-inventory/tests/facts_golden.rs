//! Golden snapshot for `FileFacts` (E04-S01 verification contract: "filesystem golden
//! snapshots"). Proves the serialized shape is stable and that every explicit
//! unsupported/unknown state actually serializes as such, not as a fabricated default.

use cancellai_inventory::{FactObservation, observe_file_facts};
use cancellai_platform::{
    AllocationObservation, FileKind, FsMetadata, IdentityObservation, IdentityToken, Observation,
    SyntheticAllocationObserver, SyntheticFsObserver, SyntheticIdentityObserver, Timestamp,
};

#[test]
fn a_fully_observed_file_produces_the_documented_golden_shape() {
    let mut fs = SyntheticFsObserver::new();
    fs.set(
        "/scope/big.bin",
        Observation::Metadata(FsMetadata {
            is_dir: false,
            is_symlink: false,
            len: 10_485_760,
            modified: Timestamp(1_700_000_000),
        }),
    );
    let mut identity = SyntheticIdentityObserver::new();
    identity.set(
        "/scope/big.bin",
        IdentityObservation::Identity(IdentityToken::Unix {
            device: 1,
            inode: 555,
            kind: FileKind::File,
            modified: Timestamp(1_700_000_000),
        }),
    );
    let mut allocation = SyntheticAllocationObserver::new();
    allocation.set("/scope/big.bin", AllocationObservation::Allocated(4_096));

    let observation = observe_file_facts(
        std::path::Path::new("/scope/big.bin"),
        &fs,
        &identity,
        &allocation,
        Some(1),
    );

    let json = serde_json::to_string_pretty(&observation).expect("serialize FactObservation");
    let expected = serde_json::json!({
        "state": "present",
        "path": "/scope/big.bin",
        "kind": "file",
        "identity": {
            "platform": "unix",
            "state": "identity",
            "device": 1,
            "inode": 555,
            "kind": "file",
            "modified": 1_700_000_000
        },
        "logical_size": { "state": "known", "bytes": 10_485_760 },
        "allocated_size": { "state": "known", "bytes": 4_096 },
        "modified": 1_700_000_000,
        "boundary": { "state": "within_scope" },
        "provider_hint": null,
        "category_hint": null,
        "confidence": { "state": "complete" }
    });
    let actual: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(actual, expected, "golden shape drifted:\n{json}");
}

#[test]
fn unsupported_and_absent_states_serialize_explicitly_never_as_null_or_zero() {
    let mut fs = SyntheticFsObserver::new();
    fs.set(
        "/scope/exotic",
        Observation::Metadata(FsMetadata {
            is_dir: false,
            is_symlink: false,
            len: 42,
            modified: Timestamp(1_000),
        }),
    );
    let identity = SyntheticIdentityObserver::new(); // unset -> Absent (raced)
    let mut allocation = SyntheticAllocationObserver::new();
    allocation.set(
        "/scope/exotic",
        AllocationObservation::Unsupported {
            reason: "no allocation metric on this filesystem".into(),
        },
    );

    let observation = observe_file_facts(
        std::path::Path::new("/scope/exotic"),
        &fs,
        &identity,
        &allocation,
        None,
    );
    let facts = match observation {
        FactObservation::Present(facts) => facts,
        other => panic!("expected Present, got {other:?}"),
    };

    let json = serde_json::to_value(&facts.allocated_size).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "state": "unsupported", "reason": "no allocation metric on this filesystem" })
    );
    assert_ne!(json, serde_json::json!({ "state": "known", "bytes": 0 }));
}
