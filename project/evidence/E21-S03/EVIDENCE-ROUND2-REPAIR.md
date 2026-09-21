# Evidence Packet - E21-S03 (round-2 repair)

- Commit/PR: repairs `667ce50c8944603b2990b38c754423691eed8f61`, the commit
  `project/evidence/E21-S03-S07-INDEPENDENT-REVIEW-ROUND2.md` reviewed and found `FAIL`
- Executor: Claude
- Independent verifier: Codex (pending re-run against this repair)
- Change Risk: CR4
- Spec version/commit: `project/epics/E21.json` (E21-S03), `docs/audits/2026-09-03-CODE_REVIEW.md`
  (CR-TE-01)

## Outcome

PARTIAL (repair complete, awaiting independent reconfirmation per this repository's
executor/verifier separation - the executor that makes a CR4 repair may not close it)

## Context

This is the second finding recorded against E21-S03 after the epic's original closure. E21 closed
2026-09-03 on an owner decision to spend only round 1 of independent review
(`project/evidence/E21-CLOSURE.md`), explicitly disclosing "no independent confirmation of the
round-1 repairs" as a residual risk. `E06-S04`'s control-plane blocker
(`project/epics/E06.json`) named exactly this gap as required work. A round-2 independent review
(`project/evidence/E21-S03-S07-INDEPENDENT-REVIEW-ROUND2.md`, Codex, 2026-09-21) reproduced a real,
new defect in E21-S03 that round 1 did not find: a `projects` root that **exists and is a regular
file** (not absent, not a symlink) fell into the same "structurally empty install" branch as a
missing root.

## Reproduction (before repair)

```text
$ HOME=<scratch> rust/target/debug/cancellai-cli clean --yes --allow-running --days 1 \
    --keep-latest 0 --tool claude
Nothing to clean: no artifact is both stale and unblocked.
exit 0

$ HOME=<scratch> python3 cancellai.py clean --yes --allow-running --days 1 --keep-latest 0 \
    --tool claude
SCAN INCOMPLETE: 1 unreadable path(s) in claude
  unreadable: <scratch>/.claude/projects: Not a directory
Nothing was cleaned: safety withheld the requested work.
exit 4
```

Independently reproduced by the executor before touching any code, confirming the round-2
reviewer's finding.

## Root cause

`rust/crates/cancellai-provider-claude/src/session.rs::discover_claude_sessions` classified
`projects_meta.file_type().is_symlink() || !projects_meta.is_dir()` as
`SessionDiscoveryScope::Unavailable` (the "structurally empty install" branch, `Complete`,
zero-cost). A present-but-wrong-kind root shares nothing with an absent or symlinked one: only
those two are a known, safe empty state (SI-009). The `!projects_meta.is_dir()` clause folded a
third, unsafe case into the same branch.

## Repair

Removed the `!projects_meta.is_dir()` clause from the early return. A present, non-symlink,
non-directory `projects` now falls through to the existing `fs::read_dir(&projects)` call, which
fails naturally with the real OS `ENOTDIR` error - routed through the same `unobservable(&projects,
&error)` path every other read failure in this function already uses (permission denial, mid-walk
disappearance, etc.). No new error-classification branch was added; the existing one now sees a
case it previously never reached.

Diff: `rust/crates/cancellai-provider-claude/src/session.rs` (production, minimal - one clause
removed, comment added explaining the fallthrough), plus regression tests below. No other file's
production logic changed for this repair (`git diff --stat` at posting time shows only this file
and its own test module plus the two test-only files below).

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 (scope-root completeness) | `a_regular_file_projects_root_is_unobservable_not_a_clean_empty_scope` (unit, `session.rs`); native reproduction above, post-repair, now exits 4 matching the Python reference | PASS |
| AC2 (reported, never silently skipped) | `a_regular_file_claude_projects_root_is_reported_incomplete_with_a_real_count` (native CLI, `cli_behavior.rs`): `inspect --json` reports `complete: false`, `error_count: 1` | PASS |
| AC4 (consistent across `clean --yes`/`--dry-run`/withholding) | `a_regular_file_claude_projects_root_withholds_and_exits_four`, `a_regular_file_claude_projects_root_withholds_dry_run_too` (native CLI) | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | Non-directory scope root must not authorize destructive work | Native `clean --yes` now exits 4, no deletion | PASS |
| SI-009 | Missing evidence is not absence of data | `error_count: 1`, `complete: false` reported via `inspect --json` | PASS |
| SI-010 | Scan errors are visible | Same as above | PASS |
| SI-014 | Safety withholding is non-success | Exit code 4, not 0, for both `--yes` and `--dry-run` | PASS |
| C-02 | Ambiguity never escalates privilege | A present-but-wrong-kind root is now `Unknown`/`Unobservable`, never `Complete` | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                                        -> PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings     -> PASS
cd rust && cargo test --workspace                                                   -> PASS (0 failed, full workspace)
python3 scripts/rust_python_parity.py check                                         -> PASS (13 NORMATIVE fixtures, both root origins)
python3 scripts/check_fixtures.py check                                             -> PASS
python3 scripts/check_mutation_boundary.py check                                    -> PASS (unaffected - this repair is E05/E21 scope-completeness, not the mutation boundary)
python3 scripts/project_os.py check                                                 -> PASS
Manual native reproduction (Rust vs. Python reference, above)                        -> matches (exit 4, both engines)
```

## Compatibility

- Unix-only reproduction (native regression tests are `#[cfg(unix)]`, matching this file's
  existing mode-000 tests for the same reason: `std::os::unix::fs::PermissionsExt` and POSIX
  file-type semantics). Not claimed for Windows; no Windows-specific behavior changed.
- No fixture-corpus (`tests/fixtures/characterization/`) addition: that format describes a
  directory's *contents*, and this case replaces the root itself with a non-directory, which the
  current fixture language has no way to express. Native CLI regression tests
  (`cli_behavior.rs`) cover the case directly against the real built binary instead, per the
  round-2 review's own "if the current fixture language can express" qualification.

## Documentation updated

- None required beyond this evidence packet: the existing module doc in `session.rs` already
  states the correct rule ("a missing or symlinked `projects/` is `Unavailable` ... a `projects/`
  that exists and cannot be listed **is** a reason") - the code simply did not implement it for
  this one case. No architecture/security document made a claim this repair contradicts.

## Method defects

- **What happened**: the round-1 story closure (E21-CLOSURE.md) and its independent review
  (E21-VERIFIER-REVIEW.md) did not test a present-but-non-directory scope root, only an absent one
  and a permission-denied (mode-000) one - a third distinct filesystem shape the same defect class
  (CR-TE-01) could take. **Prevented by**: none exists specifically for "enumerate every `fs::
  symlink_metadata` outcome a scope root can have" as a required adversarial axis; `adversarial-
  cases`' "Malformed / untrusted input" axis covers this in spirit but round 1 did not exercise it
  for this exact function. **Disposition**: proposed - worth naming as an explicit case under that
  axis for future CR4 filesystem-observation stories, but not actioned in this repair (out of this
  story's scope; recorded here per the diff-discipline rule against silent scope expansion).

## Residual risks

- None newly introduced by this repair. The pre-existing, disclosed residuals in
  `E21-CLOSURE.md` (bounded reason retention, the `fstatat`/`unlinkat` window, audit-author bias)
  are unchanged by this change.

## Verifier verdict

PENDING - awaiting independent re-run by Codex against this repair, per this repository's
executor/verifier separation (AGENT_PROTOCOL.md: an executor's work ends at `ready_for_review`
and it does not write its own Safety Verdict for a CR4 story).
