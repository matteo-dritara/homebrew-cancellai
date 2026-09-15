# E28 Independent Verifier Review — Round 2

- Review target: `00ee95f..7a7783f`
- Verifier: Codex (`/root`), independent of the Claude executor
- Date: 2026-09-15
- Epic: E28 — Toolchain Assurance

## Round decision

Round 2 is required. Round 1 was an epic-scope review conducted through five story-scoped records:
it found four material defects before repairing them. Had those stories been formally rejected
instead of repaired in the review, its yield would have been 4 of 5 (80%), above ADR-0025's 10%
threshold. The current `process_metrics.py` reports neither that round nor its findings: it skips
the story-scoped files silently, cannot parse their standalone verdict headings, and counts only
`FAIL`, so a repaired-in-round defect would still read as zero yield. This record is deliberately
epic-scoped and table-formatted so the current parser can read this second round; it does not repair
the CR2 measurement defect or retrospectively make round 1 visible.

## Per-story verdicts

| Story | Verdict | Round-2 evidence |
| --- | --- | --- |
| E28-S01 | PASS_WITH_RESIDUALS | The Python 3.10 marker repair is confirmed by the successful 3.10 CI job and source-level marker tests. Static `--no-llm` coverage remains deliberately partial. |
| E28-S02 | PASS_WITH_RESIDUALS | A Git commit signature authenticates the owner key that committed a verdict, not the Claude/Codex role; no role-key binding exists. |
| E28-S03 | PASS_WITH_RESIDUALS | The gate validates record shape and disposition, but truth/usefulness remains a human judgement and `proposed` has no expiry. |
| E28-S04 | PASS_WITH_RESIDUALS | The token conversion is approximate and user-scope components are not CI-observable; licence declarations are point-in-time evidence. |
| E28-S05 | PASS_WITH_RESIDUALS | The current rejected status set is behaviourally tested, but an explicit future edit to `CLOSED_EPIC_STATUS` expands it; the earlier PASS omitted that residual. |

## Measurement finding and owner proposal

The review-yield measurement requires a CR2 owner decision before implementation. At minimum it
must report every filename it deliberately skips, parse a round record rather than infer one from
its placement, and distinguish `defect found and repaired in review` from `nothing found`. Candidate
designs include a first-class repaired verdict, counting independently reproduced verifier repairs
as rejections, or requiring one epic round record with a per-story table. The choice changes when
ADR-0025 requires another round and must be specified as a new owner-approved work item.

## Residual dispositions

| Story | Residual | Disposition | Proposed owner work and proof |
| --- | --- | --- | --- |
| E28-S01 | Static scan has zero fully inspected files | Inherent / accepted | `--no-llm` preserves content locality; refusing all partial analysis would make the selected privacy posture unable to run. Keep the gate fail-closed for omitted skills/files and state the coverage limit. |
| E28-S01 | Waiver fingerprints across scanner-version changes | Closable | Proposed CR2 story: bind the waiver file's scanner version to the gate pin and refuse an upgrade until waivers are explicitly revalidated. Prove an old waiver cannot suppress a finding after a pin change. |
| E28-S02 | Claimed verifier identity is forgeable | Closable, but only with distinct authenticated identities | Proposed CR4 ADR/story: verifier-owned non-exportable signing keys, trusted key-to-role mapping, and a gate that verifies the commit introducing the verdict. Prove unsigned, wrong-key and rewritten/squashed verdict commits refuse. A shared owner GPG key authenticates only the owner, not the agent role. |
| E28-S03 | `proposed` can remain unresolved | Closable | Proposed CR1 story: age-report proposals like toolchain decisions without declaring them invalid. Prove dated warnings appear and accepted/declined entries remain visible. |
| E28-S03 | Truth/usefulness of a method defect | Inherent / accepted | General factual usefulness needs a human reader. Automation can require evidence links, but cannot prove an observation is valuable. |
| E28-S04 | User-scope costs and approximate conversion | Partially closable | Proposed CR2 story: accept dated, version/hash/toolchain-bound local measurements and warn on expiry or source mismatch. This reduces uncertainty but CI still cannot re-observe the local installation. |
| E28-S04 | Upstream licence can change after recording | Closable | Proposed CR2 story: record licence evidence against the pinned source revision and revalidate it when version/review cadence changes. Prove relicensing/missing evidence refuses or warns according to owner policy. |
| E28-S05 | Future expansion of `CLOSED_EPIC_STATUS` | Closable | Proposed CR2 story: combine the behavioural test with an exact current closure-set assertion and require an explicit reviewed policy change for expansion. Prove adding a closure state fails until both policy and test are consciously updated. |

## Method finding

Commit `e59aadc` claims to install SkillSpector and carries `Story: E28-S01`, but its only diff is
Ruff formatting in `tests/test_project_os.py` (E28-S05). That message made the later Python-version
repair appear already present. The E28-S05 evidence packet records this commit-message/diff
mismatch as a proposed method defect; no history is rewritten.
