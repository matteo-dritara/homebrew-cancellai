# Evidence Packet - E01-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E01)
- Change Risk: CR1
- Spec version/commit: `docs/architecture/DOMAIN_MODEL.md` as amended in this change

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Terms are defined once and reused across architecture, schemas, tests, and UI contracts | `docs/architecture/DOMAIN_MODEL.md` now defines all nine outcome terms (`AgentArtifact`, `ProviderRoot`, `Evidence`, `KnowledgeConfidence`/Confidence, Lifecycle axes, `Effective Authority`, `Action`, `SealedPlan`, `Results`) in one place; `docs/architecture/PROVIDER_MODEL.md` and `docs/architecture/AS_IS.md` were updated to link to the canonical `ProviderRoot` definition instead of using the phrase undefined. `grep` across `docs/architecture/*.md`, `docs/security/*.md` confirms no other document declares a conflicting definition of these terms (all prior uses were lowercase prose consistent with the new canonical nouns). | PASS |
| AC2 - Ambiguous legacy terms are mapped or deprecated | New `## Legacy vocabulary` section in `docs/architecture/DOMAIN_MODEL.md` maps every legacy `cancellai.py` domain name (`Action`, `Plan`, `CleanResult`, `RootAuthority`, `Scan`, `CoverageBucket`, `ProcessObservation`) to exactly one canonical term or explains why it is not promoted to a standalone noun, and lists three deprecated ambiguous phrasings ("cleanup" as a noun, "safe to delete" as a boolean, "root path" used for both the path and the capability) that must not appear in new architecture/schema/UI text. `docs/architecture/AS_IS.md`'s `RootAuthority` bullet links forward to the mapping so the legacy reference document and the canonical glossary do not diverge. | PASS |

## Safety Evidence

None. `safety_obligations: []` for this story; it is documentation-only (CR1, no code or schema changed).

## Verification Commands

```text
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
python3 scripts/release.py check
```

All five passed. `check_docs.py` is the "documentation consistency check" named in the story's verification contract: it fails on dangling local links, unreachable documents, and safety-invariant ID drift; all new cross-references (`DOMAIN_MODEL.md#providerroot`, `DOMAIN_MODEL.md#legacy-vocabulary`) resolved.

No Python source, schema, or test file was touched, so `pytest`/`ruff`/`mypy` are not applicable to this change; they were not run.

## Compatibility

- Documentation-only change. No platform, provider, or schema behavior changed.

## Performance / operability

- Not applicable.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md` - added `ProviderRoot`, `Action`, and `Legacy vocabulary` sections (the story's declared documentation impact).
- `docs/architecture/PROVIDER_MODEL.md` - linked "provider root" to the canonical `ProviderRoot` definition.
- `docs/architecture/AS_IS.md` - linked the legacy `RootAuthority` bullet to the canonical term and the legacy-vocabulary mapping.

## Residual risks

- `docs/CLI.md`, UI/schema contracts, and test code do not yet reference this vocabulary by name; that reuse arrives with E01-S03 (versioned plan/result JSON contracts) and later Rust-target work, not this story, whose scope is limited to freezing the definitions themselves.
- The Python v1 reference intentionally keeps its legacy names (`Action`, `Plan`, `CleanResult`, `RootAuthority`, ...) unchanged per the AS_IS.md freeze rule; the mapping table is the only place divergence is reconciled until Rust implements the canonical types directly.

## Verifier verdict

PENDING - epic E01 review runs once every story in E01 is `ready_for_review` (at most twice per epic, per ADR-0014).
