# Evidence Packet - E17-S06

- Commit/PR: this working-tree change (executor session, 2026-09-08)
- Executor: Claude
- Independent verifier: Codex (pending, epic-scoped per AGENT_PROTOCOL.md)
- Change Risk: CR2
- Spec version/commit: `project/epics/E17.json` (E17-S06), as of this change

## Outcome

PASS

This story executes the planning half of a decision [ADR-0011](../../../docs/adrs/0011-defer-canonical-repository-split.md)
already made: ADR-0011 decided the canonical repository *will* split (product-named
`cancellai` + Homebrew-tap-only `homebrew-cancellai`) but deferred "the exact timing and GitHub
migration mechanics" until real evidence exists. E17-S06 writes those mechanics -
`docs/RELEASING.md`'s expanded "Repository topology transition" section - and adds
`scripts/check_repository_topology.py`, a new governance check enforcing that the canonical
repository is unambiguously identified today (AC2), not merely claimed to be.

No repository was created, transferred, or mirrored by this story. That is a deliberate,
disclosed scope boundary, not an omission: creating a new GitHub repository and migrating
history are real, hard-to-reverse actions on shared state that the trigger condition this story
writes (E06-S04 cutover complete, at least one real canonical release shipped, explicit owner
authorization) has not been met - and would not be an agent's call to make unilaterally even if
it had been. `git push --mirror` and `gh issue transfer` (the two commands the runbook names)
were verified to exist as real commands (`gh issue transfer --help`) but were not executed
against this repository or any other.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - The migration plan preserves tags/releases/issues or documents intentional moves and does not break existing Homebrew users | `docs/RELEASING.md`'s "Migration steps" (1-5): step 1 names `git push --mirror` for byte-identical history preservation over GitHub's repository-transfer feature (rejected because it moves the wrong repository - see the step's own reasoning); step 2 documents issues as a deliberate, non-bulk split (era-relevant issues transferred via `gh issue transfer`, Python-v1-era issues stay); step 3 keeps existing tags/releases permanently in `homebrew-cancellai` (immutable history, matching this document's own pre-existing "published tags are never deleted" principle) while new releases move to the new repository; step 4 is the Homebrew-continuity argument itself (the tap's *name* never changes, only the Formula's upstream URLs do, so `brew update && brew upgrade` needs zero user action) | PASS |
| AC2 - The canonical source repository is unambiguously identified by release provenance and contributor documentation | Release provenance: already true as of E17-S01/S02 - `release-manifest.json`'s `build_identity.repository` is `${GITHUB_REPOSITORY}`, not a hand-maintained string, so it automatically names whichever repository actually ran the release workflow. Contributor documentation: `scripts/check_repository_topology.py check` (new) cross-checks `Formula/cancellai.rb`'s `homepage`, `scripts/release.py`'s `REPO` constant, and `docs/RELEASING.md`'s "current remote" claim all name the same repository, and fails otherwise - an enforced fact, not merely a written one; `tests/test_repository_topology.py` proves both the real repository's current consistency and that the checker actually rejects each of the three ways it could drift (9 tests) | PASS |
| AC3 - homebrew-cancellai remains usable as a tap or is retired only through an explicit compatibility plan | Migration step 5 states this explicitly: `homebrew-cancellai` is never silently retired, remains the tap indefinitely per ADR-0011's target topology, and continues under the same `brew audit`/`brew install`/`brew test` CI gates unchanged; retiring it (not merely repointing it) is named as requiring its own separate ADR and compatibility plan, which this story does not create and does not authorize | PASS |

## Safety Evidence

Not applicable in the CR3/CR4 sense - `safety_obligations: []`, CR2 (classification/planning).
The relevant care taken is procedural, not a Safety Invariant: no external, hard-to-reverse
action (repository creation/transfer, issue migration) was executed, matching this session's own
"Executing actions with care" obligation independent of the story's formal CR level.

## Verification Commands

```text
python3 -m pytest tests -v
python3 -m mypy --strict scripts/check_repository_topology.py
python3 -m ruff check . && python3 -m ruff format --check .
python3 scripts/check_repository_topology.py check
python3 scripts/check_workflows.py check
python3 scripts/check_docs.py check
gh issue transfer --help   # confirms the runbook names a real command, not an assumed one
```

All PASS locally. `tests/test_repository_topology.py`: 9/9 (real-repository consistency, each
of the three drift directions rejected individually, deliberate-migration-to-a-new-shared-name
not falsely flagged, malformed-repository-value rejection, CLI entry point). Full suite:
251/251 (was 242 before this story - 9 new).

## Compatibility

- The new check reads `Formula/cancellai.rb`, `scripts/release.py`, and `docs/RELEASING.md` as
  plain text (regex extraction, stdlib-only, matching every other governance checker in this
  repository) - no network access, no GitHub API calls, so it runs identically in this sandbox
  and in CI.

## Performance / operability

- `check_repository_topology.py check` reads three small files and does string matching;
  negligible runtime, consistent with the other `pre-commit`/CI governance checks it now runs
  alongside.

## Documentation updated

- `docs/RELEASING.md` - "Repository topology transition" section rewritten as a full runbook
  (trigger condition, migration steps, dry-run/smoke-test mapping to this story's own
  verification contract).
- `docs/security/SUPPLY_CHAIN.md` - new "Canonical repository identity" subsection under
  "Source controls".
- `CHANGELOG.md` - `[Unreleased]` entry.

## Residual risks

- The migration itself remains unexecuted, by design - this story is the plan, not the move.
  The trigger condition (E06-S04 cutover, a real canonical release, explicit owner
  authorization) is stated in `docs/RELEASING.md` but is not itself enforced by any automated
  gate; whoever executes the migration later is trusted to check it, the same way this
  repository already trusts a human to run `scripts/release.py prepare`/`finalize` deliberately
  rather than automatically.
- `scripts/check_repository_topology.py` checks three specific, currently-hardcoded sources of
  truth; a fourth place that independently names the canonical repository (should one appear
  later) would need this checker extended to cover it, the same maintenance burden every
  cross-file consistency checker in this repository already carries.

## Verifier verdict

(pending - epic-scoped review per `docs/development/AGENT_PROTOCOL.md`)
