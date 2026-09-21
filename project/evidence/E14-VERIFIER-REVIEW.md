# E14 Independent Verifier Review — Round 1

Review-Scope: epic
Round: 1
Verifier: Codex
Date: 2026-09-20
Review-Target: `36952df..f189193` (the target commit remains reachable from the current `HEAD`)

## Brief provenance

| Story | Brief-Checksum |
| --- | --- |
| E14-S01 | Brief-Checksum: f39be6b11563f40d674b85942f630c71ae16f09409c4c1c7cb42314fe6d2caf4 |
| E14-S02 | Brief-Checksum: 8304af70d248d95ff1019bd1546a58ee09461baf3c8db490f0095c2f91f3076f |
| E14-S03 | Brief-Checksum: bed472633028a03e5506c75cb1837c12511b21ed2b3cf472ccfb015e9aa73180 |
| E14-S04 | Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c (updated after the post-round CR2→CR4 reclassification; the review below still reflects what Codex actually read and found in this round) |

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S01 | PASS | `pressure::classify` is a pure function of inputs plus prior state; source inspection found no I/O, clock, shared state, authority type, or consumer in safety/policy/provider code. The existing up-boundary, hysteresis, malformed-input, multi-level, and determinism tests pass. Independent probes covered every boundary; the nominal `0.45` down boundary has the expected sub-ULP binary-float representation effect (`1.0 - 0.55 < 0.45`), but remains deterministic and does not collapse the hysteresis band. `PressureState` does not enter an authority decision, satisfying SI-027. |
| E14-S02 | FAIL | Independent adversarial test over hourly observations `(0, 0.0)`, `(3600, 100.0)`, `(7200, 100.0)` failed: `estimate_growth(..., 3600)` returned `Available { velocity_per_window: 50.0, confidence: Medium }`, and `forecast_time_to_threshold(..., 100.0, 200.0)` returned `Forecast { estimated_seconds_to_threshold: 7200.0, confidence: Medium }`. This is a completed burst followed by silence, not an ongoing trend. |
| E14-S03 | PASS | `baseline` accepts only caller-supplied numeric metadata and has no filesystem/content/path read APIs. Its `VecDeque` evicts at configured capacity before every admission; an independent 100,000-observation corpus with capacity 4,096 remained at 4,096, while a 5,000,000 session count and 20 GB size were both `Anomalous`. No baseline type is consumed by safety/policy/provider authority code, satisfying SI-027. |
| E14-S04 | FAIL | `assess_layout` correctly returns `Drifted`, concrete marker evidence, and `Some(AuthorityLevel::Observe)` for a partial signature; the independent partial-layout probe passed. But `rg` found every `assess_layout`/`LayoutDriftFinding`/`recommended_authority_ceiling` reference only in `cancellai-guardian/src/structural.rs`. None reaches provider capability reporting, policy, or `cancellai_safety::authority::compute_effective_authority`; `authority.rs` explicitly states that `ProviderCapabilityAuthority` is not wired. The value is advisory only, so a caller can retain its prior destructive authority after drift. |

### E14-S01

Verifier: Codex
Verdict: PASS
Brief-Checksum: f39be6b11563f40d674b85942f630c71ae16f09409c4c1c7cb42314fe6d2caf4

### E14-S02

Verifier: Codex
Verdict: FAIL
Brief-Checksum: 8304af70d248d95ff1019bd1546a58ee09461baf3c8db490f0095c2f91f3076f

**Reproduction:** the temporary independent Cargo integration test described above ran `cargo test -p cancellai-guardian --test e14_verifier_adversarial -- --nocapture` and failed. It printed the false `Available` estimate and two-hour `Forecast` before its assertion failed.

**Required repair:** change the fitting/sufficiency logic to reject a burst followed by a sustained plateau (or return `NotTrendingTowardThreshold` for forecasting), rather than treating its aggregate OLS slope as continuing growth. Add a regression fixture for this series and retain the confidence field on all reportable forecasts.

**Violated contract:** E14-S02 AC2, “Sparse/noisy history produces insufficient-data rather than false precision,” and its synthetic trend/burst verification contract. No authority invariant was breached: forecast remains isolated from authority consumers.

### E14-S03

Verifier: Codex
Verdict: PASS
Brief-Checksum: bed472633028a03e5506c75cb1837c12511b21ed2b3cf472ccfb015e9aa73180

