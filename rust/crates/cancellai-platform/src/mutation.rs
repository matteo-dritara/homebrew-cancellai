//! Filesystem mutation as its own OS capability seam (E03-S05,
//! `docs/architecture/PLATFORM_MODEL.md`'s "atomic rename/move capability", SI-019).
//!
//! This is the one seam in this crate whose real implementation ([`SystemMutationExecutor`])
//! actually changes the filesystem - every other capability here (`Clock`, `FsObserver`,
//! `IdentityObserver`, `PathResolver`) only observes. `scripts/check_mutation_boundary.py`
//! (E03-S05) statically enforces that no other production source file in this workspace
//! calls `std::fs::remove_file`/`remove_dir`/`remove_dir_all` directly - this file is the
//! one place that call is allowed to exist (SI-019: "all filesystem/vendor mutations route
//! through the safety executor"). `cancellai-safety`'s orchestration (`mutation_executor.rs`)
//! is the *only* production caller of this trait, and only after SI-002/SI-003/SI-013 have
//! already been checked - this seam itself performs no safety-policy check of its own
//! (authority/reversibility gating is `cancellai-safety`'s job), but it does prove, as far as
//! a safe-Rust, dependency-free implementation can, that what it deletes is what the caller
//! told it to expect.
//!
//! E03 verifier review round 1 found the original `mutate(target, operation)` signature
//! (no expected identity) let a path-based revalidate-then-delete race succeed silently: an
//! `IdentityObserver` that reports a matching identity and then, as a side effect, swaps the
//! object before the actual `remove_file` call, caused the *replacement* to be deleted while
//! `execute` reported `Succeeded`. `mutate` now takes `expected: &IdentityToken`, and
//! [`SystemMutationExecutor`]'s file-deletion path performs three checks around one held
//! file descriptor: (1) open the target once and confirm the descriptor's own device/inode
//! match `expected`; (2) immediately before the actual unlink, a second, independent, fresh
//! path lookup (not through the descriptor) re-confirms the path still resolves to that same
//! identity - this is the check that actually stops a same-named replacement from being
//! deleted, since a bare *after-the-fact* link-count check alone cannot distinguish "my own
//! unlink zeroed the original" from "a concurrent unlink already zeroed the original before I
//! touched a different object at this path," and both would otherwise look identical; (3)
//! after the unlink, re-stat the *same, already-open* descriptor as final corroboration that
//! its link count dropped to zero. This narrows, but does not perfectly close, the race:
//! step (2) and the unlink itself are still two separate syscalls with a small gap between
//! them, and true prevention (not merely detection) needs an OS-specific handle-relative
//! unlink (`openat`/`unlinkat` with `O_NOFOLLOW`). Where that guarantee cannot be established,
//! `mutate` reports `Err` rather than a possibly-false `Ok(())` - "refuse where the guarantee
//! cannot be established," not guess. Consequently only `FileKind::File` deletion is confirmed
//! this way; `cancellai-safety::mutation_executor` only ever requests it for that kind
//! (directories/symlinks are refused before reaching this seam at all - see that module's own
//! docs).
//!
//! ## Why this is still detection, and what changed underneath it (E21-S01)
//!
//! This module used to justify the residual by stating that the required capability did not
//! exist here: no reviewed FFI dependency, and `unsafe_code` forbidden workspace-wide by
//! ADR-0015. **That premise was superseded and the text was not updated.**
//! `cancellai-sealedfs` (E07-S07/E07-S09, ADR-0017) now exists, depends on `libc`, is the one
//! crate exempted from `unsafe_code = "forbid"`, and already implements `openat`/`renameat`/
//! `mkdirat` with `O_NOFOLLOW` plus a component-by-component handle-relative walk. Adding
//! `unlinkat` to it is a small extension well inside that crate's stated mandate, not a new
//! dependency decision.
//!
//! The consequence, recorded plainly because the 2026-09-03 target-engine review
//! (`docs/audits/2026-09-03-CODE_REVIEW.md`, `CR-TE-05`) found it: the risk ordering is
//! currently inverted. Writing one JSON key into Claude Code's own settings file is protected
//! by a retained no-follow handle; irreversibly deleting a user's file is not. The deletion
//! path is narrow rather than open - the three checks above refuse rather than deleting the
//! wrong object - but it is detection where prevention is now available.
//!
//! `E21-S07` carries the repair. Until it lands, this is a disclosed residual, not an
//! unavailable capability.
//!
//! ## Quarantine (E12-S01)
//!
//! [`MutationOperation::Quarantine`] re-adds the operation `E21-S07` removed (see that story's
//! note above the enum) with the confirmation technique it asked for: the same open-time and
//! immediately-before three-check shape `confirmed_delete_file_inner` uses, ending in a
//! `cancellai_sealedfs::rename_child_matching_unix_identity` handle-relative move instead of an
//! unlink. `docs/architecture/PLATFORM_MODEL.md`'s boundary rule ("crossing a filesystem/volume
//! boundary is explicit and normally prohibited... unless a dedicated operation has verified
//! semantics") is this operation - `cancellai-safety::mutation_executor` performs the explicit
//! same-device check (SI-018) before ever reaching this seam; `renameat(2)`'s own `EXDEV` is
//! only the backstop. Unix-only for now: Windows has no verified handle-relative rename yet
//! (mirroring `confirmed_delete_file`'s own Windows/Unix split before E20-S05 closed that gap
//! for deletion) and refuses explicitly rather than falling back to an unconfirmed path-based
//! move - a disclosed residual, not a silent gap.
//!
//! ## Crash recovery for `Quarantine`/`Archive` sidecars (E12-S01/E12-S03 rounds 3-5)
//!
//! Round 1 independent verifier review found that writing a move's sidecar *after* the move
//! left a real gap on a genuine crash (not merely a synchronous write failure, which round 1's
//! rollback repair handled correctly): nothing durable existed yet for a restart to recover
//! from. Round 3 inverts the ordering instead of patching around it: [`commit_move_with_sidecars`]
//! writes every sidecar's real, final content to a `.pending` name - durably (`fsync`ed content
//! *and*, since this repair, `fsync`ed directory entry) - *before* the move is even attempted.
//! The move becomes a plain rename; finalizing is a second, same-directory rename with no new
//! content write. A failure before the move means nothing was attempted at all.
//!
//! **What happens if finalizing fails after a successful move is a disclosed, deferred residual,
//! not an automated capability.** Rounds 3, 4 and 5 of independent verifier review each found a
//! genuine correctness hazard in three successive attempts at an automated recovery scanner that
//! would complete an interrupted finalize on restart (round 3: a destination name existing was
//! treated as proof a given pending sidecar belonged to it, letting a conflicting operation's
//! stale leftovers overwrite a legitimate record; round 4: the fix for that accepted an
//! already-finalized witness from an unrelated, older, completed operation as if it were a newer
//! operation's own proof; round 5: even a same-operation fix could finalize a witness before a
//! sibling's own finalize had succeeded, so a later retry discarded that sibling as an orphan).
//! Three real defects in the same mechanism was itself read as a signal rather than bad luck, and
//! the owner's decision was to stop iterating on an automated scanner here. What
//! [`commit_move_with_sidecars`] still guarantees on its own: a sidecar whose finalize failed is
//! left with its full, correct content durably recorded under its own `.pending` name - never
//! silently lost or silently wrong - so completing it (renaming each `.pending` name to its real
//! one) stays safe to do, by a human operator today and by a properly, independently verified
//! automated tool in a dedicated future story that can give this state machine the scrutiny three
//! rounds already showed it needs.
//!
//! ## Archive (E12-S03)
//!
//! [`MutationOperation::Archive`] moves an artifact into cancellAI's own archive store using
//! the exact same confirmed move `Quarantine` does (both share the same `confirmed_move_prepare`/
//! [`commit_move_with_sidecars`]). What differs is what gets recorded: a real, compressed
//! archive format needs a kernel-ring dependency this workspace does not carry (ADR-0019
//! requires a dedicated, reviewed ADR for one - the same bar `libc`/`cancellai-sealedfs` cleared
//! under ADR-0017), so this story does not implement byte compression. What it does implement:
//! the source's byte length, captured at the same open-time identity check every move already
//! performs, written as a plain-decimal sidecar (`<destination>.archive-length.json`); and,
//! since ADR-0030, a real SHA-256 digest of the source's bytes, also captured at open time and
//! written hex-encoded (`<destination>.archive-digest.json`).
//!
//! **E12-S03 round-1 independent verifier review** found that a length-only check accepted an
//! equal-length in-place corruption (all bytes changed, length untouched) as `Verified` - a
//! length mismatch alone is only ever a truncation/extension signal, not a content one. Round 1
//! repaired this with a fast, non-cryptographic FNV-1a fingerprint; round 2 independent verifier
//! review judged that insufficient for a CR4 predicate meant to authorize a future irreversible
//! source purge, and named the required repair: a reviewed kernel-ring digest decision (ADR-0030)
//! followed by a real collision-resistant digest. [`verify_archive_integrity`] now requires both
//! the recorded length *and* the recorded SHA-256 digest to match before returning `Verified`.

use std::path::{Path, PathBuf};

use crate::identity::IdentityToken;
#[cfg(any(test, unix))]
use crate::wsl::RuntimeEnvironment;
#[cfg(unix)]
use crate::wsl::{EnvironmentObserver, SystemEnvironmentObserver};

