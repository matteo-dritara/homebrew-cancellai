# Evidence Packet - E21-S03

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR4
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`, finding `CR-TE-01`; ADR-0018

## Outcome

PASS

## Scope

Makes an unobservable directory reduce authority in the Rust engine exactly as it does in the
frozen Python reference. This is the epic's blocking defect: on an ordinary tree - one directory
without read permission - the reference withheld every destructive action and exited `4` while
the target engine deleted and exited `0`, reporting the scope complete and its knowledge
verified.

Root cause, in both adapters: every failure to observe part of the tree was discarded by a bare
`else { continue }`. Codex had no completeness channel at all; Claude had one covering only a
session's companion payload directory, so the *project directory* branch beside it - reached
through a separate `read_dir` - was silently skipped. The Claude case was disclosed nowhere
before this review.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every listing, file-type and metadata failure is recorded with a named reason and a path, never discarded | `cancellai-provider-codex::session` and `cancellai-provider-claude::session`: each former `else { continue }` now pushes a `CompletenessReason` classified as `PermissionDenied`/`Disappeared`/`Io` by `io::ErrorKind`. Absence is deliberately *not* a reason, mirroring `cancellai.py::observe`, which records every `OSError` except `FileNotFoundError` - pinned by `a_missing_sessions_root_is_complete_not_partial` and `a_missing_projects_directory_is_complete_not_partial`. | PASS |
| AC2 - a scope with any such failure withholds every action for the tool and degrades knowledge_confidence for *every* artifact in it | `cancellai-policy::retention`: `describe()` derives the verdict from `ScopeCompleteness` for both providers; the `LowUnknown` degrade loops over all artifacts, not only the degraded one. `build_actions` returns an `Observe` action carrying the reason for every artifact in an incomplete scope. | PASS |
| AC3 - `error_count` reports the real number of unobservable paths | `ProviderResolution::scan_error_count()` counts the recorded reasons; the CLI's `ScanCompletenessDoc` uses it. Native run against the audit's own tree: `{"scope":"codex-cli","complete":false,"error_count":1}`, previously `{"complete":true,"error_count":0}`. | PASS |
| AC4 - clean, plan and clean --dry-run all report the withholding in the exit code | `any_incomplete` already drove the exit code; with the verdict now correct, the native reproduction below exits `4` on a real `clean --yes`. Preview and real run share `build_actions`, so they cannot disagree. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | An unreadable Codex session directory holding an eligible rollout | `an_unreadable_session_directory_makes_the_scope_partial`; `codex-partial-tree` through the parity gate; native reproduction below | PASS |
| SI-009 | An unreadable Claude *project* directory - the undisclosed branch | `an_unreadable_project_directory_makes_the_scope_partial`, which also asserts `degraded_companions` is **empty**, so it cannot pass via the E06-S02 channel | PASS |
| SI-009 | A provider simply not installed must NOT withhold | `a_missing_sessions_root_is_complete_not_partial`, `a_missing_projects_directory_is_complete_not_partial` - the counterexample that keeps the fix from degenerating into "always withhold" | PASS |
| SI-009 | An unreadable scope *root* is `Unknown`, not `Complete` | `an_unreadable_projects_root_is_unknown_not_a_clean_empty_scope` | PASS |
| SI-010 | Errors visible with cause and path | `CompletenessReason` carries both; `describe()` surfaces the first in the withholding reason | PASS |
| SI-014 | Safety-blocked is not success | Native run exits `4` and prints "Nothing was cleaned: safety withheld the requested work." | PASS |
| SI-008 | Both Codex roots feed one verdict, not just `sessions/` | `an_archived_sessions_failure_is_recorded_too` | PASS |
| — | A fully readable tree must still be `Complete` | `a_fully_readable_tree_is_complete`; every fixture-parity test now asserts `ScopeCompleteness::Complete` | PASS |

## Verification Commands

Native reproduction of the audit's own scenario, repaired engine:

```text
$HOME/.codex/sessions/2026/01/01/rollout-…-1111.jsonl   readable, mtime 2020
$HOME/.codex/sessions/locked/                            chmod 000
$HOME/.codex/sessions/locked/inner/rollout-…-2222.jsonl  not observable

$ cancellai-cli inspect --json --days 1 --keep-latest 0 --tool codex
  scan_completeness: [{"scope":"claude-code","complete":true,"error_count":0},
                      {"scope":"codex-cli","complete":false,"error_count":1}]

$ cancellai-cli clean --yes --days 1 --keep-latest 0 --tool codex
  Nothing was cleaned: safety withheld the requested work.        exit 4
  the readable rollout survives: yes
```

Before this story the same commands reported `complete: true, error_count: 0` and
`1 artifact(s) deleted, 89 bytes reclaimed.` at exit `0`.

The engine must still delete when it is lawful to - a fix that withholds unconditionally is not
a fix. On a fully readable tree, with the machine's live `codex` process overridden:

```text
$ cancellai-cli clean --yes --allow-running --days 1 --keep-latest 0 --tool codex
  1 artifact(s) deleted, 89 bytes reclaimed.                       exit 0
