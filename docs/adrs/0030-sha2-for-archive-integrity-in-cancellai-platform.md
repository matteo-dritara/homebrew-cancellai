# ADR-0030: `sha2` (digest-only) for pre-purge archive integrity in `cancellai-platform`

- Status: Accepted
- Date: 2026-09-16
- Owners: project owner
- Related: ADR-0017, ADR-0019, ADR-0024, E12-S03, SI-018, SI-019, SI-020

## Context

E12-S03's AC3 requires "restore integrity is checked before source purge." The story's own
evidence packet scoped this narrowly at implementation time: a real cryptographic content hash
needs a kernel-ring dependency this workspace did not carry in `cancellai-platform`, and
ADR-0019 requires a dedicated, reviewed ADR for that - the same bar `libc`/`cancellai-sealedfs`
cleared under ADR-0017 - not spent for archive integrity. The story shipped a byte-length check
instead, disclosed as weaker than a cryptographic hash rather than overclaimed.

Round 1 of this epic's independent verifier review reproduced the gap that leaves open: an
archived file rewritten to different bytes of the *same* length verified as `Verified`, because
length alone is a truncation/extension signal only. The round 1 repair added an FNV-1a-64
content fingerprint, explicitly disclosed as a fast, non-cryptographic fingerprint rather than a
collision-resistant digest, to close the concrete accidental/incidental-corruption finding
without spending the ADR-0019 budget mid-round.

Round 2 independent verifier review judged that residual insufficient for what AC3 actually
authorizes: this predicate is meant to gate a future *irreversible source purge* (E12-S04, once
unblocked) - the one thing this repository's Safety Invariants treat as needing the strongest
available check, not the cheapest available one. A non-collision-resistant fingerprint leaves a
deliberate-collision gap in exactly the predicate that would authorize destroying the only
known-good copy. The verifier's required repair named the decision this ADR now makes: "a
reviewed ADR-0019 kernel-ring digest decision followed by a versioned collision-resistant
digest."

