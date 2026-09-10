# E08 Independent Verifier Review - Round 1

- Epic: E08 - Universal Artifact and Project Intelligence
- Review target: `8696d7a..4738813` (`8edb1bd` at review head is the unrelated E16/E17 review closeout)
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-11

All four in-scope stories were `ready_for_review` in `project/epics/E08.json` before this
review began. The review reconstructed the contracts from the project control plane,
`DOMAIN_MODEL.md`, `TARGET.md`, the Constitution, Safety Invariants, and Threat Model; it did
not rely on executor reasoning.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E08-S01 | PASS | `AgentArtifact` adds only provider-neutral `ArtifactRelationship { kind, related_artifact_id }`; no UI or provider-shaped field is introduced. A real temporary Codex fixture with a cycle plus a duplicate session ID resolved all three discovered artifacts without an invented orphan claim or panic. The existing graph tests and this independent fixture confirm cyclic/duplicate parent IDs remain bounded; a discovered parent creates `ChildOf`, while an undiscovered parent creates no relationship. The lifecycle/risk/authority fields remain independent typed axes. |
| E08-S02 | PASS | `rg` found no Claude project-name decode or `/-for-` path reconstruction. A temporary real filesystem fixture under `projects/"   "/<uuid>.jsonl` resolved to `project_attribution: None`, proving whitespace-only names are not merely a direct-classifier special case. A normal `projects/-Users-example-project/` fixture stays verbatim with `ExplicitProviderMetadata`; partial-scan handling lowers attribution confidence together with artifact confidence, satisfying SI-023. |
| E08-S03 | PASS | The independently exercised active-orphan case preserves `ActivityState::Active` and its authority cap; unreadable mtime remains `Unknown`; only idle/stale states become `Orphaned`. A temporary filesystem fixture containing a parent cycle and a duplicate archived/session ID passed: every declared, discovered parent remained a relationship and none became an orphan. `build_actions` selects delete only for `Stale`, so `Orphaned` is observational rather than deletion eligibility. Orphan and stale explanations name the missing-parent evidence or mtime/cutoff, respectively. |
| E08-S04 | FAIL | `by_session` follows exactly one `ChildOf` edge, not a complete tree: root -> child -> grandchild produces two buckets (`root`: root/child; `child`: grandchild), where one root bucket of three is required. It also uses an absent relationship target as the bucket key: an input slice containing only `child ChildOf not-present` yields `session_root = not-present`, rather than safely falling back to `child`. The existing reconciliation test uses `BTreeSet`, so it proves neither multiplicity nor this input-slice boundary. |

## E08-S04 required repair

### Reproduction

Two temporary verifier tests were added locally, run, and removed without changing the review
target:

```text
cargo test -p cancellai-policy by_session_groups_a_transitive_child_chain_under_the_tree_root -- --nocapture
# failed: left bucket count 2, right 1

cargo test -p cancellai-policy by_session_falls_back_to_own_id_when_parent_is_absent_from_the_input_slice -- --nocapture
# failed: left ArtifactId("not-present"), right ArtifactId("child")
```

### Required repair

Make `by_session` resolve a complete `ChildOf` chain against an index of artifacts actually in
the supplied slice. A missing target, a cycle, or an ambiguous duplicate ID must fall back to
the originating artifact's own ID rather than emitting an external/non-root key or looping.
Add chain, missing-parent, cycle, and duplicate-ID regressions. Reconciliation assertions must
compare counted ID occurrences (not only a `BTreeSet`) so a duplicate or dropped source row is
observable.

This violates E08-S04's session-view outcome and AC2 (cross-dimensional reconciliation over the
input inventory); the direct-parent implementation also fails the module's documented claim to
group a Codex subagent tree under its resolved root. E08-S04 returns to `in_progress`; no other
E08 story depends on it.

## Gate status

| Command / probe | Result |
| --- | --- |
| `python3 scripts/project_os.py check` | PASS before review status changes |
| `python3 scripts/project_os.py review` and four verifier briefs | PASS; all E08 stories queued together |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS (scheduled heavy benchmarks remained intentionally ignored) |
| `cargo deny check` | PASS; existing unmatched-license-allowance and duplicate `syn` warnings only |
| Real whitespace-only Claude project fixture | PASS: explicit `Unattributed` |
| Cyclic/duplicate Codex parent fixture | PASS: no false orphan evidence |
| Transitive and absent-parent session-view probes | FAIL as reproduced above |

## Overall verdict

**FAIL — round 1 of at most 2.** E08-S01, E08-S02, and E08-S03 are independently verified and
move to `done`. E08-S04 returns to `in_progress` for the specified repair; E08 remains
`in_progress` with one review round available. No CR4 Safety Verdict is required.