```

(Without `--allow-running` the same run exits blocked, because a real `codex` process was
running - the process guard, working.)

```text
python3 scripts/rust_python_parity.py check      12 NORMATIVE fixtures, both scenarios, OK
cargo test --workspace                           318 passed, 0 failed
cargo clippy --workspace --all-targets --all-features -- -D warnings    clean
python3 scripts/check_mutation_boundary.py check OK
```

## Compatibility

- No JSON schema change: `scan_completeness[].error_count` already existed and was already an
  integer; it now carries a truthful value. `docs/architecture/JSON_CONTRACTS.md` records that.
- The Claude capability surface (`ProjectAttribution`/`SessionGraph`) had the same defect in a
  second place - it reported full support unless a *companion* was degraded. It now keys off the
  scope's completeness, and drops to `LowUnknown` rather than `Observed`, since relationships
  derived from a partial scan are not proof of what is there.
- `docs/PROVIDERS.md`'s generated matrix is unchanged (`check_provider_compatibility.py` passes):
  the changed branch only fires on a partial scan, which the matrix does not generate.

## Performance / operability

- One `Vec<CompletenessReason>` per scope, pushed to only on failure. No additional syscalls: the
  errors were already being produced and thrown away.

## Documentation updated

- `docs/CLI_RUST.md`, `docs/development/RELEASE_GATES.md` (G2 rewritten),
  `docs/architecture/JSON_CONTRACTS.md`, `CHANGELOG.md`.

## Residual risks

- Both adapters still classify errors by `io::ErrorKind`, which is coarser on some platforms
  than the reference's `errno`. A failure that maps to `Other` still withholds - the
  classification affects only the message, never the verdict.
- `fs::metadata` failing during dir-vs-file classification is recorded as a reason, which is
  *stricter* than the reference's `os.walk` (it would treat the entry as a file and let later
  filters drop it). Fail-closed, and no corpus fixture reaches it, so it cannot mask a
  divergence today - recorded so a future fixture that does reach it is read correctly.
- This packet is executor self-assessment. CR4 requires an independent adversarial pass and an
  owner-visible Safety Verdict, neither of which exists yet.


## Round-1 independent review: FAIL, and its repair

`project/evidence/E21-S03/SAFETY_VERDICT.md` returned `FAIL` on four invariants. The findings
were correct and are repaired here.

**1. An unreadable Claude `projects/` root escaped withholding (SI-008/SI-009/SI-014, C-02).**
Discovery built `ScopeCompleteness::Unknown` correctly, and `resolve_claude` then threw it away:
it returned its `empty()` - `Complete` - resolution for *any* `Unavailable` scope. A native
mode-000 `projects/` exited `0` instead of `4`. This was my defect, and it is exactly the class
this epic exists to close: the completeness was computed and then discarded one layer up.

Repair: `SessionDiscoveryScope` now distinguishes `Unavailable` (absent or symlinked - a
structurally empty install) from `Unobservable` (exists, could not be read). `resolve_claude`
matches on all three variants explicitly and carries the observation through for the second.
Pinned by `an_unreadable_claude_projects_root_withholds_and_exits_four` (native CLI, asserts
exit 4 and that the session survives), `an_unreadable_claude_projects_root_is_reported_incomplete_with_a_real_count`,
and the counterexample `a_claude_home_without_projects_is_complete_not_withheld`.

**2. Nested Claude observation failures were collapsed or discarded (SI-010).**
`directory_size_and_latest_mtime` reduced every nested failure to `fully_read: bool`, so the
caller emitted one generic reason for the whole companion and each failing path was lost;
`metadata.modified()` used `.ok()`; a failing companion `symlink_metadata` sat inside
`if let Ok(...)`. Repair: the walker is now `walk_companion_payload`, returning
`Vec<CompletenessReason>` with a path and cause per failure; `modified()` and companion
`symlink_metadata` failures are recorded rather than swallowed (absent still records nothing).

**3. `Vec<CompletenessReason>` was unbounded on a hostile tree (C-11).**
Repair: `cancellai-inventory::ReasonLog` retains at most `MAX_RETAINED_REASONS` (64) while
counting every failure, and `ScopeObservation` carries the classification and the truthful total
as one value. `reason_retention_is_bounded_but_the_count_is_not` pins both halves; `describe()`
reports the total, never the retained count, so bounding retention cannot understate the scan.

## Post-review self-check: a residual of the same class, found and closed

After repairing the above I re-ran the verifier's reasoning one level higher and found the same
collapse in the layer above discovery: both resolvers gated on `root.exists()`, and
`Path::exists()` answers `false` for "not installed" *and* for "not allowed to look". With an
unreadable `$HOME`, the engine reported a clean empty scan and exited `0` where the reference
exits `4`. The gate is removed - discovery's own observation already distinguishes the two
cases - and pinned by `an_unreadable_home_withholds_rather_than_reporting_nothing_to_clean`
with `a_home_with_no_provider_installed_is_complete_and_exits_zero` as its counterexample.

This is recorded rather than quietly folded in, because it is evidence about the review: the
verifier's finding was one instance of a pattern, and the pattern had a second instance.

## Verifier verdict

`FAIL` (round 1) - repaired above; owner-accepted closure without a round 2, see project/evidence/E21-CLOSURE.md
