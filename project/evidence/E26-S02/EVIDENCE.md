# Evidence Packet - E26-S02

- Executor: Claude | Independent verifier: self-review | Change Risk: CR1
- Spec version/commit: `project/epics/E26.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a github: source's pin is compared against upstream, and drift or abandonment reported | **Corrected after review.** The first version fetched only `pushed_at` and printed the pinned string beside a date, which looks like a comparison and is not one - an independent review said so. It now also fetches `releases/latest` and reports `current`, `**behind: upstream X**`, or `latest release could not compare`, alongside the abandonment signal for an upstream untouched for more than a year. | PASS after repair |
| AC2 - a grown capability surface requires a new decision, not an update | The command states it in its own output: a release that adds a hook or an MCP server turns prompt content into software with a shell, and the capabilities must be re-recorded before the pin moves. Enforcement rests on `check`'s required `capabilities` field, which a new decision must restate. | PASS |
| AC3 - with no network it says it could not compare, never that nothing changed | With `gh` absent the command prints "This is not 'everything is current'; it is 'nothing was checked'." A per-component failure prints `could not compare` with the reason. | PASS |
| AC4 - nothing is fetched from a source the manifest does not name | `upstream()` parses only `github:owner/name` from the manifest's own `source` field; there is no other input to the request. `test_an_upstream_is_parsed_only_from_a_github_source` pins it, including a malformed source. | PASS |

## Verification Commands

```text
python3 scripts/check_agent_toolchain.py updates -> 9 comparable upstreams, all pushed 2026-09-09
python3 -m pytest tests/test_governance_extras.py -q -> 27 passed
```

## Compatibility

- Needs `gh` and the network, so it is a **report and never a gate**: a check that needs the
  network cannot be one. It is absent from `.pre-commit-config.yaml` and from both workflows on
  purpose.

## Residual risks

- **A source that publishes no releases reports `latest release unknown`.** The pin comparison is
  only as good as the upstream's release discipline; the push date is the signal that survives a
  project which tags nothing.
- **Capability growth is not detected, only warned about.** Nothing inspects a new release to see
  whether it added a hook; the rule is stated and the human applies it.
- **A silent upstream is ambiguous.** A year without a push can mean abandoned or finished.
- **`gh` authenticates as the developer**, so what this command can see depends on who runs it.

## Verifier verdict

See `project/evidence/E25-E26-SELF-REVIEW-ROUND2.md`.
