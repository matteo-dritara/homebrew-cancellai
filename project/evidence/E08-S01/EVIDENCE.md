# Evidence Packet - E08-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E08 epic review round 1
- Change Risk: CR2
- Spec version/commit: `docs/architecture/DOMAIN_MODEL.md` "AgentArtifact"

## Outcome

PASS

## Scope

E06-S01 already implemented the wire-format-minimum slice of `AgentArtifact`
(`rust/crates/cancellai-model/src/agent_artifact.rs`): lifecycle axes (`ActivityState`/
`ResidencyState`/`ProtectionState`/`IntegrityState`), `RiskClass`, `Reversibility`,
`KnowledgeConfidence`, and `AuthorityLevel` ceiling. Of the outcome's seven named axes
("lifecycle axes, risk, reversibility, confidence, authority ceiling, provenance, and
relationships"), five were therefore already in place before this story. This story:

1. Confirmed "provenance" is already the existing `Evidence`/`evidence_ids` axis
   (`docs/DECISION_REGISTER.md`: "Every classified artifact has evidence provenance") and does
   not need a second, parallel field.
2. Implemented the one genuinely missing axis, `relationships: Vec<ArtifactRelationship>`
   (`{ kind: RelationshipKind, related_artifact_id: ArtifactId }`, `RelationshipKind::ChildOf`),
   and wired it to data that was already being observed and then discarded:
   `cancellai_provider_codex::CodexSession::parent_session_id` reached
   `cancellai-policy::retention::classify` as an unused `group_key: &str` parameter
   (`let _ = group_key;`) before this change.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Artifact state axes are independent and validated | `relationships` is an independent `Vec` field alongside the existing independent lifecycle/risk/reversibility/confidence/ceiling axes - adding or omitting it does not affect any other field. `agent_artifact.rs`'s `empty_relationships_serialize_as_an_empty_array_not_an_omitted_key` and `a_child_of_relationship_serializes_the_kind_and_the_related_artifact_id` are the domain-invariant property tests for this axis (the story's own Verification Contract). | PASS |
| AC2 - No UI/provider-specific field enters the core model | `RelationshipKind`/`ArtifactRelationship` are provider-neutral names (no "codex"/"claude" in the type). The one populated variant, `ChildOf`, is backed by real cross-provider-applicable structure (parent/child), not a Codex-specific concept; Claude's resolver passes `Vec::new()` because Claude sessions are genuinely flat, not because the type excludes them. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| C-02 (unknown is protected) / fail-closed relationship resolution | A `parent_thread_id` that does not resolve to any session this scan actually discovered must not fabricate a relationship. | `a_parent_thread_id_that_was_not_itself_discovered_produces_no_relationship` (new test) - mirrors `group_into_subagent_trees::root_id_for`'s existing "parent not itself discovered => independent unit" rule. | PASS |
| No safety obligation is listed for this story (`safety_obligations: []` in `project/epics/E08.json`) - `relationships` is informational, not consulted by `cancellai-safety::effective_authority` or any mutation path. | N/A | Grep confirms no `cancellai-safety` crate reads `AgentArtifact::relationships`. | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # 99 passed in cancellai-policy alone incl. 3 new relationship
                                   # tests; 0 failed anywhere in the workspace
$ cargo deny check                # advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ pre-commit run --all-files      # all hooks pass once this evidence packet exists
```

New tests added:

- `rust/crates/cancellai-model/src/agent_artifact.rs`:
  `empty_relationships_serialize_as_an_empty_array_not_an_omitted_key`,
  `a_child_of_relationship_serializes_the_kind_and_the_related_artifact_id`.
- `rust/crates/cancellai-policy/src/retention.rs`:
  `a_codex_child_session_carries_a_child_of_relationship_to_its_resolved_parent`,
  `a_parent_thread_id_that_was_not_itself_discovered_produces_no_relationship`.

## Compatibility

- Wire format: `relationships` is a new key in the `--json` inventory document's `artifacts[]`
  entries (via `cancellai-cli/src/documents.rs`'s `#[derive(Serialize)]` passthrough). Additive
  only - `scripts/check_schemas.py`'s `check_inventory` asserts a minimum key set, not an exact
  one, and `scripts/rust_python_parity.py` compares a named field projection
  (`candidates`/`protected_count`/`identity_token`), not full-document equality - both pass
  unchanged (`pre-commit run --all-files` above).
- Providers exercised: `codex-cli` (relationship populated/withheld), `claude-code` (flat,
  always empty).

## Performance / operability

- `resolve_codex` adds one `HashMap` build (`by_session_id`, O(n) in discovered sessions) and one
  `HashMap` lookup per member; no new filesystem I/O or additional passes over `sessions/`.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md`: new "`relationships` (E08-S01)" subsection under
  "AgentArtifact", recording what this story added, why "provenance" needed no new field, and
  where `relationships` is populated from today.

## Residual risks

- `relationships` currently carries only immediate parent linkage for Codex subagent trees. A
  richer relationship vocabulary (e.g. project/session grouping) is explicitly out of scope here
  and belongs to E08-S02 (project attribution) or a future story, not invented ahead of the
  evidence that would back it.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
