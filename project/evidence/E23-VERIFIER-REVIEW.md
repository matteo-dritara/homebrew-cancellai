# E23 Independent Verifier Review - Round 1

- Epic: E23 - Release gate history availability
- Review target: `21d4379..dbb1ae9`
- Verifier: Codex (`/root`)
- Date: 2026-09-08

E23-S01 was `ready_for_review` before this review began. It is the sole story in E23.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E23-S01 | FAIL | The release workflow now uses `fetch-depth: 0`, and the original shallow-clone failure is independently reproducible. However, `scripts/check_workflows.py::release_history_gate_errors()` examines only the first `actions/checkout` step. A valid workflow that checks out full history at `path: full-history-copy`, then performs a second default shallow checkout in the workspace where `python3 scripts/check_platforms.py check` runs, returns `[]` from the checker. The provenance check consequently remains vulnerable to a multi-checkout workflow regression. |

## Reproduction and required repair

The verifier evaluated this workflow through `release_history_gate_errors()` with its temporary
workflow path:

```yaml
jobs:
  verify:
    steps:
      - uses: actions/checkout@<40-hex-sha>
        with:
          fetch-depth: 0
          path: full-history-copy
      - uses: actions/checkout@<40-hex-sha>
      - run: python3 scripts/check_platforms.py check
```

It returned `[]`. The second checkout is the default workspace checkout and defaults to depth
one; the full-history clone is isolated under `full-history-copy`, so the provenance command
still fails exactly as it did at v1.10.0.

Required repair: make the static check associate the full-history checkout with the workspace
that executes `check_platforms.py`, and reject later/default-workspace shallow checkouts before
that gate. Add adversarial regression coverage for the separate-path full checkout plus shallow
default checkout and for any supported multiple-checkout arrangement. Do not weaken
`scripts/check_platforms.py`'s ancestor validation.

This violates E23-S01 AC3 and SI-019's release-verification boundary. AC1 and AC2 pass for the
current simple workflow. AC4 is correctly deferred until a passing review closes E23 and the
verifier/owner cuts a replacement tag; it is not the cause of this rejection.

## Gates run

| Gate | Result |
| --- | --- |
| `python3 scripts/project_os.py check`, `review`, verifier brief | PASS before review; E23-S01 was queued and ready. |
| Real shallow clone of `v1.10.0` + `scripts/check_platforms.py check` | PASS as a reproduction of the historical failure: all three historical verified commits were absent. |
| Adversarial multi-checkout policy probe | FAIL: checker returned `[]` for the bypass above. |
| Python tests | PASS — 197 tests. |
| Ruff, format, mypy | PASS. |
| Rust fmt, clippy, check, test, deny | PASS. |
| Documentation, workflow, fixture, schema, characterization, differential, workspace, mutation-boundary, provider-compatibility, platform, process, and release checks | PASS. |

## Overall verdict

**FAIL — review round 1 of at most 2.** E23-S01 returns to `in_progress`. No release tag is cut; the `v1.10.0` release remains unpublished.