### E14-S04

Verifier: Codex
Verdict: FAIL
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

(Checksum updated from the original 06830bdaa24f... after E14-S04 was reclassified CR2→CR4
post-round; the brief's body otherwise carries the same AC/scope text Codex reviewed, and this
round's verdict content is unchanged.)

**Reproduction:** the partial known layout `{config.json, sessions/}` versus observed `{sessions/}` produced `Drifted`, evidence naming the observed marker, and `Some(AuthorityLevel::Observe)`. Static consumer search then established that this recommendation has no use outside `structural.rs`; `cancellai-safety` has no provider-capability constraint in `effective_authority`.

**Required repair:** route a verified layout/capability result into the one existing effective-authority computation as a named monotonic provider-capability/layout constraint. A drifted, empty, or partial signature must bind that computation at `Observe`; a recognized provider name must not bypass it. Keep Guardian detection advisory—it must neither execute a mutation nor introduce a parallel authority decision—and add an end-to-end authority test beginning with a destructive-capable provider input and ending at `Observe` under drift.

**Violated contract:** E14-S04 AC1, “Layout drift can downgrade provider capabilities automatically,” and SI-004. Returning an ignorable recommendation is not an automatic capability downgrade; it permits the TM-05 layout-drift condition to leave destructive authority intact.

## Adversarial assessment

- The independent 100,000-observation corpus used only numeric metadata, remained within its 4,096-entry baseline, and detected both session-explosion and giant-artifact cases. It also confirmed an exact partial layout is drifted with an `Observe` recommendation.
- The executor tests strongly cover pure-module behavior, but E14-S02 does not cover a burst followed immediately by silence, and E14-S04 tests only the returned recommendation rather than its authority-boundary consumption. Those gaps created the two failures above.
- E14-S01/E14-S03/E14-S04 share S01's state vocabulary only as isolated detection primitives. No second pressure or mutation-authority implementation was introduced. The missing E14-S04 integration is a missing safety constraint, not a new safety path.
- GitHub Actions was unavailable at session start (`gh run list --branch main --limit 5` could not connect to `api.github.com`), so remote CI is recorded as unknown rather than treated as green.

## Gate status

| Command | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS before review-state update. |
| `python3 scripts/project_os.py brief E14-S01 --role verifier` | PASS. |
| `python3 scripts/project_os.py brief E14-S02 --role verifier` | PASS. |
| `python3 scripts/project_os.py brief E14-S03 --role verifier` | PASS. |
| `python3 scripts/project_os.py brief E14-S04 --role verifier` | PASS. |
| `cd rust && cargo fmt --check` | PASS. |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS. |
| `cd rust && cargo check --workspace --all-targets` | PASS. |
| `cd rust && cargo test --workspace` | PASS. |
| `cd rust && cargo deny check` | PASS after the sandboxed attempt could not acquire Cargo's read-only advisory-db lock; the permitted rerun reported advisories, bans, licenses, and sources OK (with existing duplicate/unmatched-license warnings). |
| `cd rust && cargo test -p cancellai-guardian` | PASS after temporary verifier probes were removed. |
| `cd rust && cargo test -p cancellai-guardian --test e14_verifier_adversarial -- --nocapture` | FAIL as intended: reproduced the E14-S02 false forecast above. Temporary test removed. |
| `cd rust && cargo test -p cancellai-guardian --test e14_verifier_scale` | Two scale/layout probes PASS; the exact `0.45` nominal down-boundary probe exposed only the documented binary-float calibration effect. Temporary test removed. |
| `gh run list --branch main --limit 5` | UNKNOWN: network connection to GitHub unavailable. |

No Python source file was touched by the review target, so the Python reference-stage gate list does not apply.

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/PROVIDER_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `project/epics/E14.json`
- `project/evidence/E14-S01/VERIFIER_BRIEF.md`
- `project/evidence/E14-S02/VERIFIER_BRIEF.md`
- `project/evidence/E14-S03/VERIFIER_BRIEF.md`
- `project/evidence/E14-S04/VERIFIER_BRIEF.md`

## Overall verdict

FAIL for round 1: E14-S02 and E14-S04 require executor repair. E14-S01 and E14-S03 are moved to
`done`; E14-S02 and E14-S04 are returned to `in_progress`. No E14 story depends on either failed
story, so no E14 story is newly blocked. E14 remains `in_progress`.
