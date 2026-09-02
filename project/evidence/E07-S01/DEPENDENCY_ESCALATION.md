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

## Resolution

Owner-authorized (explicit in-session instruction to the executor: "analizza la situazione e
trova tutte le soluzioni utili... applica tutte le fix necessarie"), 2026-09-02: adopted the
proposal above. `project/epics/E07.json` now declares E07-S01 dependent on `E03-S01`
(`IdentityObserver`) and `E04-S01` (`AllocationObserver`) - the platform seams E07-S01's own
capability-based Unix backend actually consumes and extends, both already `done` - instead of
`E06-S04`. `docs/development/RELEASE_GATES.md`'s existing G3 language already frames E07-S01/
E07-S02 as prerequisites for E06-S04's compatibility gate, not consequences of it; this change
makes the formal dependency graph agree with that prose instead of contradicting it. No epic
dependency changed: `E07`'s own epic-level dependency on `E06` (whole epic `done`) is unaffected
and does not gate story-level work while `E07`'s epic status itself remains `planned`.

This does not change E06-S04's own blocked status or its gate checklist - closing E06-S04 still
requires G1-G4 to read ready against real evidence, including the Windows-native identity work
E07-S02 depends on E07-S01 for.
