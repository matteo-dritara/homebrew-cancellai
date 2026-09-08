# Evidence Packet - E16-S06

- Commit/PR: this working-tree change (executor session, 2026-09-09)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR3
- Spec version/commit: `project/epics/E16.json` (E16-S06), as of this change

## Outcome

PASS

`.github/CONTRIBUTING.md`'s "Provider contributions" section stated, in prose only, that "a
community contribution cannot self-assign Built-in Verified trust or irreversible capability" -
`.github/CONTRIBUTING.md`'s own "What is enforced, and where" table states the standing rule
this violated: "If you find a rule in prose that no check enforces, that is a defect: either
automate it or delete it." This story automates it: `scripts/check_provider_trust.py`
(stdlib-only Python, matching the sibling `scripts/check_*.py` governance checkers) plus a new
`project/provider_trust.json` registry.

## Design summary

- **Manifest lint** (AC1): `lint_manifest` rejects any field on a manifest JSON document, at any
  nesting level (top-level, inside a `root`, inside a `root`'s `markers`, inside an `artifact`),
  that is not part of `cancellai-provider-api::manifest`'s own known struct-field set. This is
  an independent, Rust-toolchain-free re-check of the same `#[serde(deny_unknown_fields)]`
  property that crate's real parser enforces - defense in depth, not a replacement for it.
- **Trust registry** (`project/provider_trust.json`): records, for every shipped manifest, its
  current `cancellai_model::ProviderTrust` tier (`untrusted` for all three today - E16-S03/
  S04/S05 all default to `TrustedTier::untrusted()`), plus `verified_by`/`fixture_references`
  evidence fields.
- **Evidence requirement** (AC2): `validate_registry_entry` requires a non-empty `verified_by`
  and at least one `fixture_references` entry for any tier above `untrusted` - directly
  mirroring `cancellai-safety::trust_promotion::promote`'s `MissingVerifier`/
  `MissingFixtureEvidence` rules (independently re-implemented here, since this Python script
  cannot depend on that Rust crate, and a registry-file edit is what a real PR would actually
  change, not a Rust type).
- **Completeness cross-check**: every manifest's `provider_id` must have exactly one registry
  entry, and every registry entry must name a real, existing manifest - closes both "a shipped
  manifest with no recorded trust decision" and "a registry entry for a manifest that does not
  exist" (a way to fabricate a promotion for something never actually reviewed).
- **CODEOWNERS**: `project/provider_trust.json` added to `.github/CODEOWNERS`, alongside
  `project/decisions.json`/`project/roadmap.json` - so, on top of the automated check, only the
  project owner can merge any change to the registry at all. AC1 ("cannot mark itself") is the
  combination of both controls, not either alone: the check makes a bare claim fail loudly in
  CI even before human review; CODEOWNERS means the human review that could otherwise rubber-
  stamp a plausible-looking evidence claim is always the project owner's own.
- Wired into `.pre-commit-config.yaml` (new `provider-trust-check` hook, scoped to
  `rust/crates/cancellai-provider-api/manifests/*.json`, `project/provider_trust.json`,
  `scripts/check_provider_trust.py`) and `.github/workflows/release.yml`'s `verify` job (both
  the `mypy` invocation and its own `check` step) - `tests/test_workflows.py` already asserts
  every pre-commit gate is mirrored in the release workflow, so this addition was required for
  that pre-existing test to keep passing, not merely good practice.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Community PR cannot mark itself Built-in Verified | `test_manifest_lint_flags_a_smuggled_top_level_trust_field`/`..._capability_field` (manifest-level spoofing) and their three nested variants (root/marker/artifact); `test_checker_flags_builtin_verified_with_no_evidence_at_all` (registry-level bare claim) - all fail the check; `project/provider_trust.json` is CODEOWNERS-protected on top | PASS |
| AC2 - Promotion requires maintainer-owned evidence and compatibility tests | `test_checker_flags_builtin_verified_with_a_verifier_but_no_fixtures`, `test_checker_flags_a_whitespace_only_verified_by_as_self_attested` (a verifier name alone, or a whitespace-only one, is not evidence), `test_a_promotion_with_real_evidence_is_accepted` (the positive case - a promotion with both a real verifier and a fixture reference passes) | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-021 (provider manifest trust bounds authority; cannot self-assign) | A manifest or registry entry attempting to claim `builtin_verified` with no evidence, or smuggling a trust/capability/authority field directly into manifest JSON at any nesting level | `test_manifest_lint_flags_*` (5 tests), `test_checker_flags_builtin_verified_with_no_evidence_at_all`, `test_checker_flags_an_invalid_tier_value` | PASS |

## Verification Commands

```text
python3 -m pytest tests -q
python3 -m ruff check scripts/check_provider_trust.py tests/test_provider_trust.py
python3 -m ruff format --check scripts/check_provider_trust.py tests/test_provider_trust.py
python3 -m mypy scripts/check_provider_trust.py
python3 scripts/check_provider_trust.py check
python3 scripts/check_workflows.py check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
pre-commit run --all-files
```

All PASS locally. `tests/test_provider_trust.py`: 17/17. Full suite `python3 -m pytest tests
-q`: 268 passed (251 before this story, +17 new). `pre-commit run --all-files`: every hook
including the new `provider-trust-check` passes.

## Compatibility

- No existing public API/schema changed; this story adds one new script
  (`scripts/check_provider_trust.py`), one new data file (`project/provider_trust.json`), one
  new test module, one new `pre-commit` hook, one new CI step (mirrored into `mypy`'s argument
  list and its own `check` invocation in `release.yml`), and one new `CODEOWNERS` line.

## Documentation updated

- `.github/CONTRIBUTING.md` - "Provider contributions" section now names the automated check and
  the CODEOWNERS protection; "What is enforced, and where" table gained a row.
- `docs/development/ENGINEERING_SYSTEM.md` - "Automation principle" gained a bullet naming this
  check.
- `.github/CODEOWNERS` - `/project/provider_trust.json` added.
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- **"Compatibility tests" (part of AC2's wording) is represented here only by the
  `fixture_references` field being non-empty, not by this script independently re-running those
  fixtures.** Actually executing a named fixture reference's compatibility test is
  `cancellai-provider-api`'s own test suite's job (already exercised per-manifest by
  `builtin.rs`'s `the_committed_*_manifest_parses` and realistic-layout tests); this story's
  registry only records that such evidence exists and names it, which is the auditable claim a
  human reviewer (gated by CODEOWNERS) is expected to actually verify before merging a
  promotion - consistent with `docs/security/SUPPLY_CHAIN.md`'s own "independent AI verification
  is useful evidence but is not cryptographic human separation of duties" disclosure for a
  single-maintainer project.
- **No CI job today runs on a fork's pull request with restricted permissions specifically to
  demonstrate this check succeeds under a minimal `GITHUB_TOKEN`** - the check itself performs
  no writes and needs no elevated permissions to run (`check_provider_trust.py check` only
  reads `project/provider_trust.json` and the manifest directory), so it is safe under any
  workflow permission level `scripts/check_workflows.py` already enforces across this
  repository's workflows; a dedicated adversarial "run this exact check as an untrusted fork PR
  and confirm no privilege is available to it" CI scenario was judged out of proportion for what
  the script's own read-only design already guarantees by construction.
- **A registry entry claiming a tier for a `provider_id` that has since had its manifest file
  deleted is not specifically distinguished from "never had one"** - both surface as the same
  "names no manifest" error; disclosed as a minor diagnostic-quality gap, not a safety gap
  (either way, the check fails closed).

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
