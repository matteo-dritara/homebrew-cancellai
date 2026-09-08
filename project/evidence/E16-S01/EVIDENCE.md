# Evidence Packet - E16-S01

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E16.json` (E16-S01), as of this change

## Outcome

PASS

`rust/crates/cancellai-provider-api/src/manifest.rs` defines the v1 provider manifest schema
(`ProviderManifest`/`ManifestRoot`/`Marker`/`ArtifactPattern`/`ArtifactCategory`) and its sole
entry point, `parse_manifest`. `manifest_provider.rs` turns a parsed manifest plus resolved root
paths into real filesystem observations: `resolve_root` (env-var-or-default resolution, mirroring
`cancellai-cli::roots`'s own env-override/default-home shape but independent of it, since
`cancellai-provider-api` sits below `cancellai-cli` in the dependency graph), `fingerprint_manifest_root`
(reusing this crate's own `derive_root_confidence` - the identical rule Claude/Codex's adapters
use), and `ManifestProvider`, a `ProviderCapabilities` implementation driven entirely by data.

This crate's own module doc already flagged the gap this story closes: "The manifest model...
does not exist yet and is deferred to a later E05 story" (now closed by E16, the epic that
actually needed it).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Manifest-only integrations cannot acquire destructive capability implicitly | Enforced by *shape*, not a runtime check: `ProviderManifest`/`ManifestRoot`/`Marker`/`ArtifactPattern` have no field anywhere that can express a capability claim, a trust level, or an authority ceiling - `manifest::tests::ac1_a_manifest_smuggling_a_trust_claim_is_rejected_not_silently_ignored`, `...ac1_a_manifest_smuggling_a_native_delete_claim_is_rejected`, and `...ac1_an_artifact_pattern_smuggling_an_authority_field_is_rejected` prove a malicious manifest attempting to add such a field is rejected outright (`deny_unknown_fields`), not silently stripped. `ManifestProvider` computes authority through the same `cancellai_safety::TrustedTier` gate every hand-written adapter uses (documented in the module doc; no code in this story bypasses it - this story adds no new authority-computation path at all, only a new *input* to the existing one) and defaults `TrustedTier::untrusted()`; `manifest_provider::tests::ac1_an_unrecognized_layout_reports_unsupported_and_stays_inspection_only` proves an unrecognized layout stays `Unsupported` end to end | PASS |
| AC2 - Schema is versioned and rejects unknown security-relevant fields | `schema_version`/`CURRENT_SCHEMA_VERSION` (`manifest::tests::ac2_a_missing_schema_version_is_rejected`, `...ac2_an_unsupported_schema_version_is_rejected`); `#[serde(deny_unknown_fields)]` on every struct rejects *any* unrecognized field, not a hand-picked "security-relevant" subset - deliberately broader than the AC's literal wording, since a checker that has to first classify a field as security-relevant before rejecting it is a checker an attacker only has to find one field it forgot to classify | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - `safety_obligations: [SI-021]`, CR2. SI-021 ("Provider
manifest trust bounds authority") is satisfied structurally: this story adds no new authority
computation, only a new input (`ManifestProvider`) that flows through the pre-existing,
already-verified `TrustedTier`/`effective_authority` gate (E05-S02, its own `compile_fail`
doctest already proves external construction of an elevated `TrustedTier` is impossible). The
"malicious/overbroad manifest corpus" this story's own verification contract names is the 14
adversarial tests in `manifest.rs` (path traversal in a marker/subdir/artifact glob, duplicate
root names, an artifact referencing an undeclared root, an uppercase/invalid root name, an empty
provider id, an empty env var name, malformed JSON, a null schema version) plus the three AC1
smuggling tests above.

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/check_provider_compatibility.py check
python3 scripts/rust_python_parity.py self-test
python3 scripts/rust_python_parity.py check
python3 scripts/check_platforms.py check
python3 scripts/check_docs.py check
```

All PASS locally. `cargo test -p cancellai-provider-api`: 58/58 (33 new: 16 in `manifest.rs`,
17 in `manifest_provider.rs`). Full workspace `cargo test --workspace`: every crate green,
including `cancellai-provider-claude`/`cancellai-provider-codex`'s full pre-existing suites
unchanged (this story added no field to the shared `root_fingerprint::RootFingerprint` type -
`ManifestRootFingerprint`, a new, separate type, was added instead specifically to avoid
touching either adapter crate; see "Residual risks").

## Compatibility

- No existing public API in `cancellai-provider-api` changed signature; `cancellai-provider-claude`/
  `cancellai-provider-codex`/`cancellai-cli`/`cancellai-policy` all compile and test unchanged.

## Performance / operability

- `find_matching_files` (used by `ManifestProvider`'s `INVENTORY_MAP` capability) is a bounded
  walk (20,000-entry cap, mirroring `root_probe::contains_uuid_named_jsonl`'s own bound) that
  does not descend into symlinked directories - `manifest_provider::tests::
  find_matching_files_does_not_descend_into_a_symlinked_directory` proves this directly.

## Documentation updated

- `docs/architecture/PROVIDER_MODEL.md` - "Manifest-only" section gained an implementation
  paragraph matching the style E05-S02/S03/S04's own sections already use.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- `ManifestRootFingerprint` (in `manifest_provider.rs`) duplicates `root_fingerprint::
  RootFingerprint`'s two other fields (`origin`, `confidence`) because that shared type's
  `markers: Vec<&'static str>` cannot hold runtime-parsed `String`s - a manifest's markers come
  from JSON, not a compile-time const table. Changing the shared type to `Vec<String>` was
  considered and rejected for this story specifically to avoid touching
  `cancellai-provider-claude`/`cancellai-provider-codex`'s existing, already-verified fixture
  tests (several of which compare `fingerprint.markers` against `vec!["literal", ...]` in a way
  that would need updating for an owned-`String` type) - out of proportion for a CR2 schema
  story. A future story unifying the two types can do so deliberately, informed by whichever of
  the two shapes more callers end up needing by then.
- No real manifest exists yet to exercise this schema against a real provider - E16-S05
  (OpenCode adapter) is the first, and depends on this story.
- `ManifestProvider` is not wired into `cancellai-cli`'s `status`/`plan`/`clean` command surface -
  deliberately deferred; see E16-S05's own evidence for why.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
