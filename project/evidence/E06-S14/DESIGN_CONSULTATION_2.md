# Design consultation 2 - E06-S14 after review round 11 (2026-09-24)

Not a review round and not a verdict. After round 11 failed E06-S14 again, the owner asked Codex
and OpenCode, with Claude, for a design that closes the class rather than the instance. Codex ran
read-only (`codex exec -s read-only`); OpenCode ran under the E34-S06 sandbox on
`openrouter/nvidia/nemotron-3-ultra-550b-a55b:free` and answered this time.

## What was adopted

All three converged on a *closed* manifest: accept only the exact document the release workflow's
generator writes, instead of looking up expected entries. The implementation follows Codex's
stricter form - byte equality with `release_manifest.build_manifest`'s output rebuilt from
independently derived values (tag commit, local and on GitHub; the run id the verified
attestations name, all four archives agreeing; the digests of the downloaded bytes), and the run
checked on GitHub's run record. Two points of OpenCode's answer were not adopted: refusing the
Windows zip (the real workflow always lists it, so every real release would be refused) and a
`--signer-run-id` flag, which `gh` 2.101 does not have; the run id is read from the attestation
certificate's `runInvocationURI` instead.

## Prompt

### Design consultation - cancellAI E06-S14 (not a review round)

You are one of three models (Claude = executor, Codex, OpenCode) asked by the project owner to stop
a review loop on one story by finding a design that closes the *class* of defect, not the latest
instance. Do not write a verdict and do not modify any file. Read the repository at the current
directory and answer in your final message only.

#### The story and its history

E06-S14 (`project/epics/E06.json`, evidence `project/evidence/E06-S14/`, ADR-0040): the Homebrew
formula is a pure function of the release manifest and the tag archive's digest. `scripts/release.py`
- `manifest_digests`, `manifest_source_commit`, `tag_commit`, `manifest_sha256s`, `verify_provenance`,
`provenance_command`, `engine_sha256s`, `render_release_formula`, `adoptable_formula`, `finalize` -
checks a *published* manifest before adopting the published formula. `scripts/release_manifest.py`
holds the manifest's schema contract (`validate_document`, E17-S01) and the workflow that writes it
is `.github/workflows/release.yml` (`release-manifest-generate`, `publish`).

Each independent round found one more shape the hand-written checks in `release.py` did not
imagine:
- round 9: digests trusted from `.sha256` sidecars instead of the archive bytes;
- round 10 (`project/evidence/E06-S14/ROUND10_FINDINGS.md`, `E06-VERIFIER-REVIEW-ROUND10.md`):
  provenance accepted from any workflow/ref; a manifest naming another commit;
- round 11 (`E06-VERIFIER-REVIEW-ROUND11.md`, `tests/test_round11_adversarial.py`): a manifest with a
  second, differently named artifact for an expected target triple was accepted, because the check
  looks up expected *names* and never counts *targets* across all entries.

#### What to answer

1. Root cause in one paragraph: why does validation-by-lookup keep missing shapes?
2. A design that makes the accepted manifest shape *closed* - so that any manifest that is not
   exactly the one the release workflow would have produced for this tag is refused - rather than
   another enumerated check. Consider, among others: comparing the manifest's whole artifact list to
   the exact expected list; re-using `release_manifest.validate_document` and then requiring equality
   of a canonical projection; deriving the expected manifest from the tag (version, commit, targets,
   workflow) and comparing field by field; refusing unknown fields and extra artifacts (e.g. the
   Windows zip, SBOMs) explicitly; binding `run_id` to the attestation.
3. Enumerate, as exhaustively as you can, the remaining ways a published release could still differ
   from what `finalize` believes it adopts under your design (manifest, archives, formula asset,
   tag, attestation, time-of-check/time-of-use), and say which your design closes and which it
   cannot. We want the next independent round to find nothing new.
4. The smallest adversarial tests that would falsify your design.

Constraints: `AGENTS.md`, `docs/security/SAFETY_INVARIANTS.md` (SI-019), ADR-0040, Python stdlib only
for `scripts/`. Be concrete - cite functions. Under ~1000 words.

