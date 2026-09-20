# Safety Verdict - E12-S04

Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Verifier: Codex

- Change: Purge tombstone record primitive
- Risk: CR4
- Review target: `f215d33..8afcebc`
- Independent verifier: Codex
- Verifier: Codex
- Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
- Date: 2026-09-20

## Verdict

`FAIL`

## Safety surface changed

The change adds a public persistence path for a record labeled `PURGED`, intended to retain only
minimal contentless evidence after an irreversible action. It does not wire a production purge
to that path yet.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-020 | An irreversible purge is explicit, stronger-gated, and not disguised as cleanup metadata. | The action/reversibility pairing is correctly restricted, but the retained `PURGED` metadata accepts and stores prompt/source/path sentinels through arbitrary strings. | FAIL |
| C-09 | Persistent state must not copy prompts, source code, secrets, or file contents by default. | Independent public-API reproduction stored `PROMPT_SENTINEL_do_not_store_source_contents` and `/private/provider/source.rs` in the tombstone event. | FAIL |

## Adversarial cases

- `Delete + Irreversible` with prompt sentinel in `provider_id`, `reason_code`, `policy_id`,
  `plan_id`, and evidence ID, and an absolute source-like path in `category`: accepted and read
  back unchanged from `EventLedger`.
- The executor's 30 action/reversibility combinations confirm distinguishability, but do not
  examine adversarial values in any retained field.

## Differential / compatibility evidence

- No Python reference behavior is changed.
- Rust format, clippy, check, workspace tests, and cargo-deny all passed; those gates did not
  detect the semantic privacy violation.

## Known residual risks

- Until repaired, any caller can permanently retain arbitrary payload in a purportedly
  contentless purge tombstone.
- A future integration must separately make real purge plus tombstone recording crash/retry safe;
  this unwired primitive cannot prove that end-to-end property.

## Rollback / recovery

Do not invoke or integrate `record_purge_tombstone` until content-safe fields and regressions are
implemented. The story is returned to `in_progress`; no existing provider mutation path is
changed by this review.

## Owner decision

`REJECT`

Owner note: Independent verifier rejection. Repair and re-review are required before a CR4
Safety Verdict may record a passing result.
