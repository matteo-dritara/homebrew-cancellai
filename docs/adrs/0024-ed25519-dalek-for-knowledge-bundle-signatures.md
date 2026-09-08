# ADR-0024: `ed25519-dalek` (verification-only) for signed knowledge bundles

- Status: Accepted
- Date: 2026-09-09
- Owners: project owner
- Related: ADR-0015, ADR-0017, ADR-0019, E16-S02, SI-021, SI-022, SI-029

## Context

E16-S02's outcome is "package provider/version/layout intelligence separately from the binary
with signature/provenance verification." Its acceptance criteria require that unsigned/invalid
bundles are ignored with diagnostics, that a knowledge update cannot raise authority beyond
local trust policy, and that rollback to a prior trusted bundle is supported.
`docs/security/SUPPLY_CHAIN.md`'s "Knowledge updates" section already commits to the shape this
implements: a bundle "has schema version, publisher identity, issue/expiry metadata, and
content digest," "is verified before use," and "cannot elevate above local trust/authority
ceilings."

Verifying a cryptographic signature is not something `std` can do at all - there is no
signature-verification primitive in the standard library for any algorithm, unlike ADR-0017's
`openat`/`renameat` (which exist in `std` conceptually but not with the handle-relative
guarantee needed). This is squarely the "a dependency requires a dedicated, reviewed ADR naming
the specific capability `std` cannot express" bar `AGENTS.md` sets for the kernel ring, and
`cancellai-safety` - the crate that already owns `TrustedTier`/`TrustPromotionEvidence` (SI-021)
- is where a bundle's publisher identity gets checked against local trust policy, so it is the
right crate to also own the primitive that makes "this publisher really signed this" a checked
fact rather than an assertion.

## Decision

We add `ed25519-dalek` v3.0.0 to `cancellai-safety`, **verification-only**:

- `default-features = false, features = ["fast", "zeroize"]` - no `alloc`, `batch`, `rand_core`,
  `pkcs8`, `pem`, or `serde` feature. `cancellai-safety` never generates or holds a signing key;
  it only calls `VerifyingKey::verify_strict` against a publisher public key supplied by local
  trust policy (`TrustedPublisher`, a caller-provided table - see `knowledge_bundle.rs`'s own
  docs) and a signature carried in the bundle. `fast` enables precomputed tables for that
  verification path (a real hot-path improvement, not surface bloat: it is a `curve25519-dalek`
  compile-time table selection, not a new capability). `zeroize` is the crate's own default and
  zeroes key material rather than leaving it in memory after use - a correctness property this
  crate wants for exactly the reason ADR-0017 isolates `unsafe`: signature/key handling is a
  place where "the safe default already does the careful thing" is worth keeping even at some
  binary-size cost.
- License is BSD-3-Clause, already in `rust/deny.toml`'s allow-list (`rust/deny.toml`'s comment
  block above the license array lists it explicitly). `rust-version = "1.85"` matches this
  workspace's pinned MSRV exactly - no MSRV bump (ADR-0015's "bumped only by deliberate,
  reviewed decision" is not triggered).
- `cargo deny check` passes with this dependency added: `advisories ok, bans ok, licenses ok,
  sources ok`. It adds one new duplicate-version instance (`syn` 2.0.119 already existed for
  `clap_derive`/`serde_derive`; `curve25519-dalek-derive` pulls a second, `syn` 3.0.4) -
  `rust/deny.toml`'s `bans.multiple-versions = "warn"` already treats this as non-blocking
  policy, not a new exception carved out for this change.
- No signing capability is added anywhere in this workspace. Producing a signed bundle (the
  publisher side) is explicitly out of this ADR's and E16-S02's scope - `cancellai-safety`
  verifies bundles a publisher produced by some means outside this repository (today: hand-built
  fixtures in this crate's own tests, using `ed25519-dalek`'s signing API compiled only under
  `[dev-dependencies]`/`#[cfg(test)]`, never in the shipped binary).
- This is the same pattern ADR-0017 established: a real cryptographic/FFI capability `std`
  cannot express, added narrowly (verification only, minimal feature set), in the one crate
  whose job is to hold exactly this kind of authority-adjacent decision.

## Alternatives considered

### `ring`

A widely used, audited crate covering signatures, hashing, and AEAD. Rejected for this specific
need: `ring` links a C/assembly BoringSSL-derived core for parts of its implementation, which
sits less comfortably next to `unsafe_code = "forbid"` being the workspace default everywhere
outside `cancellai-sealedfs` (ADR-0015) - `cancellai-safety` itself stays `forbid`, and a
dependency whose own internals are not pure Rust is a different risk shape than one that is,
even though the dependency's own `unsafe` is not this crate's `unsafe`. `ed25519-dalek` (via
`curve25519-dalek`) is pure Rust, matching this workspace's existing posture more closely for no
loss of function this story needs (only Ed25519 verification, not the broader TLS/AEAD surface
`ring` also carries).

### `RustCrypto`'s `ed25519`/`signature` crates directly, hand-assembling verification

`ed25519-dalek` already depends on both (`ed25519`, `signature`) and is the reference
implementation most of the Rust ecosystem uses for Ed25519 specifically (Solana, libp2p, and
others per its own repository's adoption list) - reimplementing verification on top of the
lower-level traits ourselves would mean re-deriving the same curve arithmetic this crate already
provides, with no independent audit trail of our own version. Rejected: more code to review for
no capability gained.

### No signature scheme at all - trust by content digest/HTTPS transport only

A content digest alone (already planned for the bundle format) proves integrity against
accidental corruption but not authenticity against a malicious publisher_id claim - anyone could
publish a bundle claiming to be a trusted publisher's identity with a matching digest of their
own tampered content. Rejected: this is exactly the "unsigned bundle" case AC1 requires be
"ignored with diagnostics," so a digest-only design would fail this story's own acceptance
criteria, not merely be a weaker version of it.

## Consequences

### Positive

- `KnowledgeBundle` verification (`cancellai-safety::knowledge_bundle`) can prove a bundle came
  from a specific, locally-trusted publisher key and was not altered in transit, using an
  audited, widely-adopted primitive rather than a hand-rolled check.
- No signing capability ships in the binary at all - there is no code path by which a compromise
  of the shipped `cancellai-cli`/`cancellai-guardian` binary could ever produce a validly signed
  bundle, because the capability to sign was never linked in.

### Negative / accepted risk

- One more crate (plus its `curve25519-dalek`/`sha2`/`digest`/`subtle`/`zeroize` transitive
  tree, 19 packages total per `cargo add`'s own report) enters the kernel-ring dependency
  surface `docs/security/SUPPLY_CHAIN.md`'s Dependabot coverage and CodeQL analysis already
  apply to. Accepted: this is the cost E16-S02's own acceptance criteria require paying, and the
  feature set chosen is the minimum this story's verification-only need actually uses.
- `curve25519-dalek-derive`'s `syn` 3.0.4 duplicate is visible in `cargo deny check`'s warning
  output going forward; it was already a non-blocking warning class (`multiple-versions =
  "warn"`) before this change, not newly introduced tolerance.
