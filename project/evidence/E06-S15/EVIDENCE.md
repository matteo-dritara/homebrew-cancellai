# Evidence Packet - E06-S15

- Commit/PR: the E06-S15 commit on `main`
- Executor: Claude
- Independent verifier: pending
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit; ADR-0040 (owner decision 2026-09-24)

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - adopting the cutover requires an owner authorization naming the version and the SHA-256 of the accepted Safety Verdict | `release.py cutover_authorization_problems` reads `project/evidence/E06-S04/CUTOVER_AUTHORIZATION.md` and requires `Authorized-by:`, `Version:` and `Safety-Verdict-SHA256:` exactly once each; `finalize --adopt-cutover` refuses while it reports any problem. `test_a_valid_authorization_passes`, `test_every_field_is_required_exactly_once`, `test_adoption_needs_the_owners_authorization` | PASS |
| AC2 - a changed verdict, a failing final round, or another version refuses and leaves the formula unchanged | `test_a_verdict_edited_after_the_authorization_voids_it`, `test_a_failing_final_round_is_refused_even_when_authorized`, `test_an_authorization_for_another_version_is_refused`; the final round is read by `project_os.py`'s own `safety_verdict_passes` (`test_the_real_safety_verdict_reader_is_project_os`), so the release and the control plane cannot disagree | PASS |
| AC3 - no story status is treated as authorization | `cutover_story_status` is removed; `test_no_story_status_is_read` | PASS |

Mutation checks, each killed: dropping the verdict digest comparison, the version comparison, or
the final-round check.

## Round 10 repair (Codex; refused by the harness, `project/evidence/E06-S14/ROUND10_FINDINGS.md`)

| Finding | Repair | Evidence |
| --- | --- | --- |
| A verdict `## Round 10 ... FAIL` followed by `## Owner note ... PASS` read as passing through `project_os.safety_verdict_passes` (last verdict-shaped line wins) | `release.py safety_verdict_passes` reads the last `## Round <n>` section's own standalone verdict lines, fenced blocks excluded, and fails when any verdict-shaped line follows that section | `test_a_later_note_cannot_reverse_a_failed_final_round` (Codex's case, now through the real reader), `test_the_final_round_section_decides` (8 cases); mutations reading the first round or ignoring later sections each fail |
| `Authorized-by: stranger` was accepted | `Authorized-by` must equal the owner `.github/CODEOWNERS` assigns to `*` (`@matteo-dritara`) | `test_only_the_repository_owner_can_authorize` |

**Owner decision on authentication (2026-09-24).** Codex asked for an *authenticated* owner decision.
Every commit here, an agent's included, is signed with the owner's GPG key, so nothing in the
repository can tell the owner from a process acting as the owner; a dedicated signing key would
add, in the owner's words, too much complexity. The owner accepted the same-user limit - the one
ADR-0039 already accepts for the containment ledger - with the identity check above closing the
reproduced case.

**Flagged, not fixed:** `scripts/project_os.py safety_verdict_passes`, which gates CR4 story
closure, has the same last-line-wins weakness. It belongs to E32-S01's mechanism and is outside
this story; recorded here as a backlog candidate.

## Verification Commands

```text
python3 -m pytest tests/test_release.py -q   -> 64 passed
```

## Residual risks

- **The file is an identity claim**, like every `Verifier:` line here: whoever can commit can write
  it. What it adds over a status is a binding: it names the exact Safety Verdict bytes accepted, so
  a later verdict edit voids it, and it names the version.
- **No authorization exists yet**, by design: it follows an independent PASS on E06-S04's
  migration Safety Verdict.

## Verifier verdict

pending
