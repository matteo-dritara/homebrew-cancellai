# Evidence Packet - E08-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E08 epic review round 1
- Change Risk: CR2
- Spec version/commit: `docs/architecture/DOMAIN_MODEL.md` "`activity_signal` /
  `ActivityState::Orphaned` (E08-S03)"

## Outcome

PASS

## Scope

`ActivityState::Orphaned` existed in `cancellai-model::vocabulary` since E03-S02 but had no
producer - `cancellai-policy::retention::classify` only ever derived `Active`/`Idle`/`Stale`/
`Unknown`. This story:

1. Gives `Orphaned` a real producer: a Codex session whose `parent_session_id` names a parent
   this scan did not discover (a dangling reference), distinguished from "genuinely has no
   parent" - both previously collapsed to "no `ArtifactRelationship`" in E08-S01's own
   resolution, which this story's `resolve_codex` now also captures as `orphan_evidence: Option<
   String>` (the missing parent id) alongside `relationships`.
2. Adds `AgentArtifact::activity_signal: Option<ActivitySignal>` (`{ evidence_ids, explanation }`)
   - `Some` exactly when `activity_state` is `Orphaned` or `Stale`, naming the concrete evidence
     (missing parent id, or observed mtime + cutoff) rather than leaving a reader to infer it from
     the bare enum value.
3. Applies safety-first precedence: an unresolved parent only ever overrides what would otherwise
   be `Idle`/`Stale` - never `Active` or `Unknown` - because `cancellai_safety::authority::
   lifecycle_ceiling` already caps authority specifically for those two values and treats
   `Idle`/`Stale`/`Orphaned` identically. Overwriting `Active`/`Unknown` with `Orphaned` would have
   been a real authority-protection regression; this story does not touch `cancellai-safety` at
   all, so the story's own outcome ("without directly implying deletion eligibility") holds by
   construction.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Orphan state and protection state coexist | `a_parent_thread_id_that_was_not_itself_discovered_produces_no_relationship` (extended): its only fixture session is both `keep_latest`-pinned and has a dangling parent, and now asserts `activity_state == Orphaned` *and* `protection_state == Pinned` together. No new mechanism needed - `ActivityState`/`ProtectionState` were already independent fields. | PASS |
| AC2 - Signal explanation identifies evidence and thresholds | `a_stale_sessions_activity_signal_names_the_mtime_and_the_cutoff` (Stale: names mtime `0` and cutoff `259200`); the extended orphan test above asserts the explanation names the concrete missing parent id `00000000-0000-4000-8000-000000000000`. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| No new safety obligation is listed for this story (`safety_obligations: []`), but the design has a real regression risk: overwriting `Active`/`Unknown` with `Orphaned` would silently remove `lifecycle_ceiling`'s existing authority cap for those values. | A session with an unresolved parent whose provider process is currently running. | `a_running_process_keeps_active_state_even_with_an_unresolved_parent` (new test) - asserts `activity_state == Active` and `activity_signal == None`, i.e. orphan evidence never reaches the `Active` branch at all. | PASS |
| Same regression class, the `Unknown` value | Not independently tested with a dedicated fixture - `classify`'s own `match mtime { ... None => ActivityState::Unknown }` arm is structurally unreachable from the orphan-override branches (both `Some(t) if ...` and `Some(_)` arms are guarded on `mtime` being `Some`), so `orphan_evidence` cannot influence the `None => Unknown` arm at all; this is enforced by the match's own exhaustiveness, not by runtime behavior a test could falsify. | Code inspection: `retention.rs`'s `activity` match. | PASS |

## Verification Commands

```text
$ cd rust
$ cargo fmt --check
$ cargo clippy --workspace --all-targets --all-features -- -D warnings
$ cargo check --workspace --all-targets
$ cargo test --workspace          # cancellai-policy: 24 passed (3 new for this story, 1 extended);
                                   # cancellai-model: 15 passed (2 new); 0 failed anywhere
$ cargo deny check                # advisories ok, bans ok, licenses ok, sources ok
$ cd ..
$ pre-commit run --all-files
```

New/extended tests:

- `rust/crates/cancellai-model/src/agent_artifact.rs`:
  `no_activity_signal_serializes_as_an_explicit_null_not_an_omitted_key`,
  `an_activity_signal_serializes_its_evidence_ids_and_explanation`.
- `rust/crates/cancellai-policy/src/retention.rs`:
  `a_parent_thread_id_that_was_not_itself_discovered_produces_no_relationship` (extended with
  orphan/protection/explanation assertions - AC1, AC2),
  `a_running_process_keeps_active_state_even_with_an_unresolved_parent` (new - safety precedence),
  `a_stale_sessions_activity_signal_names_the_mtime_and_the_cutoff` (new - AC2, Stale half).

## Compatibility

- Wire format: `activity_signal` is a new, additive-only key in the `--json` inventory document's
  `artifacts[]` entries (explicit `null` when absent, never an omitted key, matching
  `relationships`/`project_attribution`'s own precedent). Schema/parity checks unaffected for the
  same reasons documented in E08-S01/E08-S02's evidence packets.
- Providers exercised: `codex-cli` (Orphaned derivation, Active-precedence), `claude-code` (Stale
  explanation content; Claude can never be Orphaned by this rule).

## Performance / operability

- No new I/O: `orphan_evidence` reuses the `by_session_id` lookup E08-S01 already built for
  `relationships`; no additional pass over discovered sessions.

## Documentation updated

- `docs/architecture/DOMAIN_MODEL.md`: new "`activity_signal` / `ActivityState::Orphaned`
  (E08-S03)" subsection under "AgentArtifact".
- `CHANGELOG.md` Unreleased/Added.

## Residual risks

- Only Codex's parent-reference chain is a real orphan producer today. A Claude-side orphan
  signal (e.g. a project directory evidenced as removed) needs the path-existence checking
  E08-S02's evidence packet already deferred, and is not invented here ahead of that evidence.

## Verifier verdict

PASS | PASS_WITH_RESIDUALS | FAIL
