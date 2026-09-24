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
