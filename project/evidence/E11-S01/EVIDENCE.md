# Evidence Packet - E11-S01

- Commit/PR: this working-tree change (executor session, 2026-09-14)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR3 (declared CR2 in the story contract; raised during execution - see "Risk
  reclassification" below)
- Spec version/commit: `project/epics/E11.json` (E11-S01), as of this change

## Risk reclassification: CR2 -> CR3

The story contract declares CR2. Implementing it required adding `serde::Deserialize` to
`cancellai_model::AuthorityLevel` (`rust/crates/cancellai-model/src/vocabulary.rs`) so the schema
could parse a requested authority using this codebase's one existing vocabulary instead of a
second, duplicated enum. `project/risk_floors.json` sets a CR3 floor on
`rust/crates/cancellai-model/src/*` ("The domain model defines what an artifact, an authority
and a plan are. It cannot mutate, but every decision downstream is expressed in its types."),
and the pre-commit risk-floor gate refused the CR2-declared commit on exactly that pattern. The
change is additive (a new derive; no existing field, variant, `Ord`/`Hash`/`Serialize` behavior,
or call site changes) and duplicating the enum instead to stay under the floor would have traded
a real floor violation for the "second, drifting copy" this codebase's own conventions warn
against - so the story's `change_risk` is raised to CR3 rather than overridden. This is the floor
doing exactly what it exists to do, not a misclassification worth an override entry.

## Outcome

PASS

`rust/crates/cancellai-policy/src/schema.rs` defines the v1 declarative policy document schema -
`PolicyDocument` (`schema_version`, `global`, `machine`, `providers`, `projects`,
`artifact_types`, `pins`), each scope a `ScopePolicy { authority, retention, budget }`, plus
`PinEntry` for the session/pin scope - and its sole entry point, `parse_policy`. The type shape
follows `docs/architecture/POLICY_MODEL.md`'s scope hierarchy
(`GLOBAL -> MACHINE -> PROVIDER -> PROJECT -> ARTIFACT_TYPE -> SESSION/PIN`) and reuses the
versioned-document pattern already established by `cancellai-provider-api::manifest` and
`cancellai-safety::knowledge_bundle` (`#[serde(deny_unknown_fields)]` on every struct,
`schema_version` gated against `CURRENT_SCHEMA_VERSION`). `cancellai_model::AuthorityLevel`
gained `Deserialize` (previously `Serialize`-only) so the schema names a requested authority
using the one vocabulary this codebase already has, rather than inventing a second
string-to-enum mapping. Resolving several scopes into one `EffectivePolicy` (constitutional
precedence, conflict resolution) is explicitly out of scope - that is E11-S02's constraint
resolver; this story only parses and structurally validates the raw document.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Unknown policy keys fail validation rather than being ignored | `#[serde(deny_unknown_fields)]` on `PolicyDocument`, `ScopePolicy`, and `PinEntry` rejects *any* unrecognized field outright rather than silently dropping it - proven at every nesting level: `schema::tests::ac1_an_unknown_top_level_key_is_rejected_not_silently_ignored`, `...ac1_an_unknown_key_inside_a_scope_is_rejected`, `...ac1_an_unknown_key_inside_a_pin_is_rejected`. Versioning is the same gate `manifest.rs`/`knowledge_bundle.rs` already use: `...a_missing_schema_version_is_rejected`, `...an_unsupported_schema_version_is_rejected_rather_than_silently_reinterpreted`, `...a_null_schema_version_is_rejected` | PASS |
| AC2 - Policies are data, not executable code | Enforced by shape, not a runtime check: `ScopePolicy`/`PinEntry` have no field anywhere that could hold a shell command, script, or provider-native operation - only a requested `AuthorityLevel` and raw retention/budget text. `schema::tests::ac2_a_document_smuggling_a_shell_command_field_is_rejected` and `...ac2_a_scope_smuggling_a_command_field_is_rejected` prove an attempt to add such a field (`"exec"`, `"command"`) is rejected outright by `deny_unknown_fields`, not silently stripped and ignored | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - this story declares no `safety_obligations`, CR2. SI-025
("Policy cannot override constitutional ceilings") is relevant context but not discharged by
this story: this module computes no authority ceiling and performs no resolution at all - it
only carries a *requested* value forward for E11-S02 to bound. The story's own verification
contract ("Schema/version migration tests") is discharged by the version-gate tests above; there
is exactly one schema version today, so the "migration" tested is the seam
(`UnsupportedSchemaVersion` rejection) a future migration attaches to, not a migration between
two live versions.

Adversarial cases planned before implementation (`adversarial-cases` skill, folded into the test
suite above and below):

- unknown-to-authority promotion: `schema::tests::an_absent_scope_deserializes_as_none_never_a_default_authority`, `...a_scope_present_with_no_authority_set_is_none_not_a_default` - an absent/partial scope never silently becomes a stronger default authority.
- policy/trust conflicts: `schema::tests::two_scopes_that_disagree_are_both_preserved_verbatim_not_merged` - two scopes with different requested authorities are both readable verbatim; the schema does not pick a winner (that is E11-S02's job).
- malformed/untrusted input: `schema::tests::an_unrecognized_authority_value_is_rejected_not_approximated`, `...an_incorrectly_cased_authority_value_is_rejected`, `...malformed_json_is_rejected_with_a_clear_error_not_a_panic`.
- boundary values: `schema::tests::an_empty_document_with_only_a_schema_version_is_valid`, `...an_empty_scope_key_is_rejected`, `...a_whitespace_only_scope_key_is_rejected`, `...an_empty_pin_session_is_rejected`, `...a_duplicate_pin_session_is_rejected`, `...a_single_pin_and_many_scope_entries_parse_without_pathological_cost` (2,000 provider entries).
- axes 1/2/3/5/6/9 (path/identity, partial reads, links/mounts, concurrency, crash/retry, platform differences) do not apply: this module performs no filesystem I/O, no mutation, and holds no shared state - it is a pure text-to-struct parse/validate function.

CR3's "rollback/recovery behavior" requirement does not apply for the same reason: `parse_policy`
performs no I/O and writes no state anywhere (not to disk, not to any ledger) - there is nothing
to roll back, and no partial-write state a crash could leave inconsistent. `scripts/
check_mutation_boundary.py check` confirms this story added no new caller of the mutation
capability: PASS, unchanged from baseline (only `cancellai-platform::mutation` and
`cancellai-safety::mutation_executor` reference it).

## Verification Commands

```text
cd rust
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test --workspace
cargo deny check
cd ..
python3 scripts/check_schemas.py check
python3 scripts/check_fixtures.py check
python3 scripts/check_docs.py check
python3 scripts/check_mutation_boundary.py check
python3 scripts/project_os.py check
```

All PASS locally. `cargo test -p cancellai-policy --lib`: 69/69 (21 new in `schema::tests`).
Full workspace `cargo test --workspace`: every crate green, no pre-existing test's behavior
changed - the only cross-crate effect is `AuthorityLevel` gaining `Deserialize`, which is
additive (no existing call site matches on the derive list) and does not change `Serialize`
output or `Ord`/`Hash` behavior.

## Compatibility

- No existing public API in `cancellai-policy` or `cancellai-model` changed signature or
  behavior; every other module in both crates compiles and tests unchanged.
- JSON is the format implemented (reusing this workspace's existing `serde`/`serde_json`
  dependency); `docs/architecture/POLICY_MODEL.md` already left the serialization format open
  ("YAML or another readable declarative form once the Rust implementation selects parsing
  dependencies") - see Residual risks.

## Performance / operability

- `an_empty_document_with_only_a_schema_version_is_valid` and
  `a_single_pin_and_many_scope_entries_parse_without_pathological_cost` (2,000 provider entries)
  show parsing is linear and has no pathological cost at a realistic-to-generous document size.
  No filesystem or network I/O in this module - nothing to measure beyond CPU-bound parsing.

## Documentation updated

- `docs/architecture/POLICY_MODEL.md` - new "Rust schema (E11-S01)" section naming the concrete
  types, the versioning/`deny_unknown_fields` pattern, and the explicit boundary with E11-S02.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **Serialization format is JSON, not YAML.** `docs/architecture/POLICY_MODEL.md`'s own example
  is YAML, but the document explicitly defers that choice ("once the Rust implementation selects
  parsing dependencies"). JSON needs no new dependency (ADR-0019 review avoided for this story);
  adopting YAML later is a separate, reviewed decision, not a semantic change to this schema.
- **No duplicate-key detection within one JSON object** (e.g. two `"codex"` keys under
  `providers`): `serde_json`'s own last-value-wins behavior applies, matching ordinary JSON
  semantics elsewhere in this codebase. Not treated as a schema defect for this story; flagged
  here in case a future story wants stricter detection (would require a custom `Deserialize`
  impl or a `serde_json::Map` pre-pass, out of proportion for a CR2 schema story).
- **No resolver, no CLI/TUI wiring.** `parse_policy` is not called from any command surface yet -
  deliberately deferred. E11-S02 (constraint resolver) is the first consumer; nothing in this
  repository parses a user-supplied policy file today.
- **`providers`/`projects`/`artifact_types` keys are validated only for non-emptiness**, not
  against a known provider-id set or a stricter identifier grammar - the core is provider-neutral
  (C-08) and `ProjectRef` is deliberately an arbitrary verbatim string elsewhere in this
  codebase, so a stricter grammar here would risk rejecting valid real project names.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