/// One class of real mutation this seam can perform.
///
/// `E21-S07` reduced this to exactly one variant, having removed an earlier `Quarantine` (a
/// bare `fs::rename`) and `DeleteDirectoryTree` (a bare `fs::remove_dir_all`) that were neither
/// identity-confirmed nor requested by any production caller - "re-adding either is that
/// story's job, together with the confirmation technique it needs." `Quarantine` below is that
/// re-addition for E12-S01: identity-confirmed, handle-relative, and requested by
/// `cancellai-safety::mutation_executor` for `ActionClass::Quarantine`. `DeleteDirectoryTree`
/// remains unadded - no story has asked for it yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationOperation {
    /// Permanently remove a regular file. Confirmed by open-file-descriptor identity *and*
    /// performed relative to a held, no-follow directory descriptor (see module docs).
    DeleteFile,
    /// Move a regular file into cancellAI's own quarantine store. Confirmed by identity and
    /// performed relative to held, no-follow directory descriptors on both ends (module docs,
    /// "Quarantine (E12-S01)"). `destination` is the full destination path - its parent
    /// directory must already exist as the caller's approved quarantine root. `record` is a
    /// contentless restore-metadata sidecar (`docs/architecture/PERSISTENCE_MODEL.md`'s
    /// quarantine store rules: identity/metadata only, never artifact payload content) written
    /// atomically alongside the move.
    Quarantine {
        destination: PathBuf,
        record: Vec<u8>,
    },
    /// Move an artifact out of cancellAI's quarantine store back to a destination outside it
    /// (E12-S02). The reverse of `Quarantine`'s move, without its sidecar - the destination
    /// here is the artifact's original provider location, not cancellAI's own store.
    Restore { destination: PathBuf },
    /// Move an artifact into cancellAI's own archive store (E12-S03). Same shape as
    /// `Quarantine` - `record` is the same kind of caller-composed, contentless metadata
    /// sidecar - plus a second, platform-authored length sidecar this seam writes on its own
    /// (see module docs, "Archive (E12-S03)"): a real compressed archive format is a disclosed
    /// residual, not implemented here (no new kernel-ring dependency without a dedicated ADR).
    Archive {
        destination: PathBuf,
        record: Vec<u8>,
    },
}

/// Why a real mutation attempt failed. Always the underlying OS error text, or this seam's
/// own identity-confirmation failure text - this seam does not otherwise interpret or
/// classify failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationError(pub String);

/// A sink for real filesystem mutation. `expected` is the identity the caller already
/// confirmed via `IdentityObserver` immediately before calling this (see module docs for how
/// [`SystemMutationExecutor`] uses it).
pub trait MutationExecutor: Send + Sync {
    fn mutate(
        &self,
        target: &Path,
        expected: &IdentityToken,
        operation: MutationOperation,
    ) -> Result<(), MutationError>;
}

/// The real, OS-backed executor. The only place in this crate - and, per
/// `scripts/check_mutation_boundary.py`, in this entire workspace outside this one file -
/// that calls a filesystem removal primitive directly.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemMutationExecutor;

impl MutationExecutor for SystemMutationExecutor {
    fn mutate(
        &self,
        target: &Path,
        expected: &IdentityToken,
        operation: MutationOperation,
    ) -> Result<(), MutationError> {
        match operation {
            MutationOperation::DeleteFile => confirmed_delete_file(target, expected),
            MutationOperation::Quarantine {
                destination,
                record,
            } => confirmed_quarantine_move(target, expected, &destination, &record),
            MutationOperation::Restore { destination } => {
                confirmed_restore_move(target, expected, &destination)
            }
            MutationOperation::Archive {
                destination,
                record,
            } => confirmed_archive_move(target, expected, &destination, &record),
        }
    }
}