## Codex

**Root cause.** `manifest_digests` asks whether each expected artifact *name* exists, then discards the rest of the manifest. Each new check narrows a known path through that lookup; it does not define the complete set of acceptable documents. That is why a second artifact with the same target and a different name survives. `release_manifest.validate_document` defines a broader, valid distribution manifest: it permits multiple artifacts per target, while E06-S14 needs one particular manifest produced by `release-manifest-generate`. Acceptance needs to compare that complete expected document with the published one.

**Closed design.** Give the four archives in `.github/workflows/release.yml`—the three Homebrew tarballs **and the Windows zip**—one ordered release inventory shared with `release.py`. In `manifest_sha256s`, download each published archive under a size cap, compute its SHA-256 from its bytes, and verify its attestation. Establish one trusted run ID from the *verified attestations*, requiring all four to identify the same run, canonical repository and `release.yml`, tag ref, and tag commit. Check that run’s event, ref, commit and successful conclusion through GitHub’s run record. A `run_id` supplied only by the manifest proves nothing.

Build the expected document with `release_manifest.build_manifest`: schema version 1, `release_manifest`, requested version, `stable`, resolved tag commit, canonical repository/workflow, verified run ID, compatibility range `1..1`, and exactly those four ordered `(name, target_triple, computed digest)` entries. Apply `validate_document`, then require equality of the **whole document**, including every nested object and the entire artifact list. Do not form a projection that drops fields. For the literal “what this workflow produced” claim, compare the downloaded manifest bytes with the generator’s `json.dumps(expected, indent=2) + "\n"` output; that also refuses duplicate JSON keys, unknown fields, reordered entries and alternative encodings. Make `render_release_formula` use the same closed validator before `published_engines`, and have `adoptable_formula` consume its verified result. Keep the current byte comparison of the published `cancellai.rb` with `render_formula`. The generic E17-S01 schema remains broad; this is the narrower E06-S14 release profile. This closes the round-11 duplicate without adding a special duplicate-target rule.

**What remains.** This design closes omitted, extra, renamed, duplicated or relabeled manifest artifacts; wrong version/channel/compatibility/commit/workflow/run; altered archives at verification time; and a formula asset that differs at verification time. It also prevents splicing archives attested by different runs. It does **not** by itself authenticate who uploaded the release assets, cover the published `.sha256` sidecars or SBOM files, prove the binary’s behavior, or prove that the publisher ran every intended gate. If the claim extends to the *entire published release*, `finalize` must enumerate release asset names exactly, verify sidecars against archive bytes and verify SBOM assets and their binding; otherwise state that its guarantee covers the formula, manifest and four archives.

A same-user or compromised release workflow can still produce and attest self-consistent bad bytes (the trust limit behind SI-019 and ADR-0040). After `finalize` checks, an editor can replace a release asset, retarget the remote tag, or serve changed bytes; subsequent Homebrew downloads may then differ. The local `tag_commit` check alone does not establish the remote tag’s identity, and `archive_sha256` and archive downloads are separate observations. Pinning immutable release assets or a commit-addressed source archive, restricting tag and asset edits, and re-verifying immediately before distribution reduce that gap; a one-time `finalize` cannot eliminate future replacement. Network or attestation unavailability must continue to refuse.

**Small falsification set.** Run each through simulated `finalize` and assert the live formula remains byte-identical on refusal:

1. A schema-valid fourth artifact with a different name but an existing Homebrew target; also a fifth target and a missing Windows entry.
2. One mutation each to a top-level field, nested identity/compatibility field, artifact name/order, and an unknown nested field or duplicate JSON key.
3. Correct manifest and formula with one changed archive byte; then correct bytes whose verified attestation has another run ID while repository/ref/commit agree.
4. All four archives and manifest agreeing, but `cancellai.rb` changed by one byte.
5. A valid snapshot followed by asset or remote-tag replacement, to demonstrate and document the remaining time-of-check/time-of-use limit rather than claim the gate closes it.

