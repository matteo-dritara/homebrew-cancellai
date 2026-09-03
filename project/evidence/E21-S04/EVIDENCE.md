# Evidence Packet - E21-S04

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR2
- Spec version/commit: ADR-0018; `docs/audits/2026-09-03-CODE_REVIEW.md`, finding `CR-TE-02`

## Outcome

PASS

## Scope

Makes completeness impossible to omit by construction rather than by review. `CR-TE-02` found
that `cancellai-inventory` - four `done` stories, including a completeness model an adversarial
review round forced into shape - was unreachable from the shipped binary, and that the exact
defect E04-S03's verifier had rejected reappeared in the adapters that replaced it. This story
takes the guarantee, not the walker (ADR-0018).

This story must not change a single observable classification; the differential gate is the
proof of that, and it stayed green across the refactor.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - provider discovery returns `cancellai-inventory`'s `ScopeCompleteness`, adapters keep their layout-specific traversal | `SessionDiscoveryResult.completeness` and `RolloutDiscoveryResult.completeness` are `cancellai_inventory::ScopeCompleteness`. Both walks are unchanged in shape - Claude's flat project/session layout and Codex's date-partitioned rollout tree remain different traversals, per ADR-0018's rejected-alternative section. | PASS |
| AC2 - policy obtains planning candidates only through a value that cannot be constructed without completeness | `ProviderResolution::artifacts` is private. `observed()` is the reporting route and says so; `planning_view()` returns a `ProviderPlanningView` bundling `completeness` with the artifacts, and `build_actions` takes `&[ProviderPlanningView]` rather than resolutions. `scan_complete()`/`scan_incomplete_reason()`/`scan_error_count()` are all derived from the one stored `ScopeCompleteness`, so the three can no longer disagree about the same scan. | PASS |
| AC3 - `cancellai-inventory` is reachable from the shipped binary, and a check fails if it stops being | `scripts/check_rust_workspace.py::_check_required_reachability` walks the transitive Cargo graph from `cancellai-cli`. Proven by removing the dependency from all four crates that carry it: `cancellai-cli can no longer reach cancellai-inventory: … A crate the shipped binary does not depend on cannot enforce anything for it (CR-TE-02 / ADR-0018).` | PASS |
| AC4 - ADR-0018 records the decision and the alternatives weighed | `docs/adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md`, including the rejected full rebase onto `scan_scope` (rejected for parity risk during a safety repair, not for correctness) and the rejected "declare it non-production". | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 / SI-009 | A downstream crate taking the candidates and leaving the completeness behind | `compile_fail` doctest on `ProviderResolution`: `resolution.artifacts.len()` from outside the crate does not compile. The paired positive doctest (`resolution.observed().len()`) must keep compiling, so the guarantee is a boundary rather than a wall. Both run under `cargo test --doc`. | PASS |
| — | The refactor must not change behaviour | `python3 scripts/rust_python_parity.py check` - 12 NORMATIVE fixtures, both root-origin scenarios, unchanged before and after | PASS |

## Verification Commands

```text
$ cargo test -p cancellai-policy --doc -- --list
crates/cancellai-policy/src/retention.rs - retention::ProviderResolution (line 111): test
crates/cancellai-policy/src/retention.rs - retention::ProviderResolution (line 100): test

$ cargo test -p cancellai-policy --doc
test … (line 111) ... ok                       # observed() still compiles
test … (line 100) - compile fail ... ok        # artifacts is unreachable

$ python3 scripts/check_rust_workspace.py check
rust workspace OK: 13 crates match TARGET.md, acyclic, model/safety isolated

$ cargo test --workspace                       318 passed, 0 failed
$ python3 scripts/rust_python_parity.py check  12 NORMATIVE fixtures, both scenarios, OK
```

## Compatibility

- `cancellai-policy` gains a dependency on `cancellai-inventory`. The graph stays acyclic
  (`inventory -> {model, platform}`) and `check_rust_workspace.py` confirms it.
- `ProviderResolution`'s public field set changed. The only consumer is `cancellai-cli`, updated
  in the same change.

## Performance / operability

- `ProviderPlanningView` borrows; no cloning of artifact vectors was introduced. The shipped-path
  benchmark E21-S05 adds covers this path and passes well inside budget.

## Documentation updated

- `docs/adrs/0018-…md` (new), `docs/architecture/TARGET.md`, `docs/architecture/DOMAIN_MODEL.md`.

## Residual risks

- `cancellai-inventory` still owns a walker (`scan_scope`) that production does not call. ADR-0018
  records this as a deliberate residual rather than leaving it to be rediscovered; it remains the
  reference implementation of the completeness model, and E10 is the natural place to revisit
  unification if it needs one traversal for its own reasons.
- The reachability check proves the crate is on the dependency path, not that its guarantees are
  used well. The `compile_fail` regression is what covers the second question, and only for the
  planning route.


## Round-1 independent review: FAIL, and its repair

The verifier confirmed the construction-level interface is sound and that no destructive path
plans from `observed()`, but failed the story because it faithfully carried a falsely `Complete`
value from E21-S03 - a correct verdict: a type that transports a wrong answer intact is not
satisfying SI-008/SI-009, it is only not making things worse.

Repaired by E21-S03's fix upstream, plus a strengthening here: `ProviderResolution` now stores a
`ScopeObservation` (classification *and* truthful unobserved count) rather than a bare
`ScopeCompleteness`, so `scan_complete()`, `scan_incomplete_reason()` and `scan_error_count()`
are three views of one value that cannot disagree, and the count can no longer be inferred from
a bounded reason list. `ProviderPlanningView` carries the observation for the same reason.

## Verifier verdict

`FAIL` (round 1) - repaired above; owner-accepted closure without a round 2, see project/evidence/E21-CLOSURE.md
