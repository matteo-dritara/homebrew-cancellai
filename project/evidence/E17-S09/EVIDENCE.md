# Evidence Packet - E17-S09

- Commit/PR: the archive-selection fix on `main`
- Executor: Claude
- Independent verifier: none - the owner waived the Codex round for this session's work. The failing observation is release run 34733067617, job `release-manifest-generate`.
- Change Risk: CR3
- Spec version/commit: `project/epics/E17.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - the archive is identified by suffix | `archive_for()` filters `<name>.*` down to `.tar.gz`/`.zip` instead of taking `sorted(...)[0]`. `test_the_archive_is_chosen_over_the_sbom_that_sorts_before_it` builds the exact directory the release produced - SBOM, archive, checksum sidecar - and asserts the archive's bytes come back. | PASS |
| AC2 - no archive refuses and says what was there | `test_no_archive_is_an_error_that_says_what_was_there`. The pre-existing test for this path asserted the message contained "no file matching"; it now asserts the suffixes and the contents, because the old text would not have told anyone the answer was an SBOM. | PASS |
| AC3 - two archives refuse rather than choose | `test_two_archives_are_refused_rather_than_resolved_by_picking`. Picking is the failure mode that produced this bug in the first place, one level up. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| Release integrity | A published manifest whose checksums describe something other than the artifacts users download | This is the guard that exists to prevent exactly that, and it was comparing against the wrong bytes. It failed closed - `publish` was skipped, nothing shipped - which is the only reason this is a defect and not an incident. | PASS |
| Release integrity | The fix making the guard pass by loosening it | The round-trip test constructs the manifest from the archive and verifies against a directory containing all three files, so a guard that silently matched nothing would fail it. Both refusal paths are also tested. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_release_manifest.py -q -> 43 passed
python3 -m pytest tests -q                          -> 483 passed, 438 subtests
python3 scripts/release_manifest.py check           -> golden document matches the schema
python3 -m mypy scripts/release_manifest.py         -> clean
```

## Compatibility

- Release tooling. No product behaviour, no manifest schema change: the same field is compared
  against different bytes.

## Performance / operability

- Not applicable.

## Residual risks

- **Not verified against the real artifacts.** The v1.13.1 build's archives are still available as
  workflow artifacts and were deliberately not downloaded; the evidence here is a reconstruction of
  the directory shape, not the bytes themselves. The next release run is the real proof.
- **The archive suffix list is fixed.** A future platform packaged as something other than
  `.tar.gz` or `.zip` is refused rather than mis-hashed, which is the right direction but is still
  a list somebody has to remember to extend.
- **This is the fourth latent defect in the release pipeline found by running it.** Two releases
  failed before reaching this job. There is no reason to believe the ones after it have been
  exercised either - `publish` itself has not run since v1.11.0.

## Verifier verdict

closed on the owner's waiver; no independent verdict
