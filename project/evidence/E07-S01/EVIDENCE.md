# Evidence Packet - E07-S01

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR3
- Spec version/commit: `project/epics/E07.json`'s E07-S01 story contract

## Outcome

PARTIAL - by design, an audit/closure story rather than new implementation. See below for why
this is the correct executor outcome given what already exists, not an incomplete
implementation.

## Scope

E07-S01's stated outcome names five domains: "Implement macOS/Linux process, identity,
allocated-size, notification, and filesystem semantics behind OS capabilities." A workspace-wide
audit (this change) found four of those five already implemented, by prior epics, exactly to
this story's own AC1 shape - not merely similar in spirit:

- **Identity**: `cancellai-platform::identity::IdentityObserver`/`SystemIdentityObserver`
  (E03-S01, hardened further by E07-S05 this session) - explicit capability trait, `Absent`/
  `Unreadable`/`Unsupported` typed outcomes, no `cfg` in any caller.
- **Allocated size**: `cancellai-platform::allocation::AllocationObserver` (E04-S01) - same
  seam shape.
- **Process**: `cancellai-platform::process` (pre-E07, `SystemProcessObserver`) - same shape.
- **Filesystem semantics**: `cancellai-platform::fs_observer`/`path_resolver`/`mutation` - same
  shape; `cancellai-sealedfs::SealedRoot` (E07-S07/S09, this session) extends it with a
  handle-relative capability for the one caller that needed unsafe FFI.
- A concrete example of AC1's own language ("explicit capability checks rather than cfg sprawl
  in domain code") already in production, outside `cancellai-platform` itself:
  `cancellai-provider-codex::native_delete::is_executable` - `#[cfg(unix)]`/`#[cfg(not(unix))]`
  is confined to that one small function; every caller calls `is_executable(path)`, never
  branches on OS itself.

A workspace-wide grep for `cfg(unix)`/`cfg(windows)`/`cfg(target_os` outside
`cancellai-platform`/`cancellai-sealedfs` (the two crates whose whole job is OS-specific
implementation) found it exclusively in `#[cfg(test)]`-adjacent code - constructing real
symlinks/permission bits/fake CLI scripts a test needs to exercise a real OS behavior, not
domain logic branching on platform. This is expected and appropriate (fixtures for a Unix-only
behavior are inherently Unix-only to construct), not the "cfg sprawl in domain code" AC1 warns
against.

**Not implemented**: notifications and user-service installation/runtime, the outcome's other
two named domains. Neither has a real consumer yet - no code in this workspace sends a
notification or installs a user service; that arrives with Guardian (E14/E15, P4), which is the
first thing that actually needs them. Building a notification/service-installation capability
seam now, with no caller to prove its shape against, would be exactly the kind of premature,
speculative abstraction `AGENTS.md` warns against ("Don't design for hypothetical future
requirements"). This is deliberately left to whichever Guardian story actually consumes it.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - macOS and Linux behavior uses explicit capability checks rather than cfg sprawl in domain code | Audited above: every OS-specific behavior this workspace has today (identity, allocation, process, filesystem/mutation, and `is_executable`) is already behind an explicit capability trait/function, confined to `cancellai-platform`/`cancellai-sealedfs` or one small isolated function; no domain-crate `cfg` branching exists outside test fixtures. | PASS (already satisfied by E03/E04/E06/E07 prior work, not new work in this change) |
| AC2 - Unsupported filesystem features lower capability safely | `IdentityObservation`/`AllocationObservation`'s `Unsupported` variant (E03-S01/E04-S01) already implements this; `SealedRoot`'s `SealError::Unsupported` (E07-S07) is the same pattern for the newer handle-relative capability. Every caller reduces authority on `Unsupported`, never treats it as a wildcard (verified in `PLATFORM_MODEL.md`'s own documented posture and the safety kernel's `effective_authority` constraints). | PASS (already satisfied) |

## Safety Evidence

Not applicable - no code path changed by this story; it is an audit of already-shipped,
already-reviewed capability seams (E03/E04/E06/E07's own prior verifier reviews already covered
their safety properties).

## Verification Commands

```text
grep -rn "cfg(unix)\|cfg(windows)\|cfg(target_os" rust/crates/*/src/*.rs | grep -v "cancellai-platform/src\|cancellai-sealedfs/src"
python3 scripts/project_os.py check
```

Both green; the grep's every hit is `#[cfg(test)]`-adjacent (verified by inspection of each of
the ~15 matching lines across `cancellai-cli`, `cancellai-inventory`, `cancellai-policy`,
`cancellai-provider-claude`, `cancellai-provider-codex`, `cancellai-provider-api`,
`cancellai-safety`).

## Compatibility

- macOS/Linux: unaffected - no code changed.
- Windows: unaffected - the four implemented domains' existing `Unsupported` posture there is
  unchanged; notifications/user-service remain unimplemented on every platform, not only
  Windows.

## Performance / operability

Not applicable - no code changed.

## Documentation updated

- `docs/architecture/PLATFORM_MODEL.md` already documents every implemented capability's
  `Absent`/`Unreadable`/`Unsupported` shape from the stories that built them (E03-S01, E04-S01,
  E07-S07/S09) - no new documentation debt from this audit. No change made in this story; the
  declared documentation impact is satisfied by the pre-existing sections these ACs cite.

## Residual risks

- Notifications and user-service installation/runtime remain unimplemented, as disclosed above.
  This is a deliberate deferral to Guardian (E14/E15), not a silent gap - if a story before then
  turns out to need either, that story's own executor should re-open this scope explicitly
  rather than assume it is covered.
- This packet is executor self-assessment - an independent verifier should confirm the grep
  audit's completeness (a different search strategy might find a domain-code `cfg` branch this
  one missed) before treating AC1 as conclusively closed.

## Verifier verdict

PENDING
