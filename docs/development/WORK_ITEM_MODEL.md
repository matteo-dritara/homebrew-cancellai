# Work Item Model

## Story contract

The machine-readable story is the minimum implementation contract:

- stable ID;
- title;
- status;
- outcome;
- Change Risk Level;
- dependencies;
- acceptance criteria;
- safety invariant references;
- verification plan;
- documentation impact.

Generated [`../BACKLOG.md`](../BACKLOG.md) renders these contracts for humans.

## Story statuses

`planned -> ready -> in_progress -> ready_for_review -> verification -> done`

| Status | Meaning | Owned by |
| --- | --- | --- |
| `planned` | contract exists, not scheduled | owner |
| `ready` | contract is complete and dependencies are satisfied (see below); may be picked up | owner |
| `in_progress` | an executor is implementing it | executor |
| `ready_for_review` | implementation, tests and documentation are complete and all gates pass; the story is waiting for its **epic's** review round | executor |
| `verification` | the independent reviewer is actively falsifying the epic this story belongs to | reviewer |
| `done` | verified, evidence committed, and for CR4 a Safety Verdict recording a pass exists | owner |

Side states:

- `blocked` - cannot proceed; blocker is documented.
- `cancelled` - intentionally removed with rationale/decision reference.

A status change is project state and belongs in version control.

### `ready_for_review` is the standard executor exit state

An executor **never** moves its own work past `ready_for_review`. That status is the handoff itself, and it carries obligations:

- every gate required by the story's Change Risk Level passes locally;
- tests, documentation and changelog entries land in the same change;
- an executor evidence packet is committed under `project/evidence/`, per story or per epic batch;
- `python3 scripts/project_os.py check` enforces that evidence exists before a story may sit in `ready_for_review`.

The Safety Verdict required to close a CR4 story is the **reviewer's** output, so it is gated at `done`, never at `ready_for_review`.

List the queue with:

```sh
python3 scripts/project_os.py review
```

### Intra-epic dependency chains are satisfied at `ready_for_review`

A story dependency is satisfied - and unblocks `ready`/`in_progress`/`ready_for_review` on the
dependent story - as soon as:

- the dependency is a story in a **different** epic, or an epic-level dependency: it must be
  `done` (that epic is independently closed and released, per ADR-0014);
- the dependency is a story in the **same** epic: it must have reached `ready_for_review`, not
  `done`.

This is what makes epic-scope, once-at-the-end review (below) possible for an epic whose
stories form a chain rather than sitting side by side: E01's stories depend on each other in
sequence, so requiring each link to be independently `done` before the next could start would
force a review round per story and contradict "review is per epic, not story by story."
`scripts/project_os.py check` enforces exactly this distinction, not a uniform "done" rule
across every dependency edge.

## Review is per epic, and bounded to two rounds

An epic is reviewed when **all** of its stories are `ready_for_review`, not story by story.
The reviewer receives one coherent change with one contract to falsify, rather than a
fragment whose neighbours are still moving.

**Two rounds, maximum.** ADR-0014 / PD-022:

1. round 1 reviews the completed epic;
2. the executor repairs what it found;
3. round 2 reviews the repairs;
4. the epic closes.

Findings that survive round 2 do **not** trigger a third round. They become new work items
in the backlog, recorded in the epic's closure packet as accepted residual risk with the
story id that will carry them. An unbounded loop hides defects behind a status that never
changes; a bounded one converts them into visible, scheduled work.

`scripts/check_process.py` counts committed verifier review records per epic and fails above
the ceiling. E00 predates this rule and ran three rounds; it is the reason the rule exists,
and it is recorded as an explicit exception rather than quietly exempted.

## Closing an epic cuts a release

An epic reaching `done` produces a version tag and everything that follows it. This is not an
optional follow-up: `scripts/release.py check` fails when a closed epic has no release
evidence naming it, and that check runs in `pre-commit` and in CI.

```sh
python3 scripts/release.py prepare --version X.Y.Z --epic EXX
git commit -am "chore(release): X.Y.Z"
git tag -a vX.Y.Z -m "cancellAI X.Y.Z" && git push --follow-tags
python3 scripts/release.py finalize --version X.Y.Z
```

`prepare` bumps the version in the source and the packaging metadata, cuts the changelog
section, and writes the release evidence packet from the epic's contract. `finalize` writes
the archive checksum into the Homebrew formula - a separate command because that checksum
cannot exist until GitHub has generated the archive from the tag.

`.github/workflows/release.yml` re-runs every gate at the tag and publishes the GitHub
release from the committed evidence packet. A release verified against whatever `main` looked
like afterwards is not evidence about the artifact users install.

See [ADR-0014](../adrs/0014-epic-closure-is-a-release-and-review-is-bounded.md) and
[RELEASING.md](../RELEASING.md).

## Change Risk Levels

### CR0 - Documentation / metadata

Examples: prose, generated planning metadata, typo, non-executable templates.

Minimum gates:

- governance/docs validation;
- link/format checks where available;
- no product behavior claim that contradicts implementation.

### CR1 - Observational behavior

Examples: status rendering, read-only query, TUI view, metrics calculation that cannot influence mutation authority.

Minimum gates:

- unit/integration tests;
- deterministic outputs where contracted;
- performance/compatibility check proportional to change.

### CR2 - Classification / planning / state semantics

Examples: project attribution, provider capability result, policy schema, plan serialization.

Minimum gates:

- unit + integration;
- golden fixture tests;
- error/unknown-state tests;
- differential tests during Python->Rust migration where applicable;
- independent verifier for safety-relevant classification changes.

### CR3 - Reversible/conditional mutation

Examples: quarantine, archive, metadata rewrite, configuration write, service install.

Minimum gates:

- all CR2 gates;
- fault injection/crash recovery;
- idempotency/retry behavior;
- rollback/restore path;
- adversarial filesystem/concurrency tests;
- independent verification.

### CR4 - Irreversible mutation or authority boundary

Examples: purge, safety executor, root capability, policy authority lattice, provider trust, release-channel authority, remote mutation boundary.

Minimum gates:

- all CR3 gates;
- explicit threat cases;
- direct invariant mapping;
- exhaustive/property/fuzz tests where appropriate;
- independent adversarial verifier;
- owner-visible Safety Verdict;
- release evidence references;
- no unresolved HIGH/CRITICAL residual safety risk without explicit owner acceptance.

## Story changes during implementation

If implementation reveals an AC is wrong or an architectural decision is missing:

1. stop widening implementation;
2. update story/RFC/ADR first;
3. re-run governance generation;
4. have verifier use the new contract.

The code never becomes the unofficial spec by accident.

## Epics and phases

Epics define coherent capability outcomes. Phases have exit criteria. Dates may be estimated separately, but a phase does not become complete because a calendar date arrived.

## RFC trigger

Write an RFC before coding when a story proposes:

- a new public protocol/schema with long-lived compatibility cost;
- a new dependency with material security/operational impact;
- a new mutation model;
- a new network trust boundary;
- a new persistence strategy;
- a major UI/domain semantic change;
- a change that could reasonably produce competing architecture options.

## ADR trigger

Write an ADR when the team/owner has chosen among significant alternatives and the rationale must survive. An RFC explores; an ADR records the accepted architectural decision.