/// Refuses a WSL2 guest, independent of the platform-specific check below. A WSL2 guest runs a
/// real Linux kernel, so the Unix confirmed-delete path in `confirmed_delete_file_inner` would
/// very likely work correctly there - but "very likely" is an inference, not this codebase's
/// own evidence (E20-S02/E20-S03 round-1 independent verifier review: neither this crate's WSL
/// detection nor its `/mnt` filesystem-context classification was ever consulted on the
/// deletion path, so a WSL2 guest silently took the identical, unverified-there Unix delete
/// route as native Linux - contradicting `docs/PLATFORMS.md`'s own claim that a non-tier-1
/// environment remains inspect-only). Pure and environment-independent so it is directly
/// unit-testable with a fabricated [`RuntimeEnvironment`] on any host, matching this crate's
/// `wsl` module's own split between real OS observation and testable pure logic.
#[cfg(any(test, unix))]
fn refuse_unverified_wsl2_mutation(env: RuntimeEnvironment) -> Result<(), MutationError> {
    if env == RuntimeEnvironment::Wsl2 {
        return Err(MutationError(
            "confirmed deletion is refused on a WSL2 guest: this codebase's own Unix mutation \
             path has not been independently verified there yet (E20-S02/E20-S03 residual, \
             SI-017/SI-018) - authority is reduced rather than inferred from generic Linux"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn confirmed_delete_file(target: &Path, expected: &IdentityToken) -> Result<(), MutationError> {
    refuse_unverified_wsl2_mutation(SystemEnvironmentObserver.detect())?;
    confirmed_delete_file_inner(target, expected, || {})
}

/// The real logic, parameterized by a hook that runs between the open-time identity
/// confirmation and the actual `remove_file` call. In production this hook is a no-op
/// (`confirmed_delete_file` above); tests use it to deterministically reproduce the exact
/// race the round-1 review found - swapping the target *after* it has been confirmed open
/// but *before* it is unlinked - without needing real thread-timing luck.
#[cfg(unix)]
fn confirmed_delete_file_inner(
    target: &Path,
    expected: &IdentityToken,
    between_open_and_unlink: impl FnOnce(),
) -> Result<(), MutationError> {
    use std::os::unix::fs::MetadataExt;

    // This Unix-only deletion path has no verified interpretation of a `Windows` identity
    // token (Windows has its own `confirmed_delete_file_inner` below, E20-S05). A caller
    // passing one anyway - only possible via a synthetically constructed `IdentityToken` in a
    // test, never a real `SystemIdentityObserver` result on a Unix host - gets a typed
    // refusal, not a panic, matching this codebase's fail-closed posture for an unexpected
    // identity shape (SI-017).
    let IdentityToken::Unix {
        device: expected_device,
        inode: expected_inode,
        modified: expected_modified,
        modified_nanos: expected_modified_nanos,
        ..
    } = expected
    else {
        return Err(MutationError(
            "confirmed file deletion on this platform requires a Unix identity token".to_string(),
        ));
    };

    let file = std::fs::File::open(target)
        .map_err(|e| MutationError(format!("could not open target for confirmed deletion: {e}")))?;
    let before = file
        .metadata()
        .map_err(|e| MutationError(format!("could not stat open target before deletion: {e}")))?;
    // E07-S05: device+inode alone was found, on real Linux, insufficient - a delete-and-
    // recreate that happened to land within the same wall-clock second reused the just-freed
    // inode too, so a swapped-in replacement could pass this check purely by chance. Comparing
    // the sub-second modification-time remainder as well (the same disambiguator
    // `IdentityToken::Unix::modified_nanos` now carries) catches what device+inode+whole-second
    // `modified` cannot.
    if before.dev() != *expected_device
        || before.ino() != *expected_inode
        || before.mtime() != expected_modified.0 as i64
        || before.mtime_nsec() as u32 != *expected_modified_nanos
    {
        return Err(MutationError(
            "target identity changed between revalidation and deletion (open-time check)"
                .to_string(),
        ));
    }

    between_open_and_unlink();

    // A second, independent, fresh path lookup (not through the held fd) immediately before
    // the actual unlink - this is what actually catches a swap that happened after the
    // open-time check above (as in `between_open_and_unlink`): a bare nlink check *after*
    // `remove_file` cannot distinguish "my own unlink zeroed the original's link count" from
    // "a concurrent unlink of the original already zeroed it before I ever touched the
    // (different) object now sitting at this path" - both produce the same post-hoc nlink
    // reading. Refusing here, *before* calling `remove_file` at all, is what keeps a
    // same-named replacement from being deleted as collateral damage.
    let just_before = std::fs::symlink_metadata(target).map_err(|e| {
        MutationError(format!(
            "could not re-stat target immediately before deletion: {e}"
        ))
    })?;
    if just_before.dev() != *expected_device
        || just_before.ino() != *expected_inode
        || just_before.mtime() != expected_modified.0 as i64
        || just_before.mtime_nsec() as u32 != *expected_modified_nanos
    {
        return Err(MutationError(
            "target identity changed immediately before deletion (path re-check failed); refusing to delete a different object".to_string(),
        ));
    }

    // E21-S07: the unlink itself is now issued relative to a directory descriptor opened once,
    // with `O_NOFOLLOW` at every component, rather than through the target's path. That is the
    // difference between detecting the swap and preventing it: `std::fs::remove_file(target)`
    // re-resolved the whole path through the kernel's normal, link-following name resolution
    // every time it ran, so a rename or symlink-swap of any intermediate directory in the
    // window above redirected the removal. `cancellai-sealedfs` (ADR-0017) cannot be redirected
    // that way, because there is no path left to redirect.
    //
    // The identity comparison inside `unlink_child_matching_unix_identity` is deliberately not
    // a replacement for the two checks above - it is a third, handle-relative one, and the
    // held file descriptor's post-unlink link-count check below still runs. Defence in depth
    // over one clever check, as everywhere else in this kernel.
    let parent = target.parent().ok_or_else(|| {
        MutationError("target has no parent directory; refusing to delete".to_string())
    })?;
    let file_name = target.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        MutationError(
            "target's file name is not representable as UTF-8; refusing to delete a name \
                 this seam cannot bind handle-relatively"
                .to_string(),
        )
    })?;
    let sealed = cancellai_sealedfs::SealedRoot::bind_existing(parent).map_err(|e| {
        MutationError(format!(
            "could not bind the target's parent directory without following a link: {e}"
        ))
    })?;
    sealed
        .unlink_child_matching_unix_identity(file_name, *expected_device, *expected_inode)
        .map_err(|e| MutationError(format!("delete failed: {e}")))?;

    // Final corroboration via the fd opened at the very start: an open fd stays valid after
    // its directory entry is unlinked (Unix semantics), so if the unlink above really removed
    // the object this fd holds, that object's link count is now 0.
    let after = file
        .metadata()
        .map_err(|e| MutationError(format!("could not stat held handle after deletion: {e}")))?;
    if after.nlink() != 0 {
        return Err(MutationError(
            "deletion removed a different filesystem object than the one confirmed open \
             (post-deletion link-count check failed); the intended target may still exist"
                .to_string(),
        ));
    }
    Ok(())
}

/// The quarantine-move counterpart of [`confirmed_delete_file_inner`]: same open-time and
/// immediately-before-the-real-syscall identity checks around a hook, ending in a handle-relative
/// `renameat` (via `cancellai_sealedfs::SealedRoot::rename_child_matching_unix_identity`) instead
/// of an unlink, plus a contentless restore-metadata sidecar written atomically once the move
/// itself has succeeded.
#[cfg(unix)]
fn confirmed_quarantine_move(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
    record: &[u8],
) -> Result<(), MutationError> {
    refuse_unverified_wsl2_mutation(SystemEnvironmentObserver.detect())?;
    confirmed_quarantine_move_inner(source, expected, destination, record, || {})
}

#[cfg(unix)]
fn confirmed_quarantine_move_inner(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
    record: &[u8],
    between_open_and_move: impl FnOnce(),
) -> Result<(), MutationError> {
    let prepared =
        confirmed_move_prepare(source, expected, destination, false, between_open_and_move)?;
    commit_move_with_sidecars(
        prepared,
        &[Sidecar {
            suffix: "quarantine-record",
            bytes: record.to_vec(),
        }],
    )
}

/// The restore counterpart of [`confirmed_quarantine_move`] (E12-S02): the identical
/// identity-confirmed, no-clobber move, reversed in direction and *without* writing any
/// sidecar into the destination - the destination here is the artifact's original provider
/// location, not cancellAI's own quarantine store, and nothing of cancellAI's belongs there.
#[cfg(unix)]
fn confirmed_restore_move(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
) -> Result<(), MutationError> {
    refuse_unverified_wsl2_mutation(SystemEnvironmentObserver.detect())?;
    confirmed_restore_move_inner(source, expected, destination, || {})
}

#[cfg(unix)]
fn confirmed_restore_move_inner(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
    between_open_and_move: impl FnOnce(),
) -> Result<(), MutationError> {
    let prepared =
        confirmed_move_prepare(source, expected, destination, false, between_open_and_move)?;
    commit_prepared_move(&prepared)
}

/// The archive counterpart of [`confirmed_quarantine_move`] (E12-S03): the identical
/// identity-confirmed, no-clobber move, plus three sidecars - the caller-supplied opaque
/// `record` (format/version/identity metadata, exactly like `Quarantine`'s own), a
/// platform-authored `.archive-length` sidecar recording the source's byte length observed at
/// open time, and a platform-authored `.archive-digest` sidecar recording a SHA-256 digest of
/// the source's bytes, also observed at open time (ADR-0030, E12-S03 round-2 independent
/// verifier review: a length-only check accepted an equal-length in-place corruption as
/// `Verified`, and round 1's own FNV-1a fingerprint repair was itself judged insufficient for a
/// CR4 pre-purge predicate - see [`digest_content`]'s own doc, and [`verify_archive_integrity`]
/// for how all three sidecars are checked together before any future purge).
#[cfg(unix)]
fn confirmed_archive_move(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
    record: &[u8],
) -> Result<(), MutationError> {
    refuse_unverified_wsl2_mutation(SystemEnvironmentObserver.detect())?;
    confirmed_archive_move_inner(source, expected, destination, record, || {})
}

#[cfg(unix)]
fn confirmed_archive_move_inner(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
    record: &[u8],
    between_open_and_move: impl FnOnce(),
) -> Result<(), MutationError> {
    let prepared =
        confirmed_move_prepare(source, expected, destination, true, between_open_and_move)?;
    let digest = prepared
        .digest
        .expect("archive always requests compute_digest = true");
    let source_length = prepared.source_length;
    commit_move_with_sidecars(
        prepared,
        &[
            Sidecar {
                suffix: "archive-record",
                bytes: record.to_vec(),
            },
            Sidecar {
                suffix: ARCHIVE_LENGTH_SIDECAR_SUFFIX,
                bytes: source_length.to_string().into_bytes(),
            },
            Sidecar {
                suffix: ARCHIVE_DIGEST_SIDECAR_SUFFIX,
                bytes: encode_hex(&digest).into_bytes(),
            },
        ],
    )
}

/// The suffix [`confirmed_archive_move_inner`] writes the length sidecar under and
/// [`verify_archive_integrity`] reads it back from - one constant so the writer and reader
/// cannot silently drift apart by naming convention alone. Not itself platform-gated: the
/// verify side is plain, portable `std::fs` and has no OS-specific requirement of its own.
const ARCHIVE_LENGTH_SIDECAR_SUFFIX: &str = "archive-length";

/// The suffix the SHA-256 digest sidecar is written under and read back from (ADR-0030,
/// E12-S03 round-2 repair) - see [`digest_content`] for what it guarantees.
const ARCHIVE_DIGEST_SIDECAR_SUFFIX: &str = "archive-digest";

/// Hex-encodes `bytes` in lowercase, one `%02x` pair per byte - the same convention
/// `cancellai-safety::knowledge_bundle`'s own `encode_hex` uses for its content digests, kept
/// as a small local copy rather than a cross-crate dependency in the wrong direction
/// (`cancellai-safety` depends on `cancellai-platform`, not the reverse).
fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Everything [`confirmed_move_prepare`] confirms and binds before the actual OS-level move is
/// attempted: both ends' held [`cancellai_sealedfs::SealedRoot`]s, the bare child names on each
/// side, the source's byte length, and - only when requested - a SHA-256 digest of the source's
/// content, all captured at the one open-time, already-identity-confirmed read this seam
/// performs. Splitting "confirm and prepare" from "commit" ([`commit_prepared_move`]) is what
/// makes it possible to write a move's sidecar content *before* the move itself rather than
/// after - see [`commit_move_with_sidecars`] for why that ordering is what actually closes the
/// crash-recovery gap E12-S01/E12-S03 round 2 independent verifier review found.
#[cfg(unix)]
struct PreparedMove {
    source_sealed: cancellai_sealedfs::SealedRoot,
    dest_sealed: cancellai_sealedfs::SealedRoot,
    source_name: String,
    dest_name: String,
    source_length: u64,
    digest: Option<[u8; 32]>,
    device: u64,
    inode: u64,
}

/// The shared identity-confirmed checks and binding every move (`Quarantine`/`Restore`/
/// `Archive`) performs before the actual `renameat`: open-time and immediately-before checks
/// around `between_open_and_move` (mirroring [`confirmed_delete_file_inner`]'s own two checks
/// around the unlink), then binding both ends' [`cancellai_sealedfs::SealedRoot`]s. Stops short
/// of performing the move itself - see [`commit_prepared_move`]/[`commit_move_with_sidecars`].
#[cfg(unix)]
fn confirmed_move_prepare(
    source: &Path,
    expected: &IdentityToken,
    destination: &Path,
    compute_digest: bool,
    between_open_and_move: impl FnOnce(),
) -> Result<PreparedMove, MutationError> {
    use std::os::unix::fs::MetadataExt;

    // This Unix-only move path has no verified interpretation of a `Windows` identity token -
    // see the identical reasoning in `confirmed_delete_file_inner` above.
    let IdentityToken::Unix {
        device: expected_device,
        inode: expected_inode,
        modified: expected_modified,
        modified_nanos: expected_modified_nanos,
        ..
    } = expected
    else {
        return Err(MutationError(
            "confirmed move on this platform requires a Unix identity token".to_string(),
        ));
    };

    let file = std::fs::File::open(source)
        .map_err(|e| MutationError(format!("could not open target for move: {e}")))?;
    let before = file
        .metadata()
        .map_err(|e| MutationError(format!("could not stat open target before move: {e}")))?;
    if before.dev() != *expected_device
        || before.ino() != *expected_inode
        || before.mtime() != expected_modified.0 as i64
        || before.mtime_nsec() as u32 != *expected_modified_nanos
    {
        return Err(MutationError(
            "target identity changed between revalidation and move (open-time check)".to_string(),
        ));
    }
    let source_length = before.len();

    // Read here, against the same already-open, already-identity-confirmed descriptor the
    // checks above just validated - not a second, separately-racy open of `source` by path -
    // and strictly before `between_open_and_move`, so the digest always describes the exact
    // bytes those checks confirmed, never a later, possibly swapped, object.
    let digest = if compute_digest {
        Some(digest_content(&file).map_err(|e| {
            MutationError(format!(
                "could not read target content for archive digest: {e}"
            ))
        })?)
    } else {
        None
    };

    between_open_and_move();

    // A second, independent, fresh path lookup immediately before the actual move - the same
    // reasoning as `confirmed_delete_file_inner`'s `just_before` re-check.
    let just_before = std::fs::symlink_metadata(source).map_err(|e| {
        MutationError(format!(
            "could not re-stat target immediately before move: {e}"
        ))
    })?;
    if just_before.dev() != *expected_device
        || just_before.ino() != *expected_inode
        || just_before.mtime() != expected_modified.0 as i64
        || just_before.mtime_nsec() as u32 != *expected_modified_nanos
    {
        return Err(MutationError(
            "target identity changed immediately before move (path re-check failed); refusing \
             to move a different object"
                .to_string(),
        ));
    }

    let source_parent = source.parent().ok_or_else(|| {
        MutationError("target has no parent directory; refusing to move".to_string())
    })?;
    let source_name = source.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        MutationError(
            "target's file name is not representable as UTF-8; refusing to move a name this \
             seam cannot bind handle-relatively"
                .to_string(),
        )
    })?;
    let dest_parent = destination.parent().ok_or_else(|| {
        MutationError("destination has no parent directory; refusing to move".to_string())
    })?;
    let dest_name = destination
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            MutationError(
                "destination's file name is not representable as UTF-8; refusing to move a \
                 name this seam cannot bind handle-relatively"
                    .to_string(),
            )
        })?;

    let source_sealed =
        cancellai_sealedfs::SealedRoot::bind_existing(source_parent).map_err(|e| {
            MutationError(format!(
                "could not bind the target's parent directory without following a link: {e}"
            ))
        })?;
    let dest_sealed = cancellai_sealedfs::SealedRoot::bind_existing(dest_parent).map_err(|e| {
        MutationError(format!(
            "could not bind the destination directory without following a link: {e}"
        ))
    })?;

    Ok(PreparedMove {
        source_sealed,
        dest_sealed,
        source_name: source_name.to_string(),
        dest_name: dest_name.to_string(),
        source_length,
        digest,
        device: *expected_device,
        inode: *expected_inode,
    })
}

