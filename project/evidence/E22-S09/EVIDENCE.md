# Evidence Packet - E22-S09

- Commit/PR: the packet link rewrite on `main`
- Executor: Claude
- Independent verifier: none - the owner waived the Codex round for this session's work. The failing observation is `check_docs.py check` on the freshly prepared v1.13.1 packet.
- Change Risk: CR3 (the floor `project/risk_floors.json` sets for `scripts/release.py`)
- Spec version/commit: `project/epics/E22.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - root-relative links are rewritten | `relocate_links()` prefixes `../../`, applied to the embedded body in `render_evidence`. The v1.13.1 packet went from three broken targets to four resolving ones (the fourth is in the residual-risks section written for this release). | PASS |
| AC2 - everything else is left alone | The pattern excludes `https?://`, `#`, a leading `/` and an existing `../`. `test_absolute_anchor_and_already_relative_links_are_left_alone` covers all four, and `test_rewriting_is_idempotent` covers the case that matters most - a packet regenerated twice must not accumulate prefixes. | PASS |
| AC3 - every committed packet resolves | `test_every_committed_packet_link_resolves_from_where_it_lives` walks all 14 packets and resolves each link against the packet's own directory. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| n/a | A rewrite that silently breaks a working link | Idempotence and the exclusion set are both tested, and the repository-wide test is the backstop: it fails on any packet, old or new, whose links stop resolving. | PASS |

## Verification Commands

```text
python3 -m pytest tests/test_release.py -q  -> 21 passed, 38 subtests
python3 scripts/check_docs.py check         -> 292 files, links consistent
python3 -m mypy scripts/release.py          -> clean
```

## Compatibility

- Release tooling only. The rewrite affects packets generated from here on; the one already
  generated for v1.13.1 was rewritten in place with the same function.

## Performance / operability

- Not applicable.

## Residual risks

- **The depth is hard-coded.** `../../` is correct because packets live at
  `project/evidence/RELEASE-*.md`. Moving them breaks every rewritten link silently, and nothing
  ties the prefix to the actual path.
- **Only markdown link syntax is rewritten.** A bare path in prose, or an HTML `<a href>`, is
  untouched.

## Verifier verdict

closed on the owner's waiver; no independent verdict
