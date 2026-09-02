# E07-S01 Dependency Escalation

- Work item: E07-S01 - Unix platform backend (CR3)
- Executor: Codex (`/root`)
- Date: 2026-09-02
- Status: blocked before implementation; no code or test work started.

## Conflict

`project/epics/E07.json` declares E07-S01 dependent on E06-S04. E06-S04 is currently
`blocked`, as confirmed by `project/evidence/E06-S04/EVIDENCE.md` and the generated executor
brief. The control-plane dependency therefore prevents E07-S01 from starting.

However, `docs/development/RELEASE_GATES.md`'s E06-S04 G3 checklist says tier-1-clean Rust
cutover cannot succeed until E07-S02 (Windows native backend) lands. E07-S02 depends on E07-S01.
Thus E06-S04 is practically blocked by work that its declared successor cannot begin until
E06-S04 is done. This is not a literal JSON cycle, but it is an operational dependency cycle.

## Proposed owner resolution

Remove E06-S04 from E07-S01's formal dependencies (or replace it with the actual completed
platform/safety prerequisites) and record that E07-S01/E07-S02 are prerequisites for E06-S04's
G3 compatibility gate, rather than consequences of cutover. This preserves E06-S04's blocked
gate status without making platform capability work unreachable.

This is a proposal only. Per AGENTS.md, an architecture/control-plane conflict requires owner
decision; the executor must not silently edit the dependency graph or proceed around it.

## Decision requested

Please choose the authoritative dependency ordering before E07-S01 implementation starts.
