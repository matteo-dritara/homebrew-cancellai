# ADR-0014: Closing an epic cuts a release, and review is bounded to two rounds

- Status: Accepted
- Date: 2026-08-28
- Decision owners: project owner / cEOS
- Related: PD-021, PD-022, PD-018, C-16, C-17, E00

## Context

E00 exposed two failure modes in the engineering system that are about *cadence*, not about correctness.

**Work that is done does not reach users.** E00 closed with every P0 defect repaired and none of it released. The shipped Homebrew formula still pointed at `v1.0.2`, a build whose protected-name constants were documentation rather than a barrier. Finished work sitting unreleased is not a neutral state: users keep running the version with the defects, and the gap between `main` and the released artifact grows until releasing becomes its own project.

**Review had no bound.** E00 ran three review rounds. Each one found a real defect and each one was worth having, but the process contained no rule that would ever stop it: rounds 1, 2 and 3 rejected 6/7, 7/7 and 1/7 stories, and nothing in the system said when the loop ends. Reviews were also requested per story, so the reviewer re-read overlapping code repeatedly and the epic could not converge while individual stories bounced.

Both are cadence problems, and cadence problems compound: the longer review runs, the further behind the release falls.

## Decision

### Closing an epic cuts a release

An epic reaching `done` produces a version tag and everything that follows it: a version bump, a cut changelog section, an updated Homebrew formula, a GitHub release, and a committed release evidence packet. This is not an optional follow-up. `scripts/check_process.py` refuses a `done` epic that has no release evidence naming it.

`scripts/release.py` automates the sequence. It is deliberately two commands, because the archive checksum cannot exist before the tag does:

```sh
python3 scripts/release.py prepare --version X.Y.Z --epic E00
# commit, tag, push
python3 scripts/release.py finalize --version X.Y.Z
```

`.github/workflows/release.yml` re-runs every gate at the tag and publishes the GitHub release from the committed evidence packet, so the released artifact is verified at the commit it was cut from rather than at whatever `main` looked like afterwards.

### Review happens once per epic, at most twice

An epic is reviewed when **all** of its stories are implemented, not story by story. The reviewer receives one coherent change with one contract to falsify.

A maximum of **two** review rounds per epic:

1. round 1 reviews the completed epic;
2. the executor repairs what it found;
3. round 2 reviews the repairs;
4. the epic closes.

Findings that survive round 2 do not trigger a third round. They become **new work items** in the backlog, recorded in the epic's closure packet as accepted residual risk with the story id that will carry them. An unbounded loop hides the same defects behind a status that never changes; a bounded loop converts them into visible, scheduled work.

`scripts/check_process.py` enforces the ceiling by counting committed verifier review records per epic.

## Consequences

Positive:

- released state tracks reviewed state, so the gap between what is verified and what users run stays one epic wide at most;
- the reviewer sees a complete epic instead of a story fragment whose neighbours are still moving;
- "when does review end" has an answer that does not depend on how tired anyone is;
- unfixed findings become scheduled work with an id, rather than an epic that quietly never closes.

Costs:

- **A second round can miss what a third would have caught.** E00's third round found a real defect that rounds 1 and 2 did not. This decision accepts that risk explicitly: the alternative was an unbounded process, and an unbounded process ships nothing. The mitigation is that surviving findings become backlog items rather than being dismissed.
- Releases become more frequent, so version numbers move faster than the product's user-visible surface changes. That is the correct direction for SemVer, but it means the changelog carries internal work more visibly.
- Batching review to epic scope makes each round larger. A reviewer must budget for the whole epic rather than one story.

## Rejected alternatives

- **Release on a schedule instead of on epic closure:** decouples releasing from verification, so a release can carry half-reviewed work or wait weeks for the next slot.
- **Release per story:** version churn without a coherent unit of change, and a story is not independently meaningful to a user.
- **Unbounded review until a clean round:** what E00 did. It produced real findings and no completion criterion; the third round was stopped by the owner, not by the process, which means the process had no answer.
- **One review round only:** round 2 of E00 rejected all seven stories, so a single round would have shipped every one of those defects. One round is measurably not enough here.
- **Deferring the release to a separate manual step after closure:** that is exactly the state this ADR corrects.
