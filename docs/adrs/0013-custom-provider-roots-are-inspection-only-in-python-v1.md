# ADR-0013: Custom provider roots are inspection-only in the Python reference

- Status: Accepted
- Date: 2026-08-27
- Decision owners: project owner / cEOS
- Supersedes: [ADR-0012](0012-custom-provider-roots-require-structure-and-intent.md)
- Related: PD-020, C-02, C-03, SI-002, SI-004, SI-009, E00-S02

## Context

This is the third answer to the same question, and the first two were rejected by independent review.

**Attempt 1 - filename markers.** A directory scored `high` confidence because it contained `auth.json` and a `sessions/` folder. Rejected: those names are plausible in unrelated projects.

**Attempt 2 - validated structure plus explicit intent** (ADR-0012). Markers had to contain provider-shaped content, and a non-default root additionally required `--allow-custom-root`. Rejected again, with a counterexample that satisfies both conditions: a directory containing a valid-JSON `auth.json`, a real `config.toml`, and a genuine `rollout-<uuid>.jsonl` under `sessions/`. The reviewer's finding was precise:

> The adopted structure-plus-intent scheme is not positive provider identity and conflicts with SI-002 for acknowledged lookalikes.

The pattern in both rejections is the same. Structure is *evidence that a directory resembles a provider installation*. SI-002 requires *proof that it is one*. No amount of filesystem inspection closes that gap, because everything inspected is forgeable by whatever produced the directory. Adding an operator flag does not supply the missing proof either - it supplies intent, and the failure mode being defended against is an operator whose intent is based on a wrong value.

Positive identity requires asking the provider which directory it owns. That is the provider capability contract, which E05 defines and this reference implementation does not have.

## Decision

In the Python reference, **only the provider's own default directory may be mutated**: `~/.codex` and `~/.claude`. A root reached through `CODEX_HOME` or `CLAUDE_CONFIG_DIR` pointing anywhere else is inspection-only.

- `status` works normally on any root, including full coverage reporting and the structural fingerprint, which remains useful *information*.
- `clean` and `configure` refuse, with a message naming the root, stating whether it structurally resembles the provider, and saying what to do instead.
- `--allow-custom-root` is removed. A switch that cannot make an unsafe operation safe must not exist: keeping it would move responsibility for an unsolved identity problem onto the operator.
- `RootAuthority.structurally_credible` survives as a reported signal and is explicitly documented as non-authoritative.

## Consequences

Positive:

- SI-002 holds without qualification: destructive authority follows from the provider's own configuration default, not from evidence that can be fabricated;
- the rule is one line, testable exhaustively, and has no operator-facing decision that can be made wrongly;
- refusal is visible and non-destructive - exit code 4 with an actionable message.

Costs:

- **This is a capability regression.** An operator who has deliberately relocated a provider root loses `clean` and `configure` entirely until the Rust core ships provider-native identity. The documented workaround is to run without the override.
- The structural fingerprint is now computed for reporting only, which is work that does not gate anything. It is cheap and bounded, and it stays because it tells the operator what cancellAI actually sees.

## Rejected alternatives

- **ADR-0012's structure plus intent:** rejected by independent review with a working counterexample. Recorded rather than deleted, because the reasoning that led there is worth keeping.
- **Provider-native identity now:** the correct answer, unavailable. It requires E05's capability contract, and building an ad-hoc version of it inside the reference would create a second provider-knowledge surface immediately before that reference is frozen.
- **Prompting the operator interactively:** does not work for the automation path, which is exactly where a wrong environment variable does damage unattended.
- **Allowing mutation but restricting it to obviously-provider-shaped subtrees:** narrows the blast radius without answering the identity question, and makes the safety rule harder to state than to violate.
