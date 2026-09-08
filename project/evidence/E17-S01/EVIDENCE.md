# Evidence Packet - E17-S01

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E17.json` (E17-S01), as of this change

## Outcome

PASS

E17-S01 defines the canonical release artifact manifest as a machine-verifiable, versioned
document: `project/schemas/release_manifest.schema.json` is the contract, and
`scripts/release_manifest.py` is the stdlib-only hand-written validator (matching every other
governance checker in this repository - no `jsonschema` dependency). It records `version`,
`channel`, `source_sha`, `build_identity` (repository/workflow/run_id),
`knowledge_compatibility` (provider-knowledge min/max schema version), and an `artifacts` list
where every distributed binary is named exactly once (`name`/`target_triple`/`sha256`).

Generating a manifest from a real cross-platform build (and attaching SBOM/provenance/signing
evidence to it) is out of scope - that is E17-S02/E17-S03, both of which depend on this story.
This story only fixes the contract, its validator, and the checksum round-trip machinery those
later stories will call.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Manifest is machine-verifiable and versioned | `project/schemas/release_manifest.schema.json` (`schema_version: {"const": 1}`, `document_type: {"const": "release_manifest"}`); `scripts/release_manifest.py::validate_document` hand-validates every field; `scripts/release_manifest.py::check` validates the golden fixture under `tests/fixtures/release_manifest/golden/manifest.golden.json` and runs in `pre-commit` (`release-manifest-check` hook) and CI (`.github/workflows/release.yml`'s `verify` job); `tests/test_release_manifest.py::ReleaseManifestSchemaTests` (18 tests) proves both the valid corpus and every rejected shape (missing/wrong `schema_version`, wrong `document_type`, unknown key, non-SemVer `version`, unrecognized `channel`, malformed `source_sha`, missing `build_identity` field, inverted `knowledge_compatibility` range, non-integer knowledge version) | PASS |
| AC2 - Every distributed binary is represented exactly once | `scripts/release_manifest.py::_check_artifacts` tracks artifact `name` occurrences and flags any count > 1 with a message citing AC2 by name; `test_checker_flags_a_duplicate_artifact_name` proves the rejection, `test_checker_allows_two_artifacts_sharing_a_target_triple` proves the constraint is on `name` specifically (an archive and an installer for the same platform are legitimate), `test_checker_flags_an_empty_artifacts_list` proves an empty manifest is rejected outright | PASS |

## Safety Evidence

Not applicable - `safety_obligations: []` for this story, and CR2 does not require adversarial
Safety Invariant counterexamples. The manifest document itself carries no mutation authority;
it is inert metadata consumed by later (E17-S02/S03/S05) release tooling.

## Verification Commands

```text
python3 -m pytest tests/test_release_manifest.py -v
python3 -m mypy --strict scripts/release_manifest.py
python3 -m ruff check scripts/release_manifest.py tests/test_release_manifest.py
python3 -m ruff format --check scripts/release_manifest.py tests/test_release_manifest.py
python3 scripts/release_manifest.py check
python3 scripts/check_workflows.py check
python3 scripts/project_os.py check
```

All commands PASS locally (30/30 tests in `tests/test_release_manifest.py`; full suite and
governance checks re-verified below under "Full local gate run").

## Compatibility

- Platforms/providers/schemas exercised: the golden fixture's `artifacts` list covers the four
  tier-1 target triples named in `docs/PLATFORMS.md`
  (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc`) as example data - the checker validates structure/uniqueness, not
  that any specific triple exists yet as a real build output (that assertion belongs to
  E17-S02, which will generate a manifest from an actual build matrix).

## Performance / operability

- `scripts/release_manifest.py check` reads one small JSON schema file and a one-document
  golden corpus; no network I/O, no external dependency. Round-trip checksum helpers
  (`sha256_of`, `checksum_matches`, `verify_artifact_checksums`) are pure functions over
  in-memory bytes, exercised directly by `ChecksumRoundTripTests`.

## Documentation updated

- `docs/security/SUPPLY_CHAIN.md` - "Canonical release evidence" section now names the schema
  path, validator command, and the artifact-identity rule (unique `name`, shareable
  `target_triple`).
- `docs/RELEASING.md` - "Target Rust release factory" section links the manifest contract and
  states that producing one from a real build is E17-S02.
- `CHANGELOG.md` - `[Unreleased]` entry recording the new schema/validator.

## Residual risks

- The manifest schema does not yet enforce that `artifacts` covers every tier-1 platform in
  `docs/PLATFORMS.md`, or cross-check `source_sha` against real git history, or verify a
  checksum against a real downloaded artifact - deliberately: those all require a real
  multi-target build to exist, which is E17-S02. `check()` validates structure and internal
  consistency only, consistent with how `scripts/check_schemas.py` validates the
  inventory/plan/explanation/result golden corpus.
- `build_manifest()`/`verify_artifact_checksums()` are not yet called from `scripts/release.py`
  - the current Python/Homebrew release path is unchanged by this story. Wiring manifest
  generation into `release.py prepare`/`finalize` for the still-shipping single-artifact
  release, or leaving that entirely to the Rust release factory, is an open sequencing
  question left for E17-S02 rather than decided implicitly here.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
