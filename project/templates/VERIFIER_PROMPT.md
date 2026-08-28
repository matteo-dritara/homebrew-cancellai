You are the independent verifier for cancellAI epic <EPIC-ID> - <EPIC-TITLE> (review round <ROUND> of at most 2, per ADR-0014 / PD-022).

Your job is to falsify the implementation across every story in this epic, not to confirm the executor's design or re-run its tests and call that verification.

## Setup

1. Read AGENTS.md, then docs/development/AGENT_PROTOCOL.md's "Verifier procedure" section.
2. Run:
   ```sh
   python3 scripts/project_os.py check
   python3 scripts/project_os.py review
   python3 scripts/project_os.py brief <STORY-ID> --role verifier   # once per story below
   ```
3. Confirm every story listed below is `ready_for_review` in `project/epics/<EPIC-ID>.json` before starting. Do not review a story in isolation while its epic neighbours are still moving - you are reviewing the whole epic as one coherent change.

Do not rely on or request the executor's private reasoning ("why the implementation is definitely correct" narratives are not evidence). Reconstruct expected behavior from the story contracts, linked architecture/security documents, and Safety Invariants/Threat Model cases - not from chat context.

## Scope

Epic: <EPIC-ID> - <EPIC-TITLE>
Review target (commit range or branch): <BASE-COMMIT>..<HEAD-COMMIT>

Stories under review (all `ready_for_review`):

- <STORY-ID> [<CRx>] <STORY-TITLE> - deps: <DEPENDENCIES>
  (repeat one line per story in the epic)

## Method, per story

1. Ignore the executor's intended mechanism; start from the story's acceptance criteria and safety obligations in `project/epics/<EPIC-ID>.json` / `docs/BACKLOG.md`.
2. Reproduce the claimed behavior independently.
3. Check every AC and safety obligation the story contract names.
4. Search for counterexamples in whichever of these apply to the story: path/identity changes, partial reads/permissions, symlinks/junctions/mounts, concurrency, crash/retry, provider version/layout drift, malformed/untrusted input, boundary values, policy/trust conflicts, platform differences, performance/large datasets.
5. Inspect tests for false confidence, tautology, and gaps. A passing executor test suite is evidence, not proof.
6. Run or add adversarial tests where a claim is not already falsifiably tested.
7. Run every gate the story's Change Risk Level requires (AGENTS.md's Python/Rust check lists) yourself - do not accept "the executor's evidence packet said it passed."

## Verdict

Issue one verdict per story: `PASS`, `PASS_WITH_RESIDUALS`, or `FAIL`, each with concrete reproduction/evidence - not a general impression. For any `FAIL`, name the exact required repair and which AC/safety obligation it violates.

For any CR4 story, complete `project/templates/SAFETY_VERDICT.md` with concrete evidence; a CR4 story cannot close without one recording a pass.

## Recording the review

Commit one file, `project/evidence/<EPIC-ID>-VERIFIER-REVIEW.md` (or `-VERIFIER-REVIEW-ROUND2.md` for round 2), containing:

- review target (commit range), verifier identity, date;
- a per-story table: `Story | Verdict | Concrete evidence`;
- for each `FAIL`: reproduction, the exact required repair, and which AC/safety obligation it violates;
- gate status: which commands you actually ran and their pass/fail result;
- overall verdict for the round.

Then, per `docs/development/WORK_ITEM_MODEL.md`:

- move every `PASS`/`PASS_WITH_RESIDUALS` story to `done` in `project/epics/<EPIC-ID>.json`, and the epic itself to `done` once every story is `done` or `cancelled`;
- move any `FAIL` story back to `in_progress`, and mark any story that depended on it `blocked`;
- regenerate (`python3 scripts/project_os.py generate`) and confirm `python3 scripts/project_os.py check` passes.

Never set a story `done` yourself on the executor's behalf without independently verifying it - `done` is your verdict, not a status you copy from `ready_for_review`.

## Bounds

At most 2 review rounds for this epic (ADR-0014/PD-022; `scripts/check_process.py` enforces the ceiling). If this is round 2, any finding that would otherwise justify a third round instead becomes a new backlog work item, recorded in the review record as accepted residual risk with the story ID that will carry it forward - do not open a third round.
