# Evidence Packet - E15-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending (Codex, per-epic review once every E15 story is `ready_for_review`)
- Change Risk: CR3
- Spec version/commit: `project/epics/E15.json`'s E15-S04 story contract

## Outcome

PASS (executor self-assessment; independent verification pending). Implements
`cancellai_guardian::killswitch` (an immediate local disable path) and `cancellai_guardian::audit`
(linking every Guardian plan decision to detection evidence, policy decision, and a plan
reference, via `cancellai_store::EventLedger`). This is E15's last story - once independent
review passes for the whole epic, closing it cuts a release (ADR-0025).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "Kill-switch stops future autonomous mutations without corrupting current state." | `apply_kill_switch` (`rust/crates/cancellai-guardian/src/killswitch.rs`) is a pure, in-memory `Vec` transform with no I/O - `engaged_switch_forces_every_item_to_observe` proves every item's `action_class`/`granted_authority` is forced to `Observe` when engaged; `engaged_switch_still_returns_every_item_read_only_inspection_is_not_withheld` proves the item count is unchanged (inspection is not withheld, only actionability). "Does not corrupt current state" holds structurally: this crate performs no mutation/execution at all (confirmed by `scripts/check_mutation_boundary.py check`, unchanged), so there is no in-flight destructive action this module could ever interrupt - `cancellai_safety::mutation_executor`'s own existing transactional/TOCTOU-safe sequence is the only thing that ever touches real state, and nothing here calls into it. "Immediate local" is verified by `a_never_engaged_switch_reads_as_disengaged`/`engage_then_is_engaged_reads_true`/`engage_then_disengage_round_trips_to_false` against a real local file, no external process. | PASS |
| AC2 "Every action links detection evidence, policy decision, and sealed plan." | `record_guardian_decision` (`rust/crates/cancellai-guardian/src/audit.rs`) builds a `cancellai_store::ledger::NewEvent` whose `mutation: Some(MutationReference { plan_id, evidence_ids })` links the plan reference and detection evidence, and whose `EventMetadata` (`policy_id` = `binding_constraints`, `reason_code` = pressure/granted-authority) links the policy decision - `an_actionable_item_is_recorded_as_plan_created_with_plan_and_evidence_linked` reads the appended event back from a real, in-memory `EventLedger` and asserts all three are present and correct; `the_policy_decision_is_recoverable_from_the_recorded_metadata` asserts the policy fields specifically. `empty_evidence_ids_is_refused_by_the_ledger_itself_not_silently_accepted` proves this function cannot bypass `EventLedger::append`'s own enforcement of that link (`docs/architecture/PERSISTENCE_MODEL.md`'s "Mutation events carry plan/evidence references") - it is the same `append` every other mutation-class writer in this workspace uses. | PASS |

## Verification Contract Evidence

| Verification item | Evidence | Result |
| --- | --- | --- |
| "Kill during planning/execution fault tests." | **Kill during planning**: `engaged_switch_forces_every_item_to_observe`/`disengaged_switch_leaves_the_plan_unchanged` exercise the kill switch applied to an already-computed plan, both engaged and disengaged. **Fault tests** (the switch's own I/O failure modes, since a synchronous, single-call planning function has no meaningful "mid-planning" interruption point to simulate): `an_unreadable_marker_directory_reads_as_engaged_not_disengaged` (a real, unreadable path - fails safe to engaged); `engage_failure_is_surfaced_and_never_reads_back_as_a_confirmed_disengage` (a real write failure - surfaced as `Err`, and the unresolved read afterward still fails safe to engaged, never a false "confirmed off"). **Kill during execution**: not applicable at this stage and disclosed as such below - this story's own scope, and this crate's, contains no execution path at all (`scripts/check_mutation_boundary.py check`, unchanged); "any in-flight destructive action still follows safety executor transaction semantics" (GUARDIAN_MODEL.md's own text) is an existing, pre-this-story guarantee of `cancellai_safety::mutation_executor`, verified by that crate's own tests (E03/E14-S05), not something this story could newly break or newly need to re-verify, since nothing here calls it. | PASS |

## Safety Evidence

The story names no `safety_obligations`. Adversarial cases considered for this CR3 change:

