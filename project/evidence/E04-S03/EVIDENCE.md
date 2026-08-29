# Evidence Packet - E04-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped review of E04)
- Change Risk: CR3
- Spec version/commit: `rust/crates/cancellai-inventory/src/completeness.rs` as added in this change

## Outcome

PASS

## Scope

`derive_completeness` classifies an `InventorySnapshot` (E04-S02) as `Complete`, `Partial`,
or `Unknown` from evidence already collected by that one traversal: the scope root's own
fact, every directory-listing error, and every per-file `FactConfidence::Partial` reason.
`ScopeCompleteness::Unknown` is reserved for the scope root itself being unobservable
(absent/unreadable) - the strongest form of missing evidence this model expresses; anything
less severe (a readable root with an unreadable/permission-denied/vanished descendant, or a
descendant whose identity/allocation could not be established) is `Partial` with every
reason named (SI-010). `planning_view` is the only public way to obtain planning-facing
candidates: it returns a `PlanningView` bundling `candidates` and `completeness` in one
struct with no bare-candidates constructor, so a caller cannot reach candidates without also
receiving the completeness they were produced under.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - Every inventory scope is COMPLETE, PARTIAL, or UNKNOWN with reasons | `ac1_a_fully_readable_tree_is_complete`, `ac1_a_nonexistent_scope_root_is_unknown_not_complete_or_silently_empty`, `ac1_nested_permission_fixture_is_partial_with_a_permission_reason` (a real `chmod 000` fixture nested two levels deep, per the story's own verification contract "Permission and race fixtures across nested directories"), and `ac1_unsupported_identity_on_a_present_file_contributes_a_partial_reason` each drive one of the three states and assert the concrete `CompletenessReason` variant produced, not just a generic non-`Complete` result. `a_disappeared_directory_is_classified_distinctly_from_permission_denied` proves `Disappeared` and `PermissionDenied` are distinct variants, not collapsed into one generic "directory error." | PASS |
| AC2 - Planning cannot erase completeness information | `ac2_planning_view_always_carries_completeness_alongside_candidates` and `ac2_a_degraded_scope_planning_view_still_reports_partial_not_complete` (a real permission fixture, asserting the *view itself* - not just the underlying snapshot - reports non-`Complete`). Structurally: `PlanningView`'s only public constructor is `planning_view`, which always derives `completeness` from the same snapshot as `candidates` - there is no code path in this crate that returns `Vec<&FileFacts>` for planning purposes without `ScopeCompleteness` attached in the same value. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 (partial scan is non-destructive) | Nested `chmod 000` subdirectory beneath an otherwise-readable scope | `ac1_nested_permission_fixture_is_partial_with_a_permission_reason` / `ac2_a_degraded_scope_planning_view_still_reports_partial_not_complete` - the scope is classified `Partial`, and a planning caller consuming `planning_view` cannot observe a `Complete` result for this scope by construction. | PASS |
| SI-009 (unknown scan state is non-destructive) | Scope root pointed at a nonexistent path | `ac1_a_nonexistent_scope_root_is_unknown_not_complete_or_silently_empty` - classified `Unknown` with a `ScopeRootUnavailable` reason, never silently treated as an empty-but-complete scope. | PASS |
| SI-010 (scan errors are visible) | Every reason-producing path above | Every `CompletenessReason` variant carries the concrete path and cause; `reasons` is never truncated/deduplicated/summarized to a count (the module doc comment states this explicitly as a deliberate choice, since deduping risks hiding evidence per C-06). | PASS |

## Verification Commands

```text
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-targets
cargo test -p cancellai-inventory
cargo deny check
python3 scripts/check_docs.py check
python3 scripts/project_os.py check
```

`cargo test -p cancellai-inventory` runs 23 unit tests (15 from E04-S01/S02 + 8 new
`completeness` tests) plus 2 golden tests, all green on first run. Every `chmod 000` fixture
restores permissions immediately after the scan (before the temp-tree `Drop` runs
`remove_dir_all`), so no test leaves an unreadable directory behind.

## Compatibility

- The nested-permission fixtures are `#[cfg(unix)]`-gated (as `chmod`/`std::fs::Permissions`
  mode bits are Unix-specific); the non-permission tests (root-unavailable, complete tree,
  disappearance classification, idempotency, planning-view bundling) run on every platform.

## Performance / operability

- `derive_completeness`/`planning_view` are pure reads over an already-built
  `InventorySnapshot` - no additional filesystem access, consistent with E04-S02's "one
  traversal per scope."

## Documentation updated

- `docs/security/SAFETY_INVARIANTS.md` - SI-008 and SI-009 each gained an implementation
  pointer to `completeness.rs`, including an explicit note on what is *not* yet wired
  (`ScopeCompleteness` is not yet connected to `KnowledgeConfidence`/`authority.rs`'s
  existing collapse logic - that connection requires a classification stage, E05/E06) - the
  story's declared documentation impact.
- `docs/architecture/DOMAIN_MODEL.md` - completeness section added alongside E04-S01/S02's
  (documentation impact expanded for narrative continuity, same rationale as E04-S02's
  evidence packet).

## Residual risks

- A true listing-to-read disappearance race (a directory vanishing between a parent's
  `read_dir` entry and this scan's own `read_dir`/`observe` call on it) is not exercised
  end-to-end against a real filesystem - constructing that race reliably in a sandboxed test
  is impractical (the same limitation `cancellai-platform::identity`'s TOCTOU tests document
  for the mount-boundary case). `a_disappeared_directory_is_classified_distinctly_from_permission_denied`
  exercises the classification directly against a synthesized `NotFound` error instead,
  proving the *mapping* is correct without proving the *race* is reliably observed.
- `ScopeCompleteness::Partial`/`Unknown` is not yet wired into `cancellai-safety`'s
  `effective_authority` (E03-S04) or `KnowledgeConfidence` - no classification stage exists
  yet to make that connection meaningfully (same residual pattern E03-S04's own evidence
  packet recorded for its own deferred inputs). This is the concrete next step once E05/E06
  exist, not a gap this story silently leaves unstated.
- `CompletenessReason::Io`/`Unreadable` messages are opaque platform strings
  (`std::io::Error::to_string()`) where a more specific `ErrorKind` was not directly
  available at the call site (per-file `Unreadable` from `FsObserver`, as opposed to this
  crate's own `read_dir` calls where `ErrorKind` *is* available and used) - documented in
  the module doc comment, not silently inconsistent.

## Verifier verdict

PENDING - epic E04 review runs once every story in E04 is `ready_for_review` (at most twice per epic, per ADR-0014).
