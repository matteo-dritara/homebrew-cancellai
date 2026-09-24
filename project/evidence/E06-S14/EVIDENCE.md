# Evidence Packet - E06-S14

- Commit/PR: the E06-S14 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit; ADR-0040 (owner decision 2026-09-24)

## Outcome

PASS

## Why this story exists

E06-S04's rounds 6-9 each found a new hole in `scripts/release.py`'s after-the-fact reconstruction
of what the formula installs: Ruby nesting misread, adoption guards bypassed, digests taken from
`.sha256` sidecars, the release manifest unconsulted. The design consultation
(`project/evidence/E06-S04/DESIGN_CONSULTATION.md`) and ADR-0040 make the formula a pure function
of the release manifest and the tag archive's digest, rendered where the bytes are built.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the release workflow renders the formula from the verified manifest and publishes it as `cancellai.rb` | `.github/workflows/release.yml` `publish`: after `release_manifest.py verify-checksums release-manifest.json dist` and the tag-archive digest, `release.py render-release-formula --manifest release-manifest.json --source-sha ... --out cancellai.rb`; `cancellai.rb` is a `gh release create` asset. `render_release_formula` validates the manifest (`manifest_digests`) and calls `render_formula`. `test_the_workflow_and_finalize_render_the_same_formula` | PASS |
| AC2 - a published formula is adopted only if byte-identical to the one rendered from the published manifest and tag digest | `adoptable_formula`; `finalize` writes exactly its result for engine versions; `published_digest_problems` (used by `verify-formula`) compares the live formula with it. `test_agreeing_evidence_adopts_the_published_formula`, `test_a_published_formula_one_byte_off_is_refused`, `test_a_live_formula_that_is_not_the_published_one_is_reported` | PASS |
| AC3 - an archive not hashing to the manifest, failing provenance, or missing; a manifest for another version or naming a target other than once: finalize refuses and the live formula is unchanged | `engine_sha256s` downloads every archive (bounded 256 MiB), hashes it, compares it with the manifest and runs `verify_provenance` (`gh attestation verify` on those exact bytes; no `gh` refuses). `--sha256` cannot stand in from the cutover on. Tests: round 9's altered archive, a manifest naming other bytes, another version, a duplicated and a missing target, a provenance failure, each missing asset (manifest, archive, formula); `test_a_refused_adoption_leaves_the_live_formula_unchanged`, `test_sha256_cannot_stand_in_for_the_published_release_after_the_cutover`, `test_verify_provenance_refuses_without_gh` | PASS |

No `.sha256` sidecar is read any more; the sidecars are still published for people who want them.

Mutation checks, each killed: skipping the published-asset comparison; skipping provenance
verification; skipping the archive-to-manifest comparison.

## Round 10 repair (Codex; refused by the harness, `ROUND10_FINDINGS.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| `gh attestation verify --repo` alone accepted an attestation from any workflow or ref of this repository | `provenance_command` adds `--signer-workflow matteo-dritara/homebrew-cancellai/.github/workflows/release.yml` and `--source-ref refs/tags/v<version>`; `manifest_digests` refuses a manifest whose `build_identity` is not this repository's `release.yml` | `test_provenance_is_bound_to_the_release_workflow_and_the_tag`, `test_a_manifest_built_by_another_workflow_is_refused`; mutating the ref to `refs/heads/main` or dropping the identity check each fails a test |

## Round 10 repair (Codex, `project/evidence/E06-VERIFIER-REVIEW-ROUND10.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A manifest claiming `source_sha: 000...000` was adopted: its commit was never compared with the tag's, and provenance was verified without `--source-digest` | `manifest_sha256s` requires the manifest's 40-hex `source_sha` to equal the commit `v<version>` resolves to locally (`tag_commit`; an unresolvable tag refuses), and `provenance_command` passes that commit as `--source-digest`; the release workflow's renderer also refuses a manifest naming no commit | Codex's `tests/test_round10_adversarial.py` (only its `provenance_command` call adapted to the new signature), `test_a_manifest_built_from_another_commit_is_refused`, `test_an_unresolvable_tag_is_refused`, `test_provenance_is_bound_to_the_release_workflow_and_the_tag` (now also `--source-digest`); dropping the commit comparison or the flag each fails a test |