## OpenCode (nemotron-3-ultra, free tier)

Now I have a complete picture. Let me provide the design consultation answer.

---

#### 1. Root cause

Validation-by-lookup (`manifest_digests` at line 466–488) checks that each *expected artifact name* appears exactly once in the manifest. It never counts `target_triple` across *all* entries, so a manifest carrying the three expected names plus a fourth artifact with a different name but a duplicate `target_triple` passes: the lookup finds each expected name once, ignores the extra entry, and returns the digests. Each review round discovers a new "extra entry" shape the lookup didn't imagine (sidecar digests, wrong workflow/ref, duplicate target under another name) because the check is **open-world** — it verifies presence of known-good entries but not absence of everything else.

---

#### 2. Closed-manifest design

Replace the lookup with **exact equality against a derived expected manifest**.

**Step 1 — Canonical projection**
Add a pure function in `release_manifest.py`:

```python
def expected_manifest_projection(
    version: str,
    channel: str,
    source_sha: str,
    repository: str,
    workflow: str,
    run_id: str,
    knowledge_min: int,
    knowledge_max: int,
    targets: tuple[str, ...],
) -> dict[str, Any]:
    """The exact manifest the release workflow must produce for this tag."""
    artifacts = [
        (f"cancellai-cli-{version}-{t}", t, "<placeholder>")  # digest filled at build time
        for t in targets
    ]
    return build_manifest(
        version=version,
        channel=channel,
        source_sha=source_sha,
        repository=repository,
        workflow=workflow,
        run_id=run_id,
        knowledge_min=knowledge_min,
        knowledge_max=knowledge_max,
        artifacts=artifacts,
    )
```

**Step 2 — Strict comparison in `manifest_digests`**
After `validate_document(doc, where)` passes, compute the expected projection using the *same* `version`, `channel`, `source_sha`, `repository`, `workflow`, `run_id`, `knowledge_min`, `knowledge_max` from the manifest itself, and the hard-coded `ENGINE_TARGETS`. Then:

- Require `doc["artifacts"]` length == `len(ENGINE_TARGETS)` (no extra artifacts: no Windows zip, no SBOMs, no second name for any target).
- Require a bijection between `doc["artifacts"]` and the expected list on `(name, target_triple)` — order-independent, but *every* field must match.
- Extract digests from the matched entries.

**Step 3 — Refuse unknown top-level keys**
`validate_document` already rejects unknown keys (line 173–175). Keep it.

**Step 4 — Bind `run_id` to attestation**
`provenance_command` (line 525–534) already passes `--source-digest` (the commit) and `--source-ref` (the tag). Add `--signer-workflow` bound to `doc["build_identity"]["workflow"]` and `--signer-run-id` bound to `doc["build_identity"]["run_id"]` (gh 2.60+ supports `--signer-run-id`). The workflow already emits `run_id` as `GITHUB_RUN_ID`.

This makes the accepted manifest shape **closed**: any manifest not *exactly* the one the release workflow would have produced for this tag is refused.

---

#### 3. Remaining divergence vectors under this design

| Vector | Closed by design? | Notes |
|--------|-------------------|-------|
| Extra artifact (SBOM, Windows zip, duplicate target under another name) | **Yes** | Artifact count + bijection on `(name, target_triple)` refuses any entry not in the expected three. |
| Missing artifact | **Yes** | Count mismatch. |
| Wrong `target_triple` on an expected name | **Yes** | Bijection requires exact triple match. |
| Wrong `source_sha` (manifest names another commit) | **Yes** | `manifest_source_commit` compares to `tag_commit(version)` (line 520). |
| Wrong `workflow` / `repository` in `build_identity` | **Yes** | Checked at line 472. |
| Wrong `run_id` (same workflow, different run) | **Yes** | Provenance command now binds `--signer-run-id`. |
| Tag archive replaced after tag push (TOCTOU) | **No** | GitHub tags are immutable; the archive URL is derived from the tag. A compromised GitHub CDN is outside the threat model. |
| Formula asset differs by one byte from rendered formula | **Yes** | `adoptable_formula` does byte-equality check (line 597–598). |
| Engine archive bytes differ from manifest digest | **Yes** | `engine_sha256s` downloads, hashes, compares (line 562–564). |
| Provenance attestation forged / missing | **Yes** | `verify_provenance` runs `gh attestation verify` bound to workflow, tag, commit, run_id. |
| Manifest published but formula asset not published / published differently | **Yes** | `adoptable_formula` downloads the published `cancellai.rb` and compares to local render. |
| `knowledge_compatibility` range widened | **Yes** | Part of the exact projection; any change fails bijection. |
| Unknown top-level key in manifest | **Yes** | `validate_document` rejects. |

