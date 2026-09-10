# Evidence Packet - E08-S02

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E08 epic review round 1
- Change Risk: CR2
- Spec version/commit: `docs/architecture/DOMAIN_MODEL.md` "AgentArtifact" / "`project_attribution`
  (E08-S02)"

## Outcome

PASS

## Scope

Added `AgentArtifact::project_attribution: Option<ProjectAttribution>` (`None` = `Unattributed`),
where `ProjectAttribution { project_ref: ProjectRef, source: AttributionSource, confidence:
KnowledgeConfidence }` and `AttributionSource { ExplicitProviderMetadata, KnownPath,
ObservedEvidence }`. Populated in `cancellai-policy::retention::classify` from a new `project:
Option<&str>` parameter:

- Claude: `Some(session.project.as_str())` - the provider's own `projects/<name>/` directory name,
  taken verbatim as `AttributionSource::ExplicitProviderMetadata`. Deliberately *not* decoded into
  a claimed real filesystem path - Claude Code's actual `/`-to-`-` encoding is lossy/ambiguous for
  path segments containing `-`, and guessing would risk exactly the overclaim SI-023 forbids.
- Codex: `None` - `cancellai-provider-codex` groups only by subagent tree, no project concept
  exists to attribute from.
- A blank/whitespace-only project name resolves to `Unattributed`, not a hollow `ProjectRef`.
- `KnownPath`/`ObservedEvidence` are defined (matching the story outcome's own three-source
  vocabulary) but unpopulated by any adapter today, same "field exists for a future real
  producer, not invented now" precedent as `FileFacts::provider_hint`.
- `project_attribution.confidence` starts equal to the artifact's own `knowledge_confidence` and
  is downgraded alongside it wherever `resolve_claude`/`resolve_codex` already downgrade
  `knowledge_confidence` for a partial/incomplete scan (SI-008/SI-009).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Uncertain attribution remains Unattributed | `a_blank_project_name_resolves_to_unattributed_rather_than_a_hollow_project_ref` (blank/whitespace metadata); `a_codex_session_is_always_unattributed` (no project concept at all). | PASS |
| AC2 - Attribution records evidence source and confidence | `a_claude_session_is_attributed_to_its_project_directory_verbatim` asserts `project_ref`, `source == ExplicitProviderMetadata`, and `confidence == artifact.knowledge_confidence` are all present on the record. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-023 (attribution uncertainty cannot become cleanup confidence) | A scan degraded after discovery (unreadable companion directory) must not leave an ordinary, perfectly-readable session's `project_attribution.confidence` higher than the artifact's own downgraded `knowledge_confidence`. | `a_degraded_scan_downgrades_project_attribution_confidence_alongside_the_artifact` (new test, reuses the existing degraded-companion fixture) - asserts every artifact in the partial scope reports `project_attribution.confidence == LowUnknown`, including the two sessions whose own evidence was perfectly readable. | PASS |
| SI-023, blank-metadata form | Degenerate metadata (blank name) must not become a `ProjectAttribution` with any confidence at all, since that would imply a stronger claim than the evidence supports. | `a_blank_project_name_resolves_to_unattributed_rather_than_a_hollow_project_ref`. | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-policy: 22 passed (5 new for this story);
                                   # cancellai-model: 13 passed (2 new); 0 failed anywhere
$ cargo deny check                # advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ pre-commit run --all-files
```

New tests added:

- `rust/crates/cancellai-model/src/agent_artifact.rs`:
  `unattributed_serializes_as_an_explicit_null_not_an_omitted_key`,
  `a_project_attribution_serializes_the_ref_source_and_confidence`.
- `rust/crates/cancellai-policy/src/retention.rs`:
  `a_claude_session_is_attributed_to_its_project_directory_verbatim`,
  `a_blank_project_name_resolves_to_unattributed_rather_than_a_hollow_project_ref`,
  `a_degraded_scan_downgrades_project_attribution_confidence_alongside_the_artifact`,
  `a_codex_session_is_always_unattributed`.

## Compatibility

- Wire format: `project_attribution` is a new, additive-only key in the `--json` inventory
  document's `artifacts[]` entries (serializes as an explicit `null` when `Unattributed`, never an
  omitted key). `scripts/check_schemas.py`/`scripts/rust_python_parity.py` are unaffected for the
  same reasons documented in E08-S01's evidence packet (minimum-key-set / field-projection
  comparisons, not full-document equality).
- Providers exercised: `codex-cli` (always Unattributed), `claude-code` (attributed and blank-name
  cases).

## Performance / operability

- No new I/O: `project_attribution` is derived from data (`ClaudeSession::project`) already read
  during discovery.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md`: new "`project_attribution` (E08-S02)" subsection under
  "AgentArtifact".
- `CHANGELOG.md` Unreleased/Added.

## Residual risks

- Real path-based attribution (`AttributionSource::KnownPath`) and true moved/deleted-project
  detection both need a real path decoder and cross-scan history the current reconstructible
  cache does not keep - explicitly deferred, not attempted as a guess here (see DOMAIN_MODEL.md's
  own note). A future story adding either should land as a dedicated, reviewed change given the
  ambiguity Claude's own directory-name encoding carries.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
