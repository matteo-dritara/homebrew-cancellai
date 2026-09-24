# Safety Verdict - E33-S03

## Verdict

The latest round below is authoritative.

## Round 10 — 2026-09-24

Verifier: Codex
Brief-Checksum: 06f4b8e341065cb1b07f471d44b7fe59f0964ded2c3c6aa937c285f4c6365e92
Change: containment history stored in one transactional SQLite event table
Risk: CR4
Commit/PR: `8ea967e..0afe3ad`

### Safety surface changed

Containment install, feed refresh and local lift decide from the event rows and
append at most one raw event inside one `BEGIN IMMEDIATE` transaction.

### Invariants and adversarial cases

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Persistence stores inert raw signed text; kernel replay remains the authority. | `containment_state::read_rows` returns opaque events, and CLI replay calls `cancellai-safety`; store has no safety dependency. | PASS |
| SI-029 / E33-S03 AC1–AC4 | Refused, concurrent or interrupted events add no row; unreadable history caps authority. | Workspace Rust tests pass; 16 stable-channel CLI cases include concurrent refresh/install, kill-before-commit, and corrupt-history authority cap. Existing foreign database lacking `events` is refused. | PASS |

The ledger remains editable by the owner's own account; ADR-0039 explicitly
accepts that limit. Main CI was unavailable here. A follow-on E33-S01 review
must judge feed behavior before its blocked status is cleared.

### Recovery

An interrupted SQLite transaction rolls back; a corrupt history remains
unreadable and caps authority until the owner repairs or removes local state.

### Owner decision

Pending for E33 epic closure.

PASS
