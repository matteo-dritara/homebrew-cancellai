# Evidence Packet - E18-S01

- Commit/PR: local checkpoint on `main`, `rust/crates/cancellai-model/src/remote_target.rs` +
  `lib.rs` + `docs/architecture/TARGET.md` + `CHANGELOG.md` + `project/epics/E18.json`
- Executor: Claude
- Independent verifier: Codex - round 1 PASS (`project/evidence/E18-VERIFIER-REVIEW.md`); not
  re-reviewed in rounds 2/3, which were scoped to E18-S02/S03 only
- Change Risk: CR3 (declared CR2 at planning; raised at commit time - see
  `project/epics/E18.json`'s `risk_reclassification_note` and
  `docs/architecture/TARGET.md`'s "Remote target vocabulary (E18-S01)")
- Spec version/commit: `project/epics/E18.json` as committed in this checkpoint

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Text (verbatim, `project/epics/E18.json`) | Evidence | Result |
| --- | --- | --- | --- |
| AC1 | "Remote inventory never masquerades as local state." | `InventoryOrigin` is a two-variant enum; `Local` carries no `MachineId`, so no remote id can compare equal to it. `remote_target::tests::ac1_no_machine_id_however_spelled_ever_compares_equal_to_local` (adversarial spellings `""`, `"local"`, `"LOCAL"`, `"localhost"`, `"this-machine"`) and `ac1_a_remote_targets_own_origin_is_never_local` (`RemoteTarget::origin` is the only producer of `InventoryOrigin` for a target, and always returns `Remote`) | PASS |
| AC2 | "Disconnected targets preserve last-seen state as stale, not current." | `RemoteTarget::state_is_current` is `false` for anything but `Connected`. `remote_target::tests::a_fresh_target_defaults_to_untrusted_and_disconnected` (fresh target starts stale) and `mock_remote_lifecycle_connect_disconnect_reconnect` (connect -> disconnect -> reconnect, staleness tracks connection exactly, not a timer) | PASS |

## Safety Evidence

No `safety_obligations` are declared on this story (pure vocabulary, no mutation/authority
wiring). One constitutional touch was found and closed during the adversarial-cases pass, not
because the story named it:

| Concern | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| C-05 (capability/trust are authority inputs) - a bare, deserializable trust/capability value reaching an authority computation with no promotion gate, the exact E05 round-1 defect (`cancellai_safety::trust_promotion`'s module doc) | Would a JSON document alone construct `RemoteTargetTrust::Trusted` or `can_execute_mutations: true`? | `RemoteTargetTrust`/`RemoteCapabilities`/`RemoteTarget` derive `Serialize` only, not `Deserialize` - the type cannot be built from external data at all, a compile-time guarantee (see "Method defects" for why this is not independently runtime-testable) | PASS by construction |

## Verification Commands

```text
$ cargo test -p cancellai-model remote_target
running 8 tests
test remote_target::tests::a_fresh_target_defaults_to_untrusted_and_disconnected ... ok
test remote_target::tests::ac1_a_remote_targets_own_origin_is_never_local ... ok
test remote_target::tests::ac1_no_machine_id_however_spelled_ever_compares_equal_to_local ... ok
test remote_target::tests::capabilities_and_trust_are_independent_per_instance_not_inferred_from_kind ... ok
test remote_target::tests::machine_id_round_trips_through_json ... ok
test remote_target::tests::machine_id_survives_unicode_and_control_characters ... ok
test remote_target::tests::mock_remote_lifecycle_connect_disconnect_reconnect ... ok
test remote_target::tests::remote_target_trust_defaults_to_untrusted ... ok
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out

$ cargo test --workspace
(every crate) test result: ok ... 0 failed  (workspace total, no regression)

$ cargo fmt --check
(clean, no output)

$ cargo clippy --workspace --all-targets --all-features -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.03s   (no warnings)

$ cargo deny check
advisories ok, bans ok, licenses ok, sources ok

$ python3 scripts/project_os.py check
governance OK: 24 decisions, 32 epics, 165 stories

$ cargo check --workspace --all-targets
Finished `dev` profile [unoptimized + debuginfo] target(s)

$ python3 scripts/check_schemas.py check
schemas OK: 4 golden documents match docs/architecture/JSON_CONTRACTS.md

$ python3 scripts/check_fixtures.py check
fixtures OK: 13 fixtures cover all required categories

$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 86 Rust source files scanned; only
rust/crates/cancellai-platform/src/mutation.rs deletes anything, only
rust/crates/cancellai-platform/src/mutation.rs, rust/crates/cancellai-safety/src/mutation_executor.rs
reference the capability that does
```

Not run in this session (unavailable/not required for this change - CI runs the full matrix):
cross-platform clippy (`--target x86_64-pc-windows-gnu` / `-unknown-linux-gnu`) - not required per
`AGENTS.md`'s own rule ("when a change touches `cancellai-platform`, or anything moves the lint
surface workspace-wide"); this change touches neither. `cargo +nightly miri` - weekly-only per
`AGENTS.md`, not per-PR. Python checks (`ruff`/`mypy`/etc.) - no Python files touched.

## Compatibility

- Pure Rust domain types, no OS/filesystem/platform interaction. No provider/schema surface
  touched. No CLI/TUI/Guardian wiring - library-level vocabulary only.

## Performance / operability

- No bulk/hot-path code; not applicable at this vocabulary layer.

## Documentation updated

- `docs/architecture/TARGET.md`: new "Remote target vocabulary (E18-S01)" subsection.
- `CHANGELOG.md`: `[Unreleased] > Added` entry.
- `project/epics/E18.json`: `change_risk` CR2 -> CR3 with `risk_reclassification_note`.

## Method defects

- **What happened**: before recommending this story as startable, I checked only `E18-S01`'s own story-level `dependencies` (`E16-S02`, satisfied) and missed `project/epics/E18.json`'s separate *epic-level* `dependencies` (`E16`, `E17`), which are not both closed (`E16` `blocked`, `E17` `planned` with `E17-S07` `blocked`) - `python3 scripts/project_os.py generate` caught it mechanically the moment I also advanced the epic's own status to `in_progress`. **Prevented by**: the gate itself worked; no document told me to check the epic-level field first, only the story-level one - `docs/development/WORK_ITEM_MODEL.md`'s dependency section covers the story-level rule but not this separate epic-level gate. **Disposition**: proposed 2026-09-18 - the immediate blocker was resolved in this session by matching the existing `E17` precedent (an epic stays `planned` while individual stories inside it proceed); whether `docs/development/WORK_ITEM_MODEL.md` should name the epic-level gate as a second, independent check is an owner call, not made here.
- none otherwise.

## Residual risks

- `RemoteTargetTrust`/`RemoteCapabilities` not implementing `Deserialize` is a compile-time
  guarantee (absence of a derive), not independently verifiable by a runtime test - Rust has no
  stable negative-trait-bound assertion. The guarantee is real (any code attempting to deserialize
  either type fails to compile) but the evidence for it is code review of the derive list, not a
  test artifact. A future reviewer should re-check the derive list directly rather than trust a
  test result for this specific claim.
- No concurrency primitive exists on `RemoteTarget` (plain `&mut self` transitions) - a future
  story that shares one `RemoteTarget` across multiple probes/threads must decide its own
  synchronization; nothing here anticipates or constrains that choice.
- `MachineId` is not yet wired onto `AgentArtifact`/the inventory pipeline (deliberately, per
  `docs/architecture/TARGET.md`'s own note that this remains true after this story) - `by_machine`
  still returns exactly one bucket. A later story owns that integration and will face real design
  questions this one does not (e.g., how a remote-origin fact reaches `CurrentStateStore` without
  crossing `cancellai-store`'s existing "never the source of truth" contract).
- This change has not yet been exercised by CI - it is a local, uncommitted-to-`origin` checkpoint
  at evidence time. `python3 scripts/project_os.py check`/`generate`, `cargo fmt --check`,
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`,
  and `cargo deny check` all ran locally on macOS only in this session.

## Verifier verdict

Round 1 (Codex, 2026-09-19): PASS, no residuals - `InventoryOrigin::Local` cannot compare equal
to a remote origin (even empty/`local`/`localhost`/Unicode/control-character ids), and
`state_is_current()` treats anything but `Connected` as stale. Owner accepted 2026-09-19 (see
E18-S02/S03's own `SAFETY_VERDICT.md` for the epic-scope acceptance this story's PASS was bundled
into for status purposes; this story is CR3 and does not require its own `SAFETY_VERDICT.md`).