`sha2` is not a new capability entering this workspace's dependency graph. `cancellai-safety` -
itself a kernel-ring crate under ADR-0019's own list - already depends on `sha2 = { version =
"0.11.0", default-features = false }` for `KnowledgeBundle` content-digest verification
(`knowledge_bundle.rs`, predating this ADR). `ed25519-dalek` (ADR-0024) pulls in a `sha2`-family
digest transitively as well. What is missing is not the dependency itself but a decision that
extends its use to a second kernel-ring crate for a second purpose - ADR-0019 draws the ring
line at "no dependency except by a dedicated, reviewed ADR naming the specific capability `std`
cannot express," with no standing exception for a crate already vetted elsewhere in the ring,
and this repository's own `rust-kernel-guard` skill enforces exactly that reading (E12-S03's
evidence packet records its verdict on reusing `sha2` here as `REQUIRES ADR` before this one
existed).

## Decision

Add `sha2 = { version = "0.11.0", default-features = false }` to `cancellai-platform`'s
`Cargo.toml` - identical version and feature configuration to `cancellai-safety`'s existing
dependency, so `Cargo.lock` resolves one shared `sha2` in the tree rather than two.

`cancellai_platform::mutation::confirmed_archive_move_inner` computes a SHA-256 digest of the
source artifact's content, streamed through the same already-open, already-identity-confirmed
file descriptor its open-time checks already use (replacing round 1's FNV-1a
`fingerprint_content` outright, not running both - a real digest supersedes the fingerprint it
was standing in for; keeping both would be two content passes for one purpose, not defence in
depth). The digest is recorded as a fourth archive sidecar (`<destination>.archive-digest.json`,
hex-encoded, alongside the pre-existing `archive-record`/`archive-length` sidecars).
`verify_archive_integrity` requires the recorded length *and* the recorded SHA-256 digest to
both match before returning `Verified` - the length check stays, since it is a strictly cheaper
first-pass rejection for the common truncation case and costs nothing extra once the file is
already being read for the digest.

Scope stays exactly what round 2 asked for and no wider:

- **Digest-only.** No signing, no key material, no new capability beyond "compute SHA-256 of a
  byte stream" - the same narrow-verification-only posture ADR-0024 already established for
  `ed25519-dalek` in this workspace.
- **`default-features = false`**, matching `cancellai-safety`'s exact configuration. Neither
  crate needs `sha2`'s `asm`/`oid`/`compress` features; both use only the portable `Digest`
  trait (`Sha256::new()` / `.update()` / `.finalize()`), so there is nothing to diverge on.
- **No new license or advisory surface.** `sha2` is already in `Cargo.lock` and already covered
  by `rust/deny.toml`'s allow-list and `cargo deny check`'s advisory scan; adding a second
  dependent does not add a second dependency.
- Real byte compression remains out of scope, exactly as E12-S03's evidence packet already
  recorded - this ADR answers only the integrity-digest half of that story's deferred work, not
  the compression half, which still needs its own ADR if a future story picks it up.

## Alternatives considered

### Keep FNV-1a and layer a second, coarser digest to avoid the ADR

Considered and rejected: it does not answer the actual finding. Round 2's objection was not "one
fingerprint is not enough," it was "the fingerprint your predicate is authorizing a purge on
must be collision-resistant, and this one states plainly that it is not." No amount of layering
non-cryptographic checks produces a cryptographic guarantee; the only fix that answers the
finding is the digest this ADR adds.

### A different digest family (`blake3`, `sha3`)

`sha2`/SHA-256 was chosen because it is already resolved in this exact workspace, in the kernel
ring, at the exact version and feature configuration needed - the smallest possible addition to
the dependency surface for the guarantee required. `blake3` is faster but would be a wholly new
dependency with no existing footprint here; `sha3` has no existing footprint either. Neither
buys this story anything SHA-256 does not already provide for the size of file this codebase
archives, and both would cost strictly more review surface for the same outcome.

### Defer indefinitely, ship E12-S03 with the disclosed FNV-1a residual

This is what round 1 tried and round 2 rejected. A CR4 predicate that will eventually gate an
irreversible purge is exactly the place this repository's own stated priorities (`AGENTS.md`:
"quarantine is preferred to purge... mutation goes through one safety boundary") say a disclosed-
but-real weakness should not be accepted merely because closing it costs a governance decision.

## Consequences

### Positive

- `verify_archive_integrity`'s `Verified` result now means what AC3 needs it to mean before it
  can ever gate a real purge: length and content both independently confirmed unchanged, via a
  digest with no known practical collision construction.
- No second crate enters the kernel-ring surface `docs/security/SUPPLY_CHAIN.md`'s Dependabot
  coverage and CodeQL analysis already apply to - `sha2` was already there.

### Negative / accepted risk

- `cancellai-platform` gains its first non-workspace, non-`cancellai-sealedfs` dependency
  (`serde` was already present for a different reason). This is the cost ADR-0019 exists to make
  visible rather than free, and it is spent here on exactly the capability `std` cannot provide
  (there is no cryptographic hash in `std`).
- Computing a digest still requires reading the entire archived artifact's content once at
  archive time (already true of round 1's FNV-1a fingerprint; unavoidable for any real content
  digest) - streamed in bounded chunks through the already-open descriptor, not loaded whole
  into memory, but proportional to file size unlike every other operation this seam performs.

## Safety and compatibility impact

- Change Risk: CR4 (this touches `cancellai-platform::mutation`, the sole SI-019 mutation-
  boundary seam, and directly changes E12-S03's AC3/SI-020 evidence).
- Safety Invariants affected: SI-018 (unaffected - no change to the same-device boundary check),
  SI-019 (unaffected - still routes through the one mutation seam), SI-020 (strengthened - the
  pre-purge integrity predicate this ADR fixes is exactly the SI-020 "irreversible actions are
  explicit and stronger-gated" surface once E12-S04 exists).
- Migration/rollback: additive sidecar (`archive-digest.json`) alongside the pre-existing
  `archive-record`/`archive-length` sidecars; no existing sidecar format changes. An archive
  created before this change carries no digest sidecar and is treated by `verify_archive_
  integrity` exactly as a missing-sidecar case already was (`RecordUnreadable`, fail closed) -
  no silent "trust it anyway" fallback for pre-existing archives.
