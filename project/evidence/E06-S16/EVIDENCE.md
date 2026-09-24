# Evidence Packet - E06-S16

- Commit/PR: the E06-S16 commits on `main`, and the v1.21.1 release
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR3
- Spec version/commit: `project/epics/E06.json` at this commit; owner decision 2026-09-25

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - `verify-release` applies every finalize check to a published release and writes nothing | `scripts/release.py verify_release`: `engine_sha256s` (closed manifest, local and remote tag, four archives, provenance, run), then `cancellai.rb` byte-compared with `render_formula` - the engine formula from 2.0.0, the Python-only one before. `test_verify_release_accepts_a_verified_release_and_refuses_a_wrong_formula_asset`, `test_verify_release_checks_a_pre_cutover_release_against_the_python_formula` | PASS |
| AC2 - v1.21.1 published by release.yml and verified on its real assets | Release run `36069385676`, all eleven jobs green; `verify-release --version 1.21.1` passed on the first attempt on the real assets (manifest byte-identical, four archives hashed and provenance-verified from one run, the tag's successful release run, `cancellai.rb` byte-identical); `finalize` then wrote a formula byte-identical to the published asset. Recorded in `project/evidence/RELEASE-v1.21.1.md` | PASS |
| AC3 - a difference refuses and names the asset | Every refusal in `engine_sha256s`/`verify_release` names the URL or run it concerns; `PublishedEngineEvidenceTests` | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_release.py -q   -> 75 passed
```

## Residual risks

- The rehearsal proves the pipeline on a release that does not switch the formula; the one step
  it cannot exercise is `finalize --adopt-cutover` writing the engine formula, which E06-S14's
  simulated `finalize` tests and E06-S15's authorization check cover.

## Verifier verdict

pending
