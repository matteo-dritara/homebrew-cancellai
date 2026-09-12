# E09 Closure - owner-directed, self-review accepted in lieu of independent review

- Epic: E09 - Atlas TUI
- Decided by: **project owner** (Matteo Pugliese)
- Date: 2026-09-12

## What this file is, and is not

This is not an independent verification. `docs/development/AGENT_PROTOCOL.md`'s standing
assignment names Codex as the independent reviewer and Claude as the executor; no Codex CLI is
available in this environment. `project/evidence/E09-SELF-REVIEW.md` is Claude reviewing
Claude's own work through a fresh, context-isolated session - real adversarial effort, real
reproductions, but not independent in the sense the protocol means, since it shares the
executor's model rather than rotating to a genuinely different one.

The owner reviewed that self-review's findings and explicitly decided to accept it as sufficient
to close this epic, rather than wait for Codex CLI access. This file records that decision
rather than letting the epic close with a `done` status nobody can trace to a choice, matching
the precedent `project/evidence/E21-CLOSURE.md` set for the same situation (owner-directed
closure without spending the process's normal verification step).

## What the self-review found, and what closes it

| Finding | Story | Resolution |
| --- | --- | --- |
| `filesystem_kind.rs`'s `classify_filesystem_name` and its two backing constants were dead code on Windows, breaking the mandatory `cargo clippy --workspace --all-targets --all-features -- -D warnings` gate on `windows-latest` | E10-S01 (see `E10-CLOSURE.md`) | Repaired and cross-platform re-verified before this closure - see `project/evidence/E10-S01/EVIDENCE.md`'s addendum |
| `App::handle_key`'s confirmation guard matched `KeyCode::Char('c')` without checking modifiers, so Ctrl+C (raw mode delivers it as `Char('c')` + `CONTROL`) could complete a pending irreversible-action confirmation instead of cancelling it | E09-S04 (CR3, SI-016) | Repaired via a shared `is_plain_c` predicate, pinned by a new regression test - see `project/evidence/E09-S04/EVIDENCE.md`'s addendum |
| `draw_explanation_detail` renders only the first relationship when an artifact has several | E09-S03 | Left as a disclosed residual (cosmetic, non-blocking, per the self-review's own recommendation) |
| The TUI's confirmation gate keys on `Reversibility::Irreversible` rather than asserting `reversibility_allowed(action_class, reversibility)` the way `mutation_executor` does - an emergent, unenforced coupling rather than a type-checked invariant | E09-S04 | Left as a disclosed residual (not exploitable today; would silently under-confirm a real deletion only if a future change breaks the coupling `retention.rs::classify` currently maintains between `Reversibility::Unknown` and an `Observe`-only ceiling) |

Both repaired findings were fixed by the executor immediately after the self-review, before this
closure - not carried forward as accepted risk, since they were cheap and concrete.

## Residual risk the owner accepts

1. **No independent confirmation of any of E09's four stories**, including the CR3/SI-016 story
   (E09-S04). Every claim in the self-review is real reproduction work, but by the same agent
   family as the executor - systematic blind spots correlated with that model are not
   mitigated by session freshness alone.
2. **The two disclosed residuals above** (E09-S03's single-relationship rendering,
   E09-S04's unenforced reversibility/action-class coupling) remain open, unassigned to a
   specific follow-up story. A future story that touches either area should re-examine them.
3. **Live-scan wiring remains deferred** (E09-S02/S03/S04's own recorded residual): the shipped
   TUI shows "not loaded yet"/"no artifacts loaded" states rather than a real scan, so several of
   the CR3 story's safety-relevant behaviors (the confirm/cancel state machine in particular)
   have not been exercised against real engine data end to end, only against synthetic
   `PlanContext` values in unit tests.
4. **Real-terminal verification** (E09-S01's own disclosed residual) happened once on a real
   macOS terminal outside any automated environment; Linux/Windows real-terminal verification
   remains a manual follow-up, not exercised by CI (which cannot allocate a real PTY).

## Consequence for E11

`E11` (Deterministic Policy Engine) declares an epic-level dependency on `E09`. Per
`docs/development/WORK_ITEM_MODEL.md`'s dependency rule, an epic-level dependency requires
`done`, which this closure satisfies in the control plane. Work on E11 may proceed.

## Release

This epic closes together with E10 in the same release, `project/evidence/RELEASE-v1.13.0.md` -
see that file and `project/evidence/E10-CLOSURE.md`'s own "Release" section for why one release
names both.
