# Safety Verdict - E16-S02

- Change: signed knowledge-bundle parsing, verification, anti-replay, and rollback
- Risk: CR4
- Commit/PR: `4d19ac62b864a12f6599b9875053bd6ccf7c6796..8696d7aeae9e82a76fefc1941702a5bfba06dc04`
- Independent verifier: Codex
- Date: 2026-09-11

## Verdict

`PASS`

## Safety surface changed

The change accepts externally supplied provider knowledge only after local-publisher lookup,
content-digest verification, Ed25519 signature verification, expiry validation, and per-publisher
sequence validation. It also keeps one prior verified bundle for fail-closed rollback.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Bundle content cannot supply executable authority or elevate its tier. | `KnowledgeBundle` has no tier/capability/authority field; `parse_bundle` denies unknown fields; `verify_bundle` copies `TrustedPublisher.tier` only from `LocalTrustPolicy`. `TrustedTier`’s external-forgery compile-fail doctest passed. | PASS |
| SI-029 | Invalid, expired, replayed, or unauthorized updates fail closed and failed rollback preserves offline inspection. | Direct source walk verified signature, digest, expiry, and strict same-publisher sequence checks precede install. `rollback` checks `previous` expiry before `take()`, leaving `current` untouched on `Expired`/`NoPriorBundle`. | PASS |

## Adversarial cases

- Missing signature, unknown fields, malformed signature, unknown signer, digest tampering, and signature tampering are refused by the parser/verifier path.
- A malformed JSON bundle cannot smuggle tier provenance: its only possible tier source is the local policy’s opaque `TrustedTier` value.
- The signing record is injective: every ordered field is prefixed by an eight-byte length and `expires_at` adds a distinct presence byte, so different publisher-id/payload field splits cannot share signing bytes.
- An expired prior bundle returns `Expired` before `previous.take()`, preserving the newer current bundle.
- `cargo tree -p cancellai-safety -i ed25519-dalek --edges features` reported only `fast` and `zeroize`; no signing or `rand_core` feature reaches the shipped dependency graph.

## Differential / compatibility evidence

- `cargo test --workspace` passed, including the signed-bundle tamper, expiry, replay, and rollback suite.
- The pre-existing `release_manifest.schema.json` is byte-identical at the E16 range endpoints and E17-S01 predates the E16 range. The E16-S02 implementation does not import or require E17-S01’s status.

## Known residual risks

- This story supplies a verification and in-memory rollback primitive, not a network updater or persistent bundle store. Those callers remain responsible for supplying a clock and local trust policy.

## Rollback / recovery

- A rejected update leaves the installed bundle unchanged. An accepted replacement can restore its still-valid predecessor through `KnowledgeStore::rollback`; an expired predecessor is deliberately refused without clearing current state.

## Owner decision

`ACCEPT`

Owner note: Independent verifier recommends acceptance; no unresolved HIGH or CRITICAL safety residual was found in this story’s boundary.