/// Performs the actual OS-level move [`confirmed_move_prepare`] already confirmed and bound -
/// a no-clobber `renameat` via `cancellai_sealedfs::rename_child_matching_unix_identity`. Used
/// directly by `Restore` (no sidecar); [`commit_move_with_sidecars`] wraps it for
/// `Quarantine`/`Archive`.
#[cfg(unix)]
fn commit_prepared_move(prepared: &PreparedMove) -> Result<(), MutationError> {
    prepared
        .source_sealed
        .rename_child_matching_unix_identity(
            &prepared.source_name,
            &prepared.dest_sealed,
            &prepared.dest_name,
            prepared.device,
            prepared.inode,
        )
        .map_err(|e| MutationError(format!("move failed: {e}")))
}

/// One sidecar a move needs to write, named by its suffix (`<destination>.<suffix>.json`) and
/// content.
#[cfg(unix)]
struct Sidecar {
    suffix: &'static str,
    bytes: Vec<u8>,
}

/// Writes every sidecar in `sidecars` durably *before* the move, performs the move, then
/// finalizes each sidecar's name (E12-S01/E12-S03 round-3 repair, SI-020).
///
/// Round 1 wrote sidecars *after* the move and rolled the move back if a write then failed -
/// which correctly turned a synchronous write failure into "nothing changed," but round 2
/// independent verifier review found the gap rollback cannot close: a real crash or power loss
/// between a successful rename and its sidecar write leaves a moved, unrecorded object with no
/// restart recovery, because nothing durable existed yet to recover *from*.
///
/// The fix inverts the ordering. Each sidecar's real, final content is written to a `.pending`
/// name - durably (`write_new_child_atomically` `fsync`s both the file's content and, since the
/// E12-S01/E12-S03 round-3 repair, the directory entry too) - *before* the move is even
/// attempted, so the content is already crash-safe no matter what happens next. The move is
/// then just a rename; finalizing is a second, same-directory rename (pending name -> real
/// name, [`cancellai_sealedfs::SealedRoot::finalize_own_child`]) that writes no new content and
/// so has almost nothing left to fail. If the move itself fails, every pending sidecar is
/// removed ([`cancellai_sealedfs::SealedRoot::remove_own_child`]) and nothing has changed
/// externally - the same "refuse and leave things exactly as they were" shape as every other
/// check in this seam.
///
/// **Automated restart recovery after a successful move but a failed/interrupted finalize is a
/// disclosed, deferred residual, not implemented here.** Rounds 3-5 of this epic's independent
/// verifier review each found a genuine correctness hazard in three successive attempts at an
/// automated recovery scanner (identity confusion between unrelated operations sharing a
/// destination name, then a finalization-ordering hazard within one operation's own group) -
/// three real defects in the same mechanism is itself a signal, not merely bad luck, and the
/// owner's decision was to stop iterating on it here rather than ship a fourth attempt. What
/// this function still guarantees on its own: if finalizing fails, every sidecar's content is
/// already durably recorded under its own `.pending` name - self-describing, correct, and never
/// silently wrong - so completing the operation (renaming each `.pending` name to its real one)
/// is always safe to do, by a human operator today and by a properly independently-verified
/// automated tool in a dedicated future story.
#[cfg(unix)]
fn commit_move_with_sidecars(
    prepared: PreparedMove,
    sidecars: &[Sidecar],
) -> Result<(), MutationError> {
    let names: Vec<(String, String)> = sidecars
        .iter()
        .map(|s| {
            (
                format!("{}.{}.json.pending", prepared.dest_name, s.suffix),
                format!("{}.{}.json", prepared.dest_name, s.suffix),
            )
        })
        .collect();

    let mut written_pending_names: Vec<&str> = Vec::new();
    for (sidecar, (pending_name, _)) in sidecars.iter().zip(names.iter()) {
        let tmp_name = format!("{pending_name}.tmp");
        if let Err(e) =
            prepared
                .dest_sealed
                .write_new_child_atomically(&tmp_name, pending_name, &sidecar.bytes)
        {
            // A later sidecar failing to write durably must not leave the earlier ones - which
            // did already succeed - behind: best-effort cleanup, since a stray pending sidecar
            // left here is merely untidy, not unsafe (its content never becomes visible under a
            // real name unless something later finalizes it).
            for already_written in &written_pending_names {
                let _ = prepared.dest_sealed.remove_own_child(already_written);
            }
            return Err(MutationError(format!(
                "could not durably record the {} sidecar ahead of the move: {e}",
                sidecar.suffix
            )));
        }
        written_pending_names.push(pending_name);
    }

    if let Err(move_err) = commit_prepared_move(&prepared) {
        for (pending_name, _) in &names {
            // Best-effort: the move itself already failed and nothing external changed - a
            // pending sidecar left behind here is merely untidy, not unsafe.
            let _ = prepared.dest_sealed.remove_own_child(pending_name);
        }
        return Err(move_err);
    }

    let mut finalize_errors = Vec::new();
    for (pending_name, final_name) in &names {
        if let Err(e) = prepared
            .dest_sealed
            .finalize_own_child(pending_name, final_name)
        {
            finalize_errors.push(format!("{final_name}: {e}"));
        }
    }
    if finalize_errors.is_empty() {
        Ok(())
    } else {
        Err(MutationError(format!(
            "the move itself succeeded and every sidecar's content was already durably \
             recorded before it, but finalizing its name did not complete for: {} - the \
             artifact is not lost; its content is durably recorded under its own pending name \
             and completing the rename is always safe to retry",
            finalize_errors.join("; ")
        )))
    }
}

/// No verified handle-relative rename exists for Windows yet (mirroring `confirmed_delete_file`'s
/// own pre-E20-S05 Unix/Windows split) - refused rather than falling back to an unconfirmed,
/// path-based `std::fs::rename` (E12-S01 residual, disclosed rather than silently gapped).
#[cfg(windows)]
fn confirmed_quarantine_move(
    _source: &Path,
    _expected: &IdentityToken,
    _destination: &Path,
    _record: &[u8],
) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed quarantine move is not implemented on this platform yet (E12-S01 residual - \
         Windows quarantine is a follow-up story)"
            .to_string(),
    ))
}

#[cfg(not(any(unix, windows)))]
fn confirmed_quarantine_move(
    _source: &Path,
    _expected: &IdentityToken,
    _destination: &Path,
    _record: &[u8],
) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed quarantine move is not implemented on this platform".to_string(),
    ))
}

/// No verified handle-relative rename exists for Windows yet - the restore-side counterpart
/// of `confirmed_quarantine_move`'s own Windows refusal (E12-S02 residual, disclosed rather
/// than silently gapped).
#[cfg(windows)]
fn confirmed_restore_move(
    _source: &Path,
    _expected: &IdentityToken,
    _destination: &Path,
) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed restore move is not implemented on this platform yet (E12-S02 residual - \
         Windows restore is a follow-up story)"
            .to_string(),
    ))
}

#[cfg(not(any(unix, windows)))]
fn confirmed_restore_move(
    _source: &Path,
    _expected: &IdentityToken,
    _destination: &Path,
) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed restore move is not implemented on this platform".to_string(),
    ))
}

/// No verified handle-relative rename exists for Windows yet - the archive-side counterpart
/// of `confirmed_quarantine_move`'s own Windows refusal (E12-S03 residual, disclosed rather
/// than silently gapped).
#[cfg(windows)]
fn confirmed_archive_move(
    _source: &Path,
    _expected: &IdentityToken,
    _destination: &Path,
    _record: &[u8],
) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed archive move is not implemented on this platform yet (E12-S03 residual - \
         Windows archive is a follow-up story)"
            .to_string(),
    ))
}

#[cfg(not(any(unix, windows)))]
fn confirmed_archive_move(
    _source: &Path,
    _expected: &IdentityToken,
    _destination: &Path,
    _record: &[u8],
) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed archive move is not implemented on this platform".to_string(),
    ))
}

#[cfg(windows)]
fn confirmed_delete_file(target: &Path, expected: &IdentityToken) -> Result<(), MutationError> {
    confirmed_delete_file_inner(target, expected, || {})
}

