# E14 Independent Verifier Review — Round 2

Review-Scope: epic
Round: 2
Verifier: Codex
Date: 2026-09-21
Review-Target: `3374c75..d9bd332`

This round re-reviews only the two stories returned by round 1. E14-S01 and E14-S03 remain
closed and are not re-judged.

## Brief provenance

| Story | Brief-Checksum |
| --- | --- |
| E14-S02 | Brief-Checksum: 8304af70d248d95ff1019bd1546a58ee09461baf3c8db490f0095c2f91f3076f |
| E14-S04 | Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c |

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S02 | PASS | Independent temporary tests confirmed that `estimate_growth` rejects a five-point completed burst followed by a flat plateau, the same plateau with small alternating noise, and a post-burst reversal, all as `InsufficientData(NoDiscernibleTrend)`. The repaired recent-half slope test therefore closes round 1's false continuing forecast without rejecting the normal continuing-trend suite. Brief-Checksum: 8304af70d248d95ff1019bd1546a58ee09461baf3c8db490f0095c2f91f3076f |
| E14-S04 | FAIL | A real `assess_layout` drift passed through `effective_authority_after_layout_assessment` and yielded `Observe`, but the equally public existing `cancellai_safety::effective_authority` accepted the same maximally permissive `AuthorityInputs` and yielded `Autopilot`. Its typed `AuthorityInputs` has no provider-capability field. Also, `LayoutDriftFinding` has public fields, so a caller can replace a real drift finding with `recommended_authority_ceiling: None` before invoking the bridge. `compute_effective_authority` remains publicly re-exported with caller-supplied constraints. The cap is thus opt-in and bypassable, not the mandatory automatic downgrade required by AC1/SI-004. Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c |

## Independent adversarial reproductions

### E14-S02 — burst, plateau, noise, and reversal

A temporary `cancellai-guardian` integration test, removed before this record, passed these
three independently selected sequences through the public `estimate_growth` API (one-hour
spacing):

1. `(0, 0)`, `(1h, 100)`, `(2h, 100)`, `(3h, 100)`, `(4h, 100)`;
2. `(0, 0)`, `(1h, 100)`, `(2h, 100.2)`, `(3h, 99.8)`, `(4h, 100.1)`, `(5h, 99.9)`;
3. `(0, 0)`, `(1h, 100)`, `(2h, 100)`, `(3h, 95)`, `(4h, 90)`.

Each returned `GrowthEstimate::InsufficientData(NoDiscernibleTrend)`. These probe longer
plateaux, low-amplitude late noise, and a reversal rather than reproducing the executor's
three-point exact plateau fixture. The existing workspace suite also passed its continuing linear
and moderate-noise trend cases, so no false-withhold regression was observed in the required
test corpus.

### E14-S04 — authority bypass

The same temporary integration test used only public APIs. It built a legitimately promoted,
otherwise maximally permissive `AuthorityInputs`, created a real drift finding with
`assess_layout("recognized-provider-name", {"sessions/"}, {"new-layout/"})`, and observed:

- `effective_authority_after_layout_assessment(inputs, &finding).level == Observe`;
- `cancellai_safety::effective_authority(inputs).level == Autopilot`;
- replacing that finding with the public `LayoutDriftFinding { support: Recognized, evidence:
  ..., recommended_authority_ceiling: None }` made the bridge return `Autopilot` too.

This is a public-API counterexample, not a test-only constructor or a direct mutation path. The
workspace-wide consumer search independently confirms that `cancellai-policy`'s public
`resolve_effective_authority` and other established consumers still call the unceiled
`effective_authority`; no production authority consumer is required to select the new wrapper.

## CR4 Safety Verdict — E14-S04

No PASS/PASS_WITH_RESIDUALS Safety Verdict is supportable.

The changed surface is the safety-kernel authority boundary: a provider-layout capability ceiling
is claimed to be the ninth monotonic Effective Authority constraint. SI-004 requires an unknown
or drifted provider layout to reduce capability. The new helper correctly computes that result
when selected, but it neither replaces nor constrains the pre-existing public authority APIs, and
the drift result itself is forgeable/discardable as public data. Therefore a caller can retain
destructive authority after a real drift finding. This violates SI-004, C-02, C-05, and TM-05.

The workspace's formatter, clippy, compile, test, dependency-policy, governance, documentation,
schema, fixture, differential/parity, mutation-boundary, provider, platform, process, evidence,
and safety-oracle gates pass. They do not establish that every public authority consumer must
carry the provider-capability ceiling, which is the failed property here.

## Required owner decision — no third patch-and-repeat round

E14-S04's current approach has failed twice. Round 1 found an advisory ceiling with no authority
consumer; round 2 finds that the added consumer is optional and bypassable. Adding another helper
or another call-site test would repeat the same structurally wrong design: a separate public
authority API can never make a safety ceiling mandatory while older public APIs omit it.

An owner-level ADR/scope decision is required before further implementation. It must choose a
single mandatory authority-construction boundary for provider capability, including how callers
obtain a non-forgeable layout/capability result, how existing `effective_authority`,
`compute_effective_authority`, and policy-resolution consumers are retired/restricted/migrated,
and what public-API compile-fail or external-crate tests prove that a drifted result cannot be
discarded. A new work item must own that redesign and any implementation. This review does not
authorize a third E14-S04 repair cycle.

## Gate status

| Command | Result |
| --- | --- |
| Temporary independent `cargo test -p cancellai-guardian --test e14_round2_independent -- --nocapture` | PASS — three completed-burst shapes withheld; public authority bypass reproduced. Temporary test removed. |
| `python3 scripts/project_os.py check` | PASS before review-state update. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS |
| `cd rust && cargo deny check` | PASS on approved rerun after the sandbox could not acquire Cargo's read-only advisory-db lock; existing duplicate/unmatched-license warnings only. |
| `python3 -m pytest tests -v` | PASS — 668 tests, 583 subtests. |
| Repository `.venv`: Ruff check/format and mypy | PASS |
| Full AGENTS.md Python checker list | PASS — including docs, fixtures, schemas, characterization, diff/parity, workspace, mutation boundary, provider/platform, process, release, evidence, handoff, safety-oracle, EARS, and sensitivity gates. |
| `gh run list --branch main --limit 5` | PASS — five latest visible `main` workflows succeeded. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `project/epics/E14.json`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/development/WORK_ITEM_MODEL.md`
- both E14-S02/E14-S04 verifier briefs and evidence packets
- `project/evidence/E14-VERIFIER-REVIEW.md`
- changed Guardian and safety authority sources plus their workspace-wide consumers

## Overall verdict

E14-S02 is **PASS** and moves to `done`. E14-S04 is **FAIL** and moves to `blocked` pending the
owner-level structural decision above; no CR4 Safety Verdict pass is issued. E14 cannot close:
not every story is done, SI-004 remains unmet, and PD-021 release evidence cannot be prepared for
an epic whose CR4 authority story is blocked. The epic remains `in_progress`.
