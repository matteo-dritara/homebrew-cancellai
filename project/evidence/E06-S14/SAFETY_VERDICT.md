# Safety Verdict - E06-S14

## Verdict

The latest round below is authoritative.

## Round 10 — 2026-09-24

Verifier: Codex
Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda
Change: published formula rendering and adoption from release assets
Risk: CR4
Commit/PR: `8ea967e..0afe3ad`

### Safety surface changed

`finalize` may replace the live Homebrew formula after verifying published
engine assets and a release manifest. That is a release authority boundary.

### Invariants and adversarial cases

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 / E06-S14 AC3 | Only archives with provenance matching the manifest's release identity are adopted. | `tests/test_round10_adversarial.py` supplies a manifest with a different `source_sha`; `finalize` adopts its formula. The `gh` invocation checks workflow/ref but omits `--source-digest`. | FAIL |

Existing simulated altered archive, missing asset, malformed manifest and
formula-byte cases passed in `tests/test_release.py`; they do not exercise the
manifest source commit against the attestation. The independent adversarial
test fails as expected. `cargo test --workspace` and Rust quality gates pass.

### Known residual risks and recovery

Do not adopt the cutover formula. Keep the current Python formula live until the
manifest source and authenticated archive source are bound and independently
reverified. Main CI was unavailable to this reviewer.

### Owner decision

Pending; this verifier rejects release adoption on the reviewed tree.

FAIL

## Round 11 — 2026-09-24

Verifier: Codex
Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda
Review target: `0afe3ad..2414975a40538cbda0d4a60f59945906d2eba062`
Risk: CR4

### Safety surface and evidence

The release manifest authorizes adoption of a published Homebrew formula.
`tests/test_round11_adversarial.py` constructs a complete manifest that
`release_manifest.validate_document` accepts, with an extra artifact named
`second-macos-arm-archive` carrying the same `aarch64-apple-darwin` target
as the expected archive. All expected archive bytes, digests, formula bytes,
tag commit and mocked provenance agree. `adoptable_formula` returns the
formula instead of refusing. Its selector counts expected names but never
counts all occurrences of a target triple.

| Invariant / obligation | Required property | Result |
| --- | --- | --- |
| E06-S14 AC3, SI-019/C-16 | A manifest target occurs exactly once before its release formula may be adopted. | FAIL |

The exact repair is to count every manifest artifact's target triple,
independent of its name, and refuse duplicate or missing Homebrew targets
before render/adoption. A simulated `finalize` regression must prove the
live formula remains byte-identical on refusal. The current Python formula
stays live; E06-S04 remains blocked. Main CI was unknown because `gh` was
unauthenticated. Full gates and reproduction:
`project/evidence/E06-VERIFIER-REVIEW-ROUND11.md`.

### Owner decision

Pending; this verifier rejects release adoption on the reviewed tree.

FAIL

## Round 12 — 2026-09-24

Verifier: Codex
Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda
Review target: `cdea9e34d4683e71c4a325ba65b7fdb737412c67..02257d24fff44aa728c80e1a9bba1a9365473342`
Risk: CR4

### Safety surface and independent evidence

`finalize` can replace the live Homebrew formula, making the release manifest,
archive hashes and build provenance a CR4 release authority boundary. The
closed-manifest repair rejects round 11's duplicate-target case. An independent
simulated `finalize` with synthetic archive files adopted a valid release and
refused 15 missing, altered and conflicting evidence cases while leaving the
live formula byte-identical. The existing release and manifest tests passed.
The real successful tag run 35875377083 supplied an API response shape that
`check_release_run` accepted. No actual cutover release was performed.

| Invariant / obligation | Required property | Evidence | Result |
| --- | --- | --- | --- |
| E06-S14 AC2–AC3, SI-019 | Only a byte-identical published formula backed by the closed manifest and verified archive bytes may be adopted. | `tests/test_round12_adversarial.py`: positive control and 15 refusal subtests; original formula bytes unchanged on every refusal. | PASS for the simulated boundary |
| E06-S14 AC1, C-16/SI-019 | The tagged release must pass its required gates before it publishes the formula asset. | `ruff format --check .` fails on the story's committed `project/evidence/E06-S14/DESIGN_CONSULTATION_2.md`; main `tests / lint` run 36046575202 failed at the same step. `release.yml` requires this gate in `verify` before `publish`. | FAIL |

### Required repair and recovery

Format the committed design-consultation code fence and demonstrate a green
release gate. Update `docs/RELEASING.md`'s cutover instructions: they still say
`finalize` trusts `.sha256` sidecars, but this story deliberately removes that
trust path. E06-S04 stays blocked and the Python formula stays live. No owner
acceptance of this failed CR4 round is recorded.

Full reproduction and gate details: `project/evidence/E06-VERIFIER-REVIEW-ROUND12.md`.

FAIL

## Round 13 — 2026-09-24

Verifier: Codex
Brief-Checksum: fa4a751b449e2ed74da03c05db7ca490cf6c54f8bf5e6cdf05089a7c0eabcbda
Review target: `b36ce198943233b5f08760b581f7441c2db61bad..2ac50ebaeafae6a8e576f1ee43aaa99ada0c1c4c`
Risk: CR4

### Safety surface changed

`finalize` may replace the live Homebrew formula only after the published formula, closed manifest, tag source archive, and four release archives agree with verified build provenance and the successful tag release run. This is release authority over the binary Homebrew will install; no provider filesystem mutation code changed.

### Invariants and adversarial cases

| Invariant / obligation | Required property | Evidence | Result |
| --- | --- | --- | --- |
| E06-S14 AC1, C-16 | Publish renders the formula from a verified manifest and the tag archive digest. | Release workflow order inspected; an independent temporary-directory CLI simulation generated and verified the manifest from four archive files, rendered the formula, then altered an archive and observed checksum refusal before publication. Latest main release prerequisites, including lint, were green. | PASS |
| E06-S14 AC2–AC3, SI-019 | Adoption requires byte-identical formula text, matching archive bytes and provenance, exact version and target set; refusal leaves the live formula unchanged. | `tests/test_round12_adversarial.py` positive control and 15 fault subtests passed; `tests/test_release.py` and round-11 adversarial tests passed. All asset downloads and attestation/run checks precede `write_atomically`; the formula byte mutation and every missing/altered asset refused with original bytes intact. `check_mutation_boundary.py check` passed. | PASS |

### Differential / compatibility evidence

No Rust source changed in this story. Rust/Python parity and platform checks passed locally; the latest main Rust quality and MSRV jobs passed on macOS, Linux and Windows. The release workflow itself executes only on a tag, so no live cutover release was claimed.

### Known residual risks and recovery

The first tagged cutover remains an integration event: re-run `verify-formula` if release assets or the tag change after finalization. If any evidence is unavailable or disagrees, `finalize` refuses before replacing the Python formula. This review's full local docs suite was contaminated by the untracked `.opencode-run/prompt.md`; origin/main's committed-tree tests and lint were green.

### Owner decision

Pending. This verdict closes E06-S14's implementation review; E06-S04 returns to the review queue. It does not authorize E06-S04's cutover or close the epic.

PASS_WITH_RESIDUALS
