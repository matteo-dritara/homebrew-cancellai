# Safety Verdict - E06-S12

Verifier: Codex
Brief-Checksum: 74c57318540c3cddc9de9c795d078cdc82f0d051c2935ab0169c875153ce2006

- Reviewed commit: `85fed2f7c66baf9da3ed79f1bb870f4277eec434`; risk CR4.
- Surface: Codex lineage content read and Claude companion failure retention, both feeding scan completeness before planning.

## Verdict

`PASS`

## Invariants

| Invariant | Evidence | Result |
| --- | --- | --- |
| SI-008 | Independent mode-000 rollout and 70-failure companion trees caused incomplete scopes and zero delete actions in default and custom roots; the stale files survived. | PASS |
| SI-009 | `io::Result<Option<String>>` distinguishes failed reads from successfully parsed no-parent content. The failure is recorded, not treated as absence. | PASS |
| SI-010 | Codex records the path and OS cause in `ReasonLog`. Claude companion walker records each failure directly into `ReasonLog` instead of buffering an unbounded vector; error_count remained exactly 70. | PASS |
| SI-019 | The scope downgrade reaches the shared plan and one mutation executor. The static mutation-boundary check passed. | PASS |

## Adversarial cases

- In two independent synthetic homes, each with default and custom provider roots, a stale Codex rollout had readable entry metadata but mode-000 content. Rust `inspect` exited 4 with `complete=false,error_count=1`; Rust `plan` had zero deletes; Python reference exited 4; the file survived.
- In both root origins a Claude companion had 70 unreadable descendants. Rust `inspect` exited 4 with error_count=70, `plan` had zero deletes, Python exited 4 and the session survived. The adapter test checks retained reasons are capped at `MAX_RETAINED_REASONS` (64).
- `read_parent_from` propagates `read_until` errors through `?`; valid unknown or no-parent content stays `Ok(None)`. Bounded 10-record/512 KiB parsing remains unchanged. The owner accepted the separate E21-S07 Unix leaf race in E06-S06's append-only Safety Verdict.

## Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy/tests and Windows-target Clippy | PASS |
| Stable-channel CLI full suite with kill-points | PASS on rerun |
| Python tests and 14-fixture Rust/Python parity in both origins | PASS |
| Mutation-boundary, project OS and verifier handoff | PASS |
| Native Linux/Windows permission fixture | NOT RUN locally; exact-main CI suites passed on both OSes |

### Owner decision

PENDING

The Unix leaf-name residual has its separate owner disposition; this verdict records no new accepted safety gap.

PASS