Not bound, recorded: `build_identity.run_id` is not compared with the attestation's run; the
signer workflow, tag ref and commit already fix which build produced the bytes.

## Round 11 and the closed-manifest redesign (`E06-VERIFIER-REVIEW-ROUND11.md`, `DESIGN_CONSULTATION_2.md`)

Round 11 found a manifest carrying a second, differently named artifact for an expected target
accepted: the check looked up expected names and ignored everything else. It was the fourth
shape in three rounds that a hand-written check had not imagined, so instead of a fifth check the
owner asked Codex and OpenCode for a design that closes the class. All three models converged:

| Mechanism | Where | What it closes |
| --- | --- | --- |
| The published manifest must be byte-identical to `release_manifest.build_manifest`'s output, serialized as the generator writes it - first from its own values (`closed_manifest`, before any download or `gh` call), then from independently derived ones | `expected_manifest_text`, `closed_manifest`, `engine_sha256s` step 5 | Extra, missing, renamed, relabelled, reordered or duplicated artifacts (round 11), any changed or unknown field, duplicate JSON keys, another serialization |
| All four archives of `RELEASE_ARCHIVES` - the Windows zip included, as the workflow lists it - downloaded, hashed, and provenance-verified; all must name one run | `engine_sha256s` step 3, `verify_provenance`, `attested_run_ids` | Archive bytes differing (round 9); archives spliced from different runs |
| Provenance: `--signer-workflow`, `--source-ref refs/tags/v<version>`, `--source-digest <tag commit>`, `--deny-self-hosted-runners`, `--format json`; the run id is read from the certificate's `runInvocationURI`, never from the manifest | `provenance_command` | Another workflow, ref, commit (round 10) or a self-hosted runner; a manifest naming another run |
| The run must be this repository's `release.yml`, triggered by the push of `v<version>` at the tag commit, concluded `success` | `check_release_run` | An attested run that was not the tag's successful release |
| The local tag and GitHub's tag must name the same commit | `remote_tag_commit` | Finalizing against a local tag nobody published |
| The release workflow's generator inputs are the ones `finalize` assumes | `test_the_release_workflow_writes_what_finalize_expects` parses `release.yml` | The two drifting apart |

Evidence: `PublishedEngineEvidenceTests` (every earlier round's counterexample, nine manifest
mutations, three serializations, run and remote-tag cases, provenance command and certificate
parsing); Codex's `tests/test_round10_adversarial.py` and `tests/test_round11_adversarial.py` pass
unchanged. Mutation checks, each killed: dropping the final byte equality, the remote-tag check,
the run-record check, the one-run check, `--deny-self-hosted-runners`, or the early closed-shape
check.

**What the guarantee covers:** the manifest, the formula and the four archives, at the moment
`finalize` runs. **Not covered:** the `.sha256` sidecars and the SBOM assets (published, not
adopted); an asset replaced or the tag retargeted after `finalize` (re-run `verify-formula`); and a
compromised release workflow producing self-consistent, attested bad bytes - the trust anchor
ADR-0040 names.

## Verification Commands

```text
python3 -m pytest tests/test_release.py -q   -> 64 passed
python3 -m mypy scripts/release.py           -> no issues
python3 scripts/check_workflows.py check     -> OK
```

## Residual risks

- **Not exercised end to end until a release is cut.** The workflow step runs only on a tag; the
  pieces it calls are unit-tested and the same function renders in CI and in `finalize`.
- **`finalize` needs `gh` and the network**, and provenance verification needs `gh` to be able to
  reach the attestation API. Unavailable means refused, never skipped.
- **An attestation proves where bytes were built, not that they are what users will download
  later**: `finalize` checks the bytes it downloads at that moment; a later substitution of a
  release asset would need `verify-formula` to be re-run.

## Verifier verdict

pending