| Case | Expected under the contract | Test |
| --- | --- | --- |
| Kill-switch marker present but unreadable (permission denied) | Fail-safe: reads as engaged, never disengaged | `an_unreadable_marker_directory_reads_as_engaged_not_disengaged` |
| Kill-switch `engage()` I/O failure (cannot write) | Surfaced as `Err`, not silently swallowed; and the switch does not read back as a false "confirmed disengaged" | `engage_failure_is_surfaced_and_never_reads_back_as_a_confirmed_disengage` |
| Kill-switch never engaged at all | Reads as disengaged (the honest default, not a fail-safe guess) | `a_never_engaged_switch_reads_as_disengaged` |
| Double-engage / double-disengage | Idempotent, no error | `engage_is_idempotent`; `disengage_when_never_engaged_is_not_an_error` |
| Second mutation-authority/deletion path (`docs/CONSTITUTION.md`) | None introduced - `disengage()` never calls `std::fs::remove_file`/`remove_dir`/`remove_dir_all`, or `SystemMutationExecutor`/`.mutate(` | `scripts/check_mutation_boundary.py check` (see Method defects for how this was found and repaired during this story) |
| Ledger accepts a mutation-class event with no evidence | Refused at the database layer, no partial write | `empty_evidence_ids_is_refused_by_the_ledger_itself_not_silently_accepted` |
| Audit content privacy (`docs/architecture/PERSISTENCE_MODEL.md`'s "contentless by default") | `record_guardian_decision` writes only the closed `EventMetadata` fields (`artifact_id`, `category`, `policy_id`, `reason_code`) already allowlisted by `EventLedger` itself - no path, transcript, or artifact content is available to it in the first place, since `GuardianPlanItem` (E15-S03) never carries any | By construction - `GuardianPlanItem`'s own fields (inspected: `artifact_id`, `action_class`, `granted_authority`, `pressure_state`, `binding_constraints`) contain no path/content field to leak |

## Verification Commands

```text
$ cd rust && cargo fmt --check
(clean)

$ cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in ...
(no warnings)

$ cd rust && cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s

$ cd rust && cargo test --workspace
... every crate: test result: ok. <N> passed; 0 failed ...
cancellai-guardian: test result: ok. 145 passed; 0 failed; 0 ignored
  (includes killswitch::tests::* [10 new], audit::tests::* [5 new])

$ cd rust && cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 104 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs
deletes anything, only rust/crates/cancellai-platform/src/mutation.rs,
rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does

$ python3 scripts/check_docs.py check
docs OK: 478 Markdown files; local links and safety IDs are consistent

$ python3 scripts/gen_docs.py --check
docs/CLI.md is up to date.
```

### Not run locally (unavailable tooling)

- Cross-target clippy (`--target x86_64-unknown-linux-gnu` / `--target x86_64-pc-windows-gnu`):
  same pre-existing local gap as E15-S01/S02/S03's own evidence packets (missing C cross-compiler
  for `cancellai-store`'s bundled `rusqlite`). Moot for `killswitch.rs`'s own code (no
  `cfg(target_os)` branch at all); `audit.rs` exercises `cancellai-store` directly, which is
  already covered by that crate's own cross-platform CI. CI's per-platform native runners run
  this before merge.
- `mypy`/`ruff`/the Python reference test suite: not run - this story touches only `rust/`,
  `docs/`, `CHANGELOG.md` and `project/`; `cancellai.py` is untouched.

## Compatibility

- No provider/schema surface touched. `killswitch.rs` has no platform-specific code; `audit.rs`
  depends only on `cancellai-store`'s already cross-platform-verified `EventLedger`.

## Performance / operability

- `KillSwitch::is_engaged` is one local file read; `engage`/`disengage` are one local file write
  each - no retry, no blocking wait beyond that one syscall. `record_guardian_decision` is one
  `EventLedger::append` call (already covered by that crate's own performance characterization).

## Documentation updated

- `docs/architecture/GUARDIAN_MODEL.md`: added the E15-S04 paragraphs to the existing "Kill
  switch" and "Audit" sections, following the established per-story documentation pattern.
- `CHANGELOG.md`: added an `### Added` entry under `## [Unreleased]`.

## Method defects

- **What happened**: the first version of `KillSwitch::disengage()` called `std::fs::remove_file` to remove the marker on disengage; `python3 scripts/check_mutation_boundary.py check` correctly refused this (`rust/crates/cancellai-guardian/src/killswitch.rs:74: direct filesystem mutation outside the safety executor: std::fs::remove_file (SI-019)`) - a real, structural finding this session ran into and repaired before committing, not a hypothetical. **Prevented by**: `scripts/check_mutation_boundary.py check`, run as part of this story's own CR3 gate set (`risk-gate` skill) before committing - the gate did exactly its job. **Disposition**: accepted 2026-09-22 - repaired in this same commit: `disengage()` now writes a `"disengaged"` content marker instead of removing the file, so the switch's state lives in the marker's content rather than its existence, and no `std::fs::remove_file`/`remove_dir_all`/`remove_dir` call exists anywhere in this crate; re-ran the full gate set after the repair, all green.

## Residual risks

- No caller wires `KillSwitch`/`record_guardian_decision` to a live orchestrator yet - the
  production location for the marker file, and the production `EventLedger`/clock the audit
  function would write through, are not chosen here (this story provides the mechanism, not the
  wiring); matches every other module in this epic's "primitive delivered, no orchestrator yet"
  shape.
- `plan_id` passed to `record_guardian_decision` is not yet a real `cancellai_safety::SealedPlan`
  identifier, since E15-S03's planner does not seal plans - disclosed in
  `docs/architecture/GUARDIAN_MODEL.md`'s "Audit" section update and in `audit.rs`'s own module
  doc. A future orchestrator that does seal plans should pass the real `SealedPlan`'s identifier
  through unchanged; nothing in this module's shape needs to change for that.
- "Kill during execution" fault testing is out of this story's reach, honestly: neither this
  crate nor this story's own scope performs any execution to interrupt. The property GUARDIAN_
  MODEL.md's text asks for ("any in-flight destructive action still follows safety executor
  transaction semantics rather than being killed mid-syscall unsafely") is an existing guarantee
  of `cancellai_safety::mutation_executor`, already covered by that crate's own test suite; this
  story adds no new interruption surface to it.
- This closes every planned story in E15 (`S01`-`S04`); per-epic independent review is the next
  step once all four are `ready_for_review` (they now are), per `docs/development/AGENT_
  PROTOCOL.md`'s "Review is per epic" rule - not something this executor triggers itself.

## Verifier verdict

(pending - independent review runs at epic scope, all four E15 stories now `ready_for_review`)