**Cannot close**: A malicious GitHub Actions runner that produces a valid manifest+artifacts+attestations for a commit that isn't the tagged one — but the tag/commit binding and `run_id` binding make this require compromising the Actions control plane, which is the trust anchor.

---

#### 4. Smallest adversarial tests falsifying the design

```python
### tests/test_closed_manifest.py
import hashlib, json, unittest
from unittest import mock
from scripts import release, release_manifest

class ClosedManifestTest(unittest.TestCase):
    def _base_manifest(self, version="2.0.0", extra_artifacts=None):
        commit = "1" * 40
        digest = hashlib.sha256(b"x").hexdigest()
        arts = [
            {"name": f"cancellai-cli-{version}-{t}", "target_triple": t, "sha256": digest}
            for t in release.ENGINE_TARGETS
        ]
        if extra_artifacts:
            arts.extend(extra_artifacts)
        return {
            "schema_version": 1,
            "document_type": "release_manifest",
            "version": version,
            "channel": "stable",
            "source_sha": commit,
            "build_identity": {"repository": release.REPO, "workflow": release.RELEASE_WORKFLOW, "run_id": "42"},
            "knowledge_compatibility": {"min_schema_version": 1, "max_schema_version": 1},
            "artifacts": arts,
        }

    def test_extra_artifact_refused(self):
        m = self._base_manifest(extra_artifacts=[{"name": "cancellai-cli-2.0.0-x86_64-pc-windows-msvc", "target_triple": "x86_64-pc-windows-msvc", "sha256": "2"*64}])
        self.assertEqual(release_manifest.validate_document(m, "t"), [])
        with mock.patch.object(release, "download", side_effect=KeyError), self.assertRaises(release.ReleaseError):
            release.manifest_digests(m, "2.0.0", "t")

    def test_duplicate_target_under_different_name_refused(self):
        m = self._base_manifest(extra_artifacts=[{"name": "second-macos-arm", "target_triple": release.ENGINE_TARGETS[0], "sha256": "3"*64}])
        self.assertEqual(release_manifest.validate_document(m, "t"), [])
        with mock.patch.object(release, "download", side_effect=KeyError), self.assertRaises(release.ReleaseError):
            release.manifest_digests(m, "2.0.0", "t")

    def test_missing_artifact_refused(self):
        m = self._base_manifest()
        m["artifacts"].pop()  # drop one
        self.assertEqual(release_manifest.validate_document(m, "t"), [])
        with mock.patch.object(release, "download", side_effect=KeyError), self.assertRaises(release.ReleaseError):
            release.manifest_digests(m, "2.0.0", "t")

    def test_wrong_run_id_refused_in_provenance(self):
        # The provenance command must include --signer-run-id bound to manifest's run_id
        m = self._base_manifest()
        self.assertEqual(release_manifest.validate_document(m, "t"), [])
        cmd = release.provenance_command("gh", "/tmp/x", "2.0.0", "1"*40)
        self.assertIn("--signer-run-id", " ".join(cmd))  # new flag
        self.assertIn("42", " ".join(cmd))               # value from manifest
```

These four tests cover the class of defects rounds 9–11 found: extra artifacts, duplicate targets, missing artifacts, and unbound `run_id`. Any future shape that differs from the exact expected manifest will fail the bijection check, not require a new enumerated check.