/// The Windows counterpart of the Unix `confirmed_delete_file_inner` above (E20-S05,
/// ADR-0020's own residual: "the deletion path is narrow rather than open" - this closes that
/// gap for Windows the same way E21-S07 closed it for Unix). Same three-check shape: an
/// open-time identity confirmation against a retained handle, a fresh path-based re-check
/// immediately before the actual delete call, and a post-delete corroboration against the
/// *same* retained handle rather than a fresh reopen. `cancellai_sealedfs::SealedRoot`'s
/// `NtCreateFile`-based, handle-relative child open (see that crate's `windows_sealed` module)
/// plays the same role `openat`/`O_NOFOLLOW` plays on Unix: the actual delete call cannot be
/// redirected by a rename/reparse-point swap of any path component, because no path is
/// resolved for it at all.
#[cfg(windows)]
fn confirmed_delete_file_inner(
    target: &Path,
    expected: &IdentityToken,
    between_open_and_unlink: impl FnOnce(),
) -> Result<(), MutationError> {
    // This Windows-only deletion path has no verified interpretation of a `Unix` identity
    // token - see the identical reasoning in the Unix implementation above, mirrored here for
    // the same fail-closed reason (SI-017).
    let IdentityToken::Windows {
        volume_serial_number: expected_volume,
        file_index: expected_file_index,
        modified_ticks: expected_modified_ticks,
        ..
    } = expected
    else {
        return Err(MutationError(
            "confirmed file deletion on this platform requires a Windows identity token"
                .to_string(),
        ));
    };

    let (file, before) = cancellai_sealedfs::open_and_observe_identity(target)
        .map_err(|e| MutationError(format!("could not open target for confirmed deletion: {e}")))?;
    if before.volume_serial_number != *expected_volume
        || before.file_index != *expected_file_index
        || before.last_write_time_ticks != *expected_modified_ticks
    {
        return Err(MutationError(
            "target identity changed between revalidation and deletion (open-time check)"
                .to_string(),
        ));
    }

    between_open_and_unlink();

    // A second, independent, fresh path lookup immediately before the actual delete call -
    // the same reasoning as the Unix path's `just_before` re-check: this is what catches a
    // swap that happened after the open-time check above, before `remove_file`/its Windows
    // equivalent has any chance to act on the wrong object.
    let just_before = cancellai_sealedfs::observe_identity(target).map_err(|e| {
        MutationError(format!(
            "could not re-observe target immediately before deletion: {e}"
        ))
    })?;
    if just_before.volume_serial_number != *expected_volume
        || just_before.file_index != *expected_file_index
        || just_before.last_write_time_ticks != *expected_modified_ticks
    {
        return Err(MutationError(
            "target identity changed immediately before deletion (path re-check failed); \
             refusing to delete a different object"
                .to_string(),
        ));
    }

    let parent = target.parent().ok_or_else(|| {
        MutationError("target has no parent directory; refusing to delete".to_string())
    })?;
    let file_name = target.file_name().and_then(|n| n.to_str()).ok_or_else(|| {
        MutationError(
            "target's file name is not representable as UTF-8; refusing to delete a name \
                 this seam cannot bind handle-relatively"
                .to_string(),
        )
    })?;
    let sealed = cancellai_sealedfs::SealedRoot::bind_existing(parent).map_err(|e| {
        MutationError(format!(
            "could not bind the target's parent directory without following a link: {e}"
        ))
    })?;
    sealed
        .unlink_child_matching_windows_identity(file_name, *expected_volume, *expected_file_index)
        .map_err(|e| MutationError(format!("delete failed: {e}")))?;

    // Final corroboration via the handle opened at the very start - queried *before* it is
    // dropped, since a closed handle cannot itself be queried. `unlink_child_matching_windows_
    // identity` only marks the object for deletion (it does not itself hold the last handle);
    // actual removal happens once every handle to it closes, which this function's own `file`
    // still is at this point.
    let pending = cancellai_sealedfs::SealedRoot::is_delete_pending(&file).map_err(|e| {
        MutationError(format!(
            "could not confirm deletion via the held handle: {e}"
        ))
    })?;
    drop(file);
    if !pending {
        return Err(MutationError(
            "deletion did not mark the confirmed handle for removal (post-deletion check \
             failed); the intended target may still exist"
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn confirmed_delete_file(_target: &Path, _expected: &IdentityToken) -> Result<(), MutationError> {
    Err(MutationError(
        "confirmed file deletion is not implemented on this platform".to_string(),
    ))
}

/// What checking an archived artifact's length and content-digest sidecars (E12-S03) found.
/// Every branch other than [`Self::Verified`] is a named refusal - there is no fallback that
/// treats "could not check" as "probably fine" (this workspace's fail-closed posture, applied to
/// integrity verification rather than to a mutation decision).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveIntegrity {
    /// The archived copy's current length and content digest both match what was recorded
    /// when it was archived.
    Verified,
    /// The archived copy's current length differs from what was recorded at archive time - a
    /// truncation or extension since then. Refuse to trust it.
    LengthMismatch { recorded: u64, actual: u64 },
    /// The archived copy's length is unchanged but its content digest is not (E12-S03 round-1
    /// independent verifier review's exact finding: an equal-length in-place corruption that a
    /// length-only check accepted as `Verified`). Refuse to trust it.
    DigestMismatch { recorded: String, actual: String },
    /// The archived copy no longer exists, or could not be examined, where the sidecars say it
    /// should.
    ArchivedCopyUnreadable(String),
    /// A sidecar (length or digest) is missing or could not be read.
    RecordUnreadable(String),
    /// A sidecar's content is not a validly recorded value.
    RecordMalformed(String),
}

/// Streams `reader`'s full content through SHA-256 in bounded-size chunks, so digesting a large
/// archived artifact never has to hold its entire content in memory at once (ADR-0030, E12-S03
/// round-2 repair). Supersedes round 1's FNV-1a `fingerprint_content`, which round 2 independent
/// verifier review judged insufficient for a CR4 predicate meant to authorize a future
/// irreversible source purge - this is a genuine cryptographic digest, not merely a fast
/// fingerprint, so it is not run alongside the one it replaces.
fn digest_content(mut reader: impl std::io::Read) -> std::io::Result<[u8; 32]> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = buf
            .get(..n)
            .expect("std::io::Read::read never returns more than the buffer it was given");
        hasher.update(chunk);
    }
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&hasher.finalize());
    Ok(digest)
}

/// Verifies an archived artifact's recorded length and content digest both still match its
/// current ones (`docs/architecture/PERSISTENCE_MODEL.md`'s "Archive integrity must be verified
/// before any source purge"). Not wired to a real purge caller yet, since none exists - E12-S04
/// is blocked on E13-S02.
///
/// Plain, portable `std::fs`/`std::io` reads, not the handle-relative no-follow discipline the
/// move side uses: this function is read-only inspection with no side effect to protect, unlike
/// a real mutation.
pub fn verify_archive_integrity(archived_path: &Path) -> ArchiveIntegrity {
    let sidecar_path = |suffix: &str| {
        let mut name = archived_path
            .file_name()
            .map(std::ffi::OsStr::to_os_string)
            .unwrap_or_default();
        name.push(format!(".{suffix}.json"));
        archived_path.with_file_name(name)
    };

    let recorded_length_text =
        match std::fs::read_to_string(sidecar_path(ARCHIVE_LENGTH_SIDECAR_SUFFIX)) {
            Ok(text) => text,
            Err(e) => return ArchiveIntegrity::RecordUnreadable(e.to_string()),
        };
    let recorded_length: u64 = match recorded_length_text.trim().parse() {
        Ok(value) => value,
        Err(e) => return ArchiveIntegrity::RecordMalformed(e.to_string()),
    };
    let recorded_digest_text =
        match std::fs::read_to_string(sidecar_path(ARCHIVE_DIGEST_SIDECAR_SUFFIX)) {
            Ok(text) => text.trim().to_string(),
            Err(e) => return ArchiveIntegrity::RecordUnreadable(e.to_string()),
        };
    if recorded_digest_text.len() != 64
        || !recorded_digest_text.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return ArchiveIntegrity::RecordMalformed(
            "archive-digest sidecar is not exactly 64 lowercase hex characters".to_string(),
        );
    }

    let actual_length = match std::fs::metadata(archived_path) {
        Ok(meta) => meta.len(),
        Err(e) => return ArchiveIntegrity::ArchivedCopyUnreadable(e.to_string()),
    };
    if actual_length != recorded_length {
        return ArchiveIntegrity::LengthMismatch {
            recorded: recorded_length,
            actual: actual_length,
        };
    }

    let archived_file = match std::fs::File::open(archived_path) {
        Ok(f) => f,
        Err(e) => return ArchiveIntegrity::ArchivedCopyUnreadable(e.to_string()),
    };
    let actual_digest = match digest_content(archived_file) {
        Ok(value) => value,
        Err(e) => return ArchiveIntegrity::ArchivedCopyUnreadable(e.to_string()),
    };
    let actual_digest_text = encode_hex(&actual_digest);
    if actual_digest_text == recorded_digest_text {
        ArchiveIntegrity::Verified
    } else {
        ArchiveIntegrity::DigestMismatch {
            recorded: recorded_digest_text,
            actual: actual_digest_text,
        }
    }
}

/// Test-only seam: synthesize a mutation outcome for a specific path without touching the
/// real filesystem - the fault-injection double this story's verification contract names
/// ("fault-injection tests"). A path with no fact explicitly `set` succeeds, since a
/// mutation double that silently fails paths the test never configured to fail would hide
/// exactly the class of bug fault injection exists to find.
#[derive(Debug, Default)]
pub struct SyntheticMutationExecutor {
    outcomes: std::collections::BTreeMap<std::path::PathBuf, Result<(), MutationError>>,
}

impl SyntheticMutationExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(
        &mut self,
        path: impl Into<std::path::PathBuf>,
        outcome: Result<(), MutationError>,
    ) -> &mut Self {
        self.outcomes.insert(path.into(), outcome);
        self
    }
}

