//! Determinism test: repeats (illustrative) plan generation byte-for-byte (E02-S04
//! verification contract).
//!
//! `cancellai_platform::snapshot::build_snapshot` stands in for real plan generation until
//! the safety kernel (E03) and inventory engine (E04) exist - see that module's doc comment.
//! What this proves is the seam composition itself: given a frozen clock and synthetic
//! filesystem facts, two independent runs produce byte-identical serialized output, and
//! changing either the clock reading or a single fact changes that output - the comparison
//! has to be able to fail, or passing it proves nothing.

use std::path::PathBuf;

use cancellai_platform::{
    FrozenClock, FsMetadata, Observation, SyntheticFsObserver, Timestamp, build_snapshot,
};

fn example_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/synthetic/root/session-a.jsonl"),
        PathBuf::from("/synthetic/root/session-b.jsonl"),
        PathBuf::from("/synthetic/root/locked"),
        PathBuf::from("/synthetic/root/missing"),
    ]
}

fn example_observer() -> SyntheticFsObserver {
    let mut observer = SyntheticFsObserver::new();
    observer.set(
        "/synthetic/root/session-a.jsonl",
        Observation::Metadata(FsMetadata {
            is_dir: false,
            is_symlink: false,
            len: 128,
            modified: Timestamp(1_700_000_000),
        }),
    );
    observer.set(
        "/synthetic/root/session-b.jsonl",
        Observation::Metadata(FsMetadata {
            is_dir: false,
            is_symlink: false,
            len: 256,
            modified: Timestamp(1_700_000_100),
        }),
    );
    observer.set(
        "/synthetic/root/locked",
        Observation::Unreadable {
            reason: "permission denied".into(),
        },
    );
    // "/synthetic/root/missing" deliberately left unset - it must observe as Absent.
    observer
}

#[test]
fn two_independent_runs_with_the_same_frozen_inputs_are_byte_identical() {
    let clock = FrozenClock::at(1_700_000_500);
    let paths = example_paths();

    let run_a = build_snapshot(&clock, &example_observer(), &paths);
    let run_b = build_snapshot(&clock, &example_observer(), &paths);

    let json_a = serde_json::to_string_pretty(&run_a).expect("serializable");
    let json_b = serde_json::to_string_pretty(&run_b).expect("serializable");
    assert_eq!(
        json_a, json_b,
        "identical frozen clock + synthetic facts must produce byte-identical output"
    );
}

#[test]
fn a_different_frozen_reading_changes_the_output() {
    let paths = example_paths();
    let json_at_500 = serde_json::to_string_pretty(&build_snapshot(
        &FrozenClock::at(1_700_000_500),
        &example_observer(),
        &paths,
    ))
    .unwrap();
    let json_at_600 = serde_json::to_string_pretty(&build_snapshot(
        &FrozenClock::at(1_700_000_600),
        &example_observer(),
        &paths,
    ))
    .unwrap();
    assert_ne!(
        json_at_500, json_at_600,
        "the comparison above must be able to fail, or it proves nothing"
    );
}

#[test]
fn a_single_changed_fact_changes_the_output() {
    let clock = FrozenClock::at(1_700_000_500);
    let paths = example_paths();

    let mut altered = example_observer();
    altered.set(
        "/synthetic/root/session-a.jsonl",
        Observation::Metadata(FsMetadata {
            is_dir: false,
            is_symlink: false,
            len: 129,
            modified: Timestamp(1_700_000_000),
        }),
    );

    let baseline =
        serde_json::to_string_pretty(&build_snapshot(&clock, &example_observer(), &paths)).unwrap();
    let changed = serde_json::to_string_pretty(&build_snapshot(&clock, &altered, &paths)).unwrap();
    assert_ne!(
        baseline, changed,
        "changing one observed fact must change the snapshot"
    );
}

#[test]
fn absent_and_unreadable_are_never_conflated_in_the_snapshot() {
    let clock = FrozenClock::at(1_700_000_500);
    let paths = example_paths();
    let snapshot = build_snapshot(&clock, &example_observer(), &paths);

    let locked = &snapshot.observations[&PathBuf::from("/synthetic/root/locked")];
    let missing = &snapshot.observations[&PathBuf::from("/synthetic/root/missing")];
    assert!(matches!(locked, Observation::Unreadable { .. }));
    assert_eq!(*missing, Observation::Absent);
    assert_ne!(
        locked, missing,
        "an unreadable path and a genuinely absent one must never serialize the same way"
    );
}
