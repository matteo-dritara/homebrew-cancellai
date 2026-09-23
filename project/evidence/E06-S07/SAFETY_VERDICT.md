# Safety Verdict - E06-S07

Verifier: Codex
Brief-Checksum: 109e9df668259845181d2c5e79b6ffad84a4575702665fd3e292b266f5905350

- Reviewed commit: `85fed2f7c66baf9da3ed79f1bb870f4277eec434`; risk CR4.
- Surface: compiled channel, signed containment trust, persistent replay and the per-deletion authority recheck.

## Verdict

`PASS_WITH_RESIDUALS`

## Invariants

| Invariant | Evidence | Result |
| --- | --- | --- |
| SI-019 | `plan_actions` applies computed authority; `clean` seals at that level and reloads containment before each `delete_one`. Mutation still routes through the one safety executor; static boundary check passed. | PASS |
| SI-022 | Raw signed bundle text is stored. Every load calls kernel replay to verify digest, signature, publisher and sequence; the trust policy is compiled project key plus owner trust file. Notice schema has no lift or trust promotion field. | PASS |
| SI-029 | Oversized, malformed, unsigned, forged, untrusted, replayed and expired install cases refuse without an append; a corrupt/torn log is unknown and caps deletion. Replay alone skips expiry so an installed containment persists. Same-user edits to owner state can lift it, as explicitly accepted. | PASS_WITH_RESIDUALS |
| SI-030 | `BuildChannel::from_compiled_env` is the production input; a missing/unrecognized compile-time channel caps at Recommend. Stable CLI tests exercise real deletion, while ordinary nightly builds withhold it. | PASS |

## Adversarial cases

- Traced `read_notice`: `File::take(MAX_NOTICE_BYTES + 1)` checks size before UTF-8 decoding or parsing. `ContainmentLedger::install` repeats the cap before parsing. A rejected install does not call `append_event`.
- Traced trust-file failure: missing means compiled key only; unreadable, malformed, duplicate or reserved publisher makes the entire ledger unknown. Notice fields cannot add a publisher, a lift or a higher ceiling.
- Traced log replay: signed installs are verified on each read and per-publisher sequence must increase. An expired *stored* bundle still binds. Torn trailing line, over-cap history and invalid event fail the whole replay, not just the final event.
- Verified the three design-review findings in code: state history appends rather than read-modify-write; `clean` reloads the history before each deletion; an action with no classified target is withheld. The owner-state limitation is recorded in ADR-0039, SI-029, `INCIDENT_RESPONSE.md` and the store implementation. This does not claim atomic serialization of an install after the final recheck with an already-running delete.
- CI on the exact reviewed SHA passed stable-channel CLI tests on macOS, Linux and Windows. Native same-user tamper is an accepted limit, not treated as a signed-notice capability.

## Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy/tests, Windows-target Clippy | PASS |
| Stable-channel CLI suite and containment integration tests | PASS on full rerun |
| `cargo deny --offline check` with writable advisory DB copy | PASS |
| Mutation-boundary, project OS and verifier handoff checks | PASS |
| Native concurrent-install interleaving after final recheck | NOT RUN; timing window is recorded as a residual |

## Residual risks

- An actor already running as the owner can delete, truncate to a valid prefix, or append a local lift to the history. The owner accepted precisely this state boundary on 2026-09-23; no stronger anti-tamper claim is made.
- A notice installed after `clean`'s last ledger recheck can race the final syscall. The next deletion rechecks; instant containment of an in-flight syscall is not established.

### Owner decision

PENDING

The prior owner-state disposition is recorded; owner acceptance of this CR4 review remains separate.

PASS_WITH_RESIDUALS