impl MutationExecutor for SyntheticMutationExecutor {
    fn mutate(
        &self,
        target: &Path,
        _expected: &IdentityToken,
        _operation: MutationOperation,
    ) -> Result<(), MutationError> {
        self.outcomes.get(target).cloned().unwrap_or(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuse_unverified_wsl2_mutation_refuses_on_wsl2() {
        let err = refuse_unverified_wsl2_mutation(RuntimeEnvironment::Wsl2)
            .expect_err("a WSL2 environment must refuse confirmed deletion");
        assert!(err.0.contains("WSL2"));
    }

    #[test]
    fn refuse_unverified_wsl2_mutation_allows_native() {
        refuse_unverified_wsl2_mutation(RuntimeEnvironment::Native)
            .expect("a native (non-WSL2) environment must not be refused by this gate");
    }

    #[cfg(unix)]
    struct TempDir(std::path::PathBuf);

    #[cfg(unix)]
    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            // Canonicalized, because E21-S07's handle-relative unlink refuses to traverse a
            // symlinked path component - and on macOS `std::env::temp_dir()` returns
            // `/var/folders/...`, where `/var` is itself a link to `/private/var`. That refusal
            // is the intended behaviour, not a test problem: E07-S09 already decided a provider
            // root reached through an intermediate link may not carry destructive authority, and
            // `cancellai-cli` proves the real root link-free before establishing it. The test
            // tree simply has to meet the same bar the production path already meets.
            // `a_symlinked_intermediate_component_refuses_the_delete` below pins the refusal
            // itself, so canonicalizing here does not hide it.
            let base = std::fs::canonicalize(std::env::temp_dir())
                .unwrap_or_else(|_| std::env::temp_dir());
            let dir = base.join(format!(
                "cancellai-mutation-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    #[cfg(unix)]
    impl Drop for TempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[cfg(unix)]
    fn identity_of(path: &Path) -> IdentityToken {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(path).expect("stat path for test identity");
        IdentityToken::Unix {
            device: meta.dev(),
            inode: meta.ino(),
            kind: crate::identity::FileKind::File,
            // The real mtime/nanos, not a placeholder: `confirmed_delete_file_inner` (E07-S05)
            // now compares both against the live file's own metadata, so a hardcoded
            // `Timestamp(0)` here would make every legitimate (no-swap) test below fail this
            // helper's own identity capture, not just the swap-detection tests that want it to.
            modified: crate::clock::Timestamp(meta.mtime() as u64),
            modified_nanos: meta.mtime_nsec() as u32,
        }
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_deletes_a_real_file_confirmed_by_identity() {
        let dir = TempDir::new("delete-confirmed");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);

        let executor = SystemMutationExecutor;
        executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect("delete should succeed");
        assert!(!file.exists());
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_reports_the_os_error_for_a_missing_target() {
        let dir = TempDir::new("missing-target");
        let missing = dir.path("does-not-exist");
        // Any well-formed expected token: open() fails before it would ever be consulted.
        let expected = IdentityToken::Unix {
            device: 0,
            inode: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_nanos: 0,
        };
        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&missing, &expected, MutationOperation::DeleteFile)
            .expect_err("deleting a missing file must fail, not silently succeed");
        assert!(!err.0.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_delete_rejects_a_target_already_swapped_before_open() {
        let dir = TempDir::new("swapped-before-open");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file); // captured identity of the ORIGINAL

        // Swap before any deletion attempt: replace the original with a different file at
        // the same path (same as an attacker winning the race entirely before this call).
        // E07-S05: a real Linux reproduction found a zero-delay swap can reuse the freed inode
        // and land within the same ~1ms mtime clock tick, defeating even the nanosecond-aware
        // comparison this test wants to exercise - a brief sleep reflects a real race's actual
        // timing (a separate attacker process needs at least a scheduling/syscall round trip),
        // not a weakening of the swap this test constructs.
        std::fs::remove_file(&file).expect("remove original");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file, b"replacement").expect("create replacement");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect_err("a target swapped before open must be rejected, not deleted");
        assert!(err.0.contains("open-time check"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the replacement must survive - it was never the intended target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_delete_detects_a_target_swapped_between_open_and_unlink() {
        // The exact race E03 verifier review round 1 exploited: the target is confirmed
        // open and matching, then swapped - simulating an observer whose "still matches"
        // answer stops being true at the worst possible moment, which no amount of
        // "revalidate right before mutating" alone can defeat (only proof of what was
        // actually deleted can).
        let dir = TempDir::new("swapped-mid-flight");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(
            result.is_err(),
            "a mid-flight swap must never be reported as a successful, correct deletion"
        );
        assert!(
            file.exists(),
            "the replacement must survive - only the confirmed original may ever be deleted"
        );
        std::fs::read_to_string(&file)
            .map(|contents| assert_eq!(contents, "replacement"))
            .expect("replacement content must be intact");
    }

    #[test]
    fn synthetic_executor_succeeds_by_default_for_unconfigured_paths() {
        let executor = SyntheticMutationExecutor::new();
        let expected = IdentityToken::Unix {
            device: 0,
            inode: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_nanos: 0,
        };
        assert_eq!(
            executor.mutate(
                Path::new("/never/configured"),
                &expected,
                MutationOperation::DeleteFile
            ),
            Ok(())
        );
    }

    #[test]
    fn synthetic_executor_injects_exactly_the_configured_fault() {
        let mut executor = SyntheticMutationExecutor::new();
        executor.set(
            "/synthetic/disk-full",
            Err(MutationError("No space left on device".into())),
        );
        let expected = IdentityToken::Unix {
            device: 0,
            inode: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_nanos: 0,
        };
        assert_eq!(
            executor.mutate(
                Path::new("/synthetic/disk-full"),
                &expected,
                MutationOperation::DeleteFile
            ),
            Err(MutationError("No space left on device".into()))
        );
        assert_eq!(
            executor.mutate(
                Path::new("/synthetic/unrelated"),
                &expected,
                MutationOperation::DeleteFile
            ),
            Ok(())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_intermediate_component_refuses_the_delete() {
        // E21-S07's containment property, stated as a test rather than as a comment: the unlink
        // is issued relative to a directory descriptor walked component-by-component with
        // `O_NOFOLLOW`, so a path that reaches the target through a link is refused outright
        // rather than followed. Same rule E07-S09 established for provider-root establishment,
        // now holding at the moment of mutation too.
        let dir = TempDir::new("symlinked-intermediate");
        let real = dir.0.join("real");
        std::fs::create_dir_all(&real).expect("create real dir");
        let file = real.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);

        let link = dir.0.join("via-link");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");
        let through_link = link.join("target.txt");

        let err = SystemMutationExecutor
            .mutate(&through_link, &expected, MutationOperation::DeleteFile)
            .expect_err("a target reached through a symlinked component must be refused");
        assert!(
            err.0.contains("without following a link"),
            "reason was: {}",
            err.0
        );
        assert!(file.exists(), "the target must survive a refused deletion");
    }

    #[cfg(unix)]
    #[test]
    fn the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_inode() {
        // The handle-relative identity check, exercised directly: the directory descriptor is
        // sound, but the *entry* now points somewhere else. `SealError::IdentityMismatch` is the
        // refusal, and the replacement must survive.
        let dir = TempDir::new("entry-swapped");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal");
            std::thread::sleep(std::time::Duration::from_millis(10));
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(result.is_err(), "a swapped entry must never be deleted");
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement must be intact"),
            "replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_quarantines_a_real_file_confirmed_by_identity() {
        let source_dir = TempDir::new("quarantine-source");
        let dest_dir = TempDir::new("quarantine-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("quarantined-artifact.txt");

        let executor = SystemMutationExecutor;
        executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: br#"{"original_path":"artifact.txt"}"#.to_vec(),
                },
            )
            .expect("quarantine move should succeed");

        assert!(!file.exists(), "the source must no longer exist");
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "hello"
        );
        let record_path = dest_dir.path("quarantined-artifact.txt.quarantine-record.json");
        assert_eq!(
            std::fs::read_to_string(&record_path).expect("restore record must exist"),
            r#"{"original_path":"artifact.txt"}"#
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_quarantine_move_rejects_a_target_already_swapped_before_open() {
        let source_dir = TempDir::new("quarantine-swapped-before-open");
        let dest_dir = TempDir::new("quarantine-swapped-before-open-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file); // captured identity of the ORIGINAL
        let destination = dest_dir.path("quarantined.txt");

        std::fs::remove_file(&file).expect("remove original");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file, b"replacement").expect("create replacement");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("a target swapped before open must be rejected, not moved");
        assert!(err.0.contains("open-time check"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the replacement must survive - it was never the intended target"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_quarantine_move_detects_a_target_swapped_between_open_and_move() {
        // The move-side analogue of `confirmed_delete_detects_a_target_swapped_between_open_
        // and_unlink`: a concurrent swap between the open-time check and the actual rename
        // must never be reported as a successful, correct quarantine of the original.
        let source_dir = TempDir::new("quarantine-swapped-mid-flight");
        let dest_dir = TempDir::new("quarantine-swapped-mid-flight-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);
        let destination = dest_dir.path("quarantined.txt");

        let result = confirmed_quarantine_move_inner(&file, &expected, &destination, b"{}", || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(
            result.is_err(),
            "a mid-flight swap must never be reported as a successful quarantine"
        );
        assert!(
            file.exists(),
            "the replacement must survive - only the confirmed original may ever be quarantined"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement content must be intact"),
            "replacement"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_quarantine_move_refuses_to_clobber_an_existing_destination() {
        let source_dir = TempDir::new("quarantine-destination-exists");
        let dest_dir = TempDir::new("quarantine-destination-exists-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("quarantined.txt");
        std::fs::write(&destination, b"already here").expect("pre-populate destination");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("an existing destination must never be silently replaced");
        assert!(err.0.contains("move failed"), "reason was: {}", err.0);
        assert!(file.exists(), "the source must survive a refused move");
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "already here",
            "the pre-existing destination object must be untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_intermediate_component_refuses_the_quarantine_move() {
        // The move-side analogue of `a_symlinked_intermediate_component_refuses_the_delete`:
        // the source is reached through a symlinked intermediate directory component and must
        // be refused outright, not moved through the link.
        let dir = TempDir::new("quarantine-symlinked-intermediate");
        let dest_dir = TempDir::new("quarantine-symlinked-intermediate-dest");
        let real = dir.0.join("real");
        std::fs::create_dir_all(&real).expect("create real dir");
        let file = real.join("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);

        let link = dir.0.join("via-link");
        std::os::unix::fs::symlink(&real, &link).expect("create symlink");
        let through_link = link.join("target.txt");
        let destination = dest_dir.path("quarantined.txt");

        let err = SystemMutationExecutor
            .mutate(
                &through_link,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("a target reached through a symlinked component must be refused");
        assert!(
            err.0.contains("without following a link"),
            "reason was: {}",
            err.0
        );
        assert!(
            file.exists(),
            "the target must survive a refused quarantine move"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_intermediate_destination_component_refuses_the_quarantine_move() {
        // The destination-side analogue of the source-side test above: the *quarantine store*
        // root is reached through a symlinked intermediate component and must be refused
        // outright too, not moved into the link's target - `SealedRoot::bind_existing` walks
        // the destination parent exactly as handle-relatively as the source's.
        let source_dir = TempDir::new("quarantine-symlinked-dest-intermediate-source");
        let dest_base = TempDir::new("quarantine-symlinked-dest-intermediate-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);

        let real_dest = dest_base.0.join("real");
        std::fs::create_dir_all(&real_dest).expect("create real destination dir");
        let link = dest_base.0.join("via-link");
        std::os::unix::fs::symlink(&real_dest, &link).expect("create symlink");
        let destination = link.join("quarantined.txt");

        let err = SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("a destination reached through a symlinked component must be refused");
        assert!(
            err.0.contains("without following a link"),
            "reason was: {}",
            err.0
        );
        assert!(
            file.exists(),
            "the source must survive a refused quarantine move"
        );
        assert!(
            !real_dest.join("quarantined.txt").exists(),
            "nothing must land at the link's real target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_quarantine_sidecar_that_cannot_be_durably_recorded_never_moves_anything() {
        // E12-S01 round-1 independent verifier review found that a version of this seam which
        // wrote its sidecar *after* the move left `source_exists=false; moved_exists=true;
        // record_exists=false` on a write failure - a moved-and-unrecorded artifact, exactly
        // the unsafe state SI-020 exists to rule out. Round 2 found round 1's own rollback-
        // after-move repair still could not survive a real crash (rather than a synchronous
        // write failure) in that same after-move window. The round-3 repair removes the window
        // entirely: the sidecar's real content is written durably to a `.pending` name *before*
        // the move is ever attempted, so a failure here means the move was never attempted at
        // all - nothing to roll back, nothing moved, nothing stranded.
        let source_dir = TempDir::new("quarantine-record-write-fails-source");
        let dest_dir = TempDir::new("quarantine-record-write-fails-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("quarantined.txt");

        // Pre-plant a directory at the exact name the durable pending sidecar's own atomic
        // write would use for its temp file, forcing `write_new_child_atomically`'s
        // `O_CREAT | O_EXCL` open to fail with `EEXIST`-against-a-directory - before the move
        // is ever attempted, under the round-3 write-before-move ordering.
        std::fs::create_dir(dest_dir.path("quarantined.txt.quarantine-record.json.pending.tmp"))
            .expect("pre-plant a directory at the pending sidecar's own temp name");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("a sidecar write failure must be reported, not silently swallowed");
        assert!(
            err.0.contains("could not durably record"),
            "reason was: {}",
            err.0
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("the original must be completely untouched"),
            "hello",
            "the move must never be attempted when its sidecar cannot be durably recorded first"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination when the move was never attempted"
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_restores_a_real_file_confirmed_by_identity_and_writes_no_sidecar() {
        let source_dir = TempDir::new("restore-source");
        let dest_dir = TempDir::new("restore-dest");
        let file = source_dir.path("quarantined.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("artifact.txt");

        let executor = SystemMutationExecutor;
        executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Restore {
                    destination: destination.clone(),
                },
            )
            .expect("restore move should succeed");

        assert!(!file.exists(), "the quarantined copy must no longer exist");
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "hello"
        );
        assert!(
            !dest_dir
                .path("artifact.txt.quarantine-record.json")
                .exists(),
            "restore must never write a sidecar into the original provider location"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_restore_move_refuses_when_the_original_destination_was_recreated() {
        // The exact TM-14 reproduction (docs/security/THREAT_MODEL.md): the original
        // destination was recreated after quarantine - restore must refuse, not overwrite it.
        let source_dir = TempDir::new("restore-destination-recreated-source");
        let dest_dir = TempDir::new("restore-destination-recreated-dest");
        let file = source_dir.path("quarantined.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("artifact.txt");
        std::fs::write(&destination, b"recreated by the provider").expect("recreate destination");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Restore {
                    destination: destination.clone(),
                },
            )
            .expect_err("a recreated destination must never be silently overwritten");
        assert!(err.0.contains("move failed"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the quarantined copy must survive a refused restore"
        );
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "recreated by the provider",
            "the recreated destination must be untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_restore_move_rejects_a_target_already_swapped_before_open() {
        let source_dir = TempDir::new("restore-swapped-before-open");
        let dest_dir = TempDir::new("restore-swapped-before-open-dest");
        let file = source_dir.path("quarantined.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);
        let destination = dest_dir.path("artifact.txt");

        std::fs::remove_file(&file).expect("remove original");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file, b"replacement").expect("create replacement");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Restore {
                    destination: destination.clone(),
                },
            )
            .expect_err("a target swapped before open must be rejected, not restored");
        assert!(err.0.contains("open-time check"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the replacement must survive - it was never the intended target"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_restore_move_detects_a_target_swapped_between_open_and_move() {
        let source_dir = TempDir::new("restore-swapped-mid-flight");
        let dest_dir = TempDir::new("restore-swapped-mid-flight-dest");
        let file = source_dir.path("quarantined.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);
        let destination = dest_dir.path("artifact.txt");

        let result = confirmed_restore_move_inner(&file, &expected, &destination, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(
            result.is_err(),
            "a mid-flight swap must never be reported as a successful restore"
        );
        assert!(
            file.exists(),
            "the replacement must survive - only the confirmed original may ever be restored"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement content must be intact"),
            "replacement"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_restore_move_refuses_when_the_destination_is_recreated_between_check_and_move() {
        // E12-S02 round-1 independent verifier review: the destination-absence check and the
        // move used to be two separate syscalls (`fstatat` then `renameat`), so provider state
        // created in that exact window was silently replaced (SI-013). `between_open_and_move`
        // fires at precisely that window - immediately before the real move - so writing the
        // "concurrently recreated provider state" there reproduces the finding's exact
        // interleaving rather than a destination that was already present before the call
        // started (which the open-time check alone would already have caught either way).
        let source_dir = TempDir::new("restore-destination-recreated-mid-flight-source");
        let dest_dir = TempDir::new("restore-destination-recreated-mid-flight-dest");
        let file = source_dir.path("quarantined.txt");
        std::fs::write(&file, b"quarantined artifact").expect("create original");
        let expected = identity_of(&file);
        let destination = dest_dir.path("artifact.txt");

        let result = confirmed_restore_move_inner(&file, &expected, &destination, || {
            std::fs::write(&destination, b"new provider state")
                .expect("simulate concurrent provider state created at the restore destination");
        });

        assert!(
            result.is_err(),
            "a destination recreated between the check and the move must never be silently \
             replaced by the restore"
        );
        assert!(
            file.exists(),
            "the quarantined copy must survive a refused restore"
        );
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "new provider state",
            "the concurrently recreated provider state must be untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn system_executor_archives_a_real_file_and_writes_all_sidecars() {
        let source_dir = TempDir::new("archive-source");
        let dest_dir = TempDir::new("archive-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello world").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived-artifact.txt");

        let executor = SystemMutationExecutor;
        executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: br#"{"format":"cancellai-archive","format_version":1}"#.to_vec(),
                },
            )
            .expect("archive move should succeed");

        assert!(!file.exists(), "the source must no longer exist");
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "hello world"
        );
        assert_eq!(
            std::fs::read_to_string(dest_dir.path("archived-artifact.txt.archive-record.json"))
                .expect("archive record must exist"),
            r#"{"format":"cancellai-archive","format_version":1}"#
        );
        assert_eq!(
            std::fs::read_to_string(dest_dir.path("archived-artifact.txt.archive-length.json"))
                .expect("length sidecar must exist"),
            "11",
            "length sidecar must record the source's real byte length"
        );
        let digest_text =
            std::fs::read_to_string(dest_dir.path("archived-artifact.txt.archive-digest.json"))
                .expect("digest sidecar must exist");
        assert_eq!(
            digest_text.trim().len(),
            64,
            "digest sidecar must be a 64-character hex SHA-256"
        );

        assert_eq!(
            verify_archive_integrity(&destination),
            ArchiveIntegrity::Verified
        );
    }

    #[cfg(unix)]
    #[test]
    fn verify_archive_integrity_detects_an_equal_length_content_corruption() {
        // E12-S03 round-1 independent verifier review's exact reproduction: an archived file
        // is corrupted in place by a same-length content change (not a truncation), which the
        // old length-only check accepted as `Verified`. The content fingerprint must catch it.
        let source_dir = TempDir::new("archive-equal-length-corruption-source");
        let dest_dir = TempDir::new("archive-equal-length-corruption-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"0123456789").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived-artifact.txt");

        let executor = SystemMutationExecutor;
        executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect("archive move should succeed");
        assert_eq!(
            verify_archive_integrity(&destination),
            ArchiveIntegrity::Verified,
            "sanity: the freshly archived, uncorrupted copy must verify"
        );

        // Corrupt every byte in place without changing the length at all.
        std::fs::write(&destination, b"abcdefghij").expect("corrupt the archived copy in place");
        assert_eq!(
            std::fs::metadata(&destination)
                .expect("stat corrupted copy")
                .len(),
            10,
            "sanity: the corruption must preserve the exact original length"
        );

        assert!(
            matches!(
                verify_archive_integrity(&destination),
                ArchiveIntegrity::DigestMismatch { .. }
            ),
            "an equal-length content corruption must never be reported as Verified"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_archive_sidecar_that_cannot_be_durably_recorded_never_moves_anything() {
        // Archive shares E12-S01's round-3 write-before-move repair: a durable-write failure
        // for any of its three sidecars must happen before the move is ever attempted, exactly
        // like quarantine's own repaired failure path above.
        let source_dir = TempDir::new("archive-record-write-fails-source");
        let dest_dir = TempDir::new("archive-record-write-fails-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello world").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived-artifact.txt");

        std::fs::create_dir(dest_dir.path("archived-artifact.txt.archive-record.json.pending.tmp"))
            .expect("pre-plant a directory at the pending record sidecar's own temp name");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: br#"{"format":"cancellai-archive","format_version":1}"#.to_vec(),
                },
            )
            .expect_err("a record sidecar write failure must be reported, not swallowed");
        assert!(
            err.0.contains("could not durably record"),
            "reason was: {}",
            err.0
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("the original must be completely untouched"),
            "hello world",
            "the move must never be attempted when its sidecar cannot be durably recorded first"
        );
        assert!(
            !destination.exists(),
            "nothing must land at the destination when the move was never attempted"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_later_archive_sidecar_failing_to_write_durably_cleans_up_the_earlier_ones_too() {
        // The third of three pending sidecars (`archive-digest`) fails to write durably, after
        // the first two (`archive-record`, `archive-length`) already succeeded - the cleanup
        // path must remove every pending sidecar already written, not only the one that failed,
        // and the move must still never be attempted.
        let source_dir = TempDir::new("archive-later-sidecar-write-fails-source");
        let dest_dir = TempDir::new("archive-later-sidecar-write-fails-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello world").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived-artifact.txt");

        std::fs::create_dir(dest_dir.path("archived-artifact.txt.archive-digest.json.pending.tmp"))
            .expect("pre-plant a directory at the pending digest sidecar's own temp name");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: br#"{"format":"cancellai-archive","format_version":1}"#.to_vec(),
                },
            )
            .expect_err("a later sidecar write failure must be reported, not swallowed");
        assert!(
            err.0.contains("could not durably record"),
            "reason was: {}",
            err.0
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("the original must be completely untouched"),
            "hello world"
        );
        assert!(!destination.exists());
        assert!(
            !dest_dir
                .path("archived-artifact.txt.archive-record.json.pending")
                .exists(),
            "the earlier, already-durably-written pending sidecar must be cleaned up too"
        );
        assert!(
            !dest_dir
                .path("archived-artifact.txt.archive-length.json.pending")
                .exists(),
            "the earlier, already-durably-written pending sidecar must be cleaned up too"
        );
    }

    #[cfg(unix)]
    #[test]
    fn verify_archive_integrity_detects_a_truncated_or_corrupted_archive() {
        // The story's own "corruption" verification-contract item: the archived copy is
        // modified after archiving (here, truncated) - integrity must be refused, not assumed.
        let source_dir = TempDir::new("archive-corruption-source");
        let dest_dir = TempDir::new("archive-corruption-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello world").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived-artifact.txt");

        SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect("archive move should succeed");

        std::fs::write(&destination, b"corrupted").expect("simulate post-archive corruption");

        assert_eq!(
            verify_archive_integrity(&destination),
            ArchiveIntegrity::LengthMismatch {
                recorded: 11,
                actual: 9
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn verify_archive_integrity_fails_closed_when_the_length_sidecar_is_missing() {
        let dir = TempDir::new("archive-no-sidecar");
        let path = dir.path("orphan.txt");
        std::fs::write(&path, b"hello").expect("create file with no sidecar");

        assert!(matches!(
            verify_archive_integrity(&path),
            ArchiveIntegrity::RecordUnreadable(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn verify_archive_integrity_fails_closed_when_the_archived_copy_is_gone() {
        let dir = TempDir::new("archive-copy-gone");
        let path = dir.path("gone.txt");
        // Write only the sidecars - the "archived copy" itself never existed at this path.
        std::fs::write(dir.path("gone.txt.archive-length.json"), b"5").expect("write sidecar");
        std::fs::write(
            dir.path("gone.txt.archive-digest.json"),
            b"0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("write sidecar");

        assert!(matches!(
            verify_archive_integrity(&path),
            ArchiveIntegrity::ArchivedCopyUnreadable(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn verify_archive_integrity_fails_closed_when_the_sidecar_content_is_malformed() {
        let dir = TempDir::new("archive-malformed-sidecar");
        let path = dir.path("artifact.txt");
        std::fs::write(&path, b"hello").expect("create file");
        std::fs::write(
            dir.path("artifact.txt.archive-length.json"),
            b"not-a-number",
        )
        .expect("write malformed sidecar");

        assert!(matches!(
            verify_archive_integrity(&path),
            ArchiveIntegrity::RecordMalformed(_)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_archive_move_refuses_to_clobber_an_existing_destination() {
        let source_dir = TempDir::new("archive-destination-exists-source");
        let dest_dir = TempDir::new("archive-destination-exists-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived.txt");
        std::fs::write(&destination, b"already here").expect("pre-populate destination");

        let err = SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("an existing destination must never be silently replaced");
        assert!(err.0.contains("move failed"), "reason was: {}", err.0);
        assert!(file.exists(), "the source must survive a refused move");
        assert_eq!(
            std::fs::read_to_string(&destination).expect("destination content"),
            "already here"
        );
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_archive_move_rejects_a_target_already_swapped_before_open() {
        let source_dir = TempDir::new("archive-swapped-before-open");
        let dest_dir = TempDir::new("archive-swapped-before-open-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived.txt");

        std::fs::remove_file(&file).expect("remove original");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file, b"replacement").expect("create replacement");

        let err = SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("a target swapped before open must be rejected, not archived");
        assert!(err.0.contains("open-time check"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the replacement must survive - it was never the intended target"
        );
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn confirmed_archive_move_detects_a_target_swapped_between_open_and_move() {
        let source_dir = TempDir::new("archive-swapped-mid-flight");
        let dest_dir = TempDir::new("archive-swapped-mid-flight-dest");
        let file = source_dir.path("artifact.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = identity_of(&file);
        let destination = dest_dir.path("archived.txt");

        let result = confirmed_archive_move_inner(&file, &expected, &destination, b"{}", || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(
            result.is_err(),
            "a mid-flight swap must never be reported as a successful archive"
        );
        assert!(
            file.exists(),
            "the replacement must survive - only the confirmed original may ever be archived"
        );
        assert!(!destination.exists());
    }

    #[cfg(windows)]
    struct WindowsTempDir(std::path::PathBuf);

    #[cfg(windows)]
    impl WindowsTempDir {
        fn new(label: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "cancellai-mutation-windows-test-{label}-{}-{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            Self(dir)
        }

        fn path(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }

    #[cfg(windows)]
    impl Drop for WindowsTempDir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[cfg(windows)]
    fn windows_identity_of(path: &Path) -> IdentityToken {
        let facts =
            cancellai_sealedfs::observe_identity(path).expect("observe path for test identity");
        IdentityToken::Windows {
            volume_serial_number: facts.volume_serial_number,
            file_index: facts.file_index,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_ticks: facts.last_write_time_ticks,
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_deletes_a_real_file_confirmed_by_identity() {
        let dir = WindowsTempDir::new("delete-confirmed");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = windows_identity_of(&file);

        let executor = SystemMutationExecutor;
        executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect("delete should succeed");
        assert!(!file.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_refuses_a_quarantine_move_as_a_disclosed_residual() {
        // E12-S01 is Unix-only for the confirmed move technique; Windows must refuse
        // explicitly rather than silently falling back to an unconfirmed path-based rename.
        // Pinned as a real Windows-CI assertion (not merely implied by the code), matching
        // this crate's own precedent for every other platform-scoped residual.
        let dir = WindowsTempDir::new("quarantine-not-implemented");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = windows_identity_of(&file);
        let destination = dir.path("quarantined.txt");

        let err = SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Quarantine {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("quarantine on Windows must be refused, not silently attempted");
        assert!(
            err.0.contains("not implemented on this platform"),
            "reason was: {}",
            err.0
        );
        assert!(file.exists(), "the source must survive the refusal");
        assert!(!destination.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_refuses_a_restore_move_as_a_disclosed_residual() {
        let dir = WindowsTempDir::new("restore-not-implemented");
        let file = dir.path("quarantined.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = windows_identity_of(&file);
        let destination = dir.path("artifact.txt");

        let err = SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Restore {
                    destination: destination.clone(),
                },
            )
            .expect_err("restore on Windows must be refused, not silently attempted");
        assert!(
            err.0.contains("not implemented on this platform"),
            "reason was: {}",
            err.0
        );
        assert!(file.exists(), "the source must survive the refusal");
        assert!(!destination.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_refuses_an_archive_move_as_a_disclosed_residual() {
        let dir = WindowsTempDir::new("archive-not-implemented");
        let file = dir.path("artifact.txt");
        std::fs::write(&file, b"hello").expect("create file");
        let expected = windows_identity_of(&file);
        let destination = dir.path("archived.txt");

        let err = SystemMutationExecutor
            .mutate(
                &file,
                &expected,
                MutationOperation::Archive {
                    destination: destination.clone(),
                    record: b"{}".to_vec(),
                },
            )
            .expect_err("archive on Windows must be refused, not silently attempted");
        assert!(
            err.0.contains("not implemented on this platform"),
            "reason was: {}",
            err.0
        );
        assert!(file.exists(), "the source must survive the refusal");
        assert!(!destination.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_system_executor_reports_the_os_error_for_a_missing_target() {
        let dir = WindowsTempDir::new("missing-target");
        let missing = dir.path("does-not-exist");
        let expected = IdentityToken::Windows {
            volume_serial_number: 0,
            file_index: 0,
            kind: crate::identity::FileKind::File,
            modified: crate::clock::Timestamp(0),
            modified_ticks: 0,
        };
        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&missing, &expected, MutationOperation::DeleteFile)
            .expect_err("deleting a missing file must fail, not silently succeed");
        assert!(!err.0.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn windows_confirmed_delete_rejects_a_target_already_swapped_before_open() {
        let dir = WindowsTempDir::new("swapped-before-open");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = windows_identity_of(&file); // captured identity of the ORIGINAL

        std::fs::remove_file(&file).expect("remove original");
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&file, b"replacement").expect("create replacement");

        let executor = SystemMutationExecutor;
        let err = executor
            .mutate(&file, &expected, MutationOperation::DeleteFile)
            .expect_err("a target swapped before open must be rejected, not deleted");
        assert!(err.0.contains("open-time check"), "reason was: {}", err.0);
        assert!(
            file.exists(),
            "the replacement must survive - it was never the intended target"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_confirmed_delete_detects_a_target_swapped_between_open_and_unlink() {
        let dir = WindowsTempDir::new("swapped-mid-flight");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = windows_identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal of the original");
            std::thread::sleep(std::time::Duration::from_millis(10));
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(
            result.is_err(),
            "a mid-flight swap must never be reported as a successful, correct deletion"
        );
        assert!(
            file.exists(),
            "the replacement must survive - only the confirmed original may ever be deleted"
        );
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement content must be intact"),
            "replacement"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_the_unlink_refuses_a_name_that_no_longer_holds_the_confirmed_identity() {
        let dir = WindowsTempDir::new("entry-swapped");
        let file = dir.path("target.txt");
        std::fs::write(&file, b"original").expect("create original");
        let expected = windows_identity_of(&file);

        let result = confirmed_delete_file_inner(&file, &expected, || {
            std::fs::remove_file(&file).expect("simulate concurrent removal");
            std::thread::sleep(std::time::Duration::from_millis(10));
            std::fs::write(&file, b"replacement").expect("simulate concurrent replacement");
        });

        assert!(result.is_err(), "a swapped entry must never be deleted");
        assert_eq!(
            std::fs::read_to_string(&file).expect("replacement must be intact"),
            "replacement"
        );
    }
}
