# ADR-0012: Custom provider roots require validated structure and explicit intent

- Status: Superseded by [ADR-0013](0013-custom-provider-roots-are-inspection-only-in-python-v1.md)
- Date: 2026-08-27
- Decision owners: project owner / cEOS
- Related: PD-002, PD-018, C-02, C-03, SI-002, SI-004, SI-009, E00-S02

> **Superseded.** Independent review produced a counterexample satisfying both conditions adopted here: a directory with a valid-JSON `auth.json`, a real `config.toml` and a genuine `rollout-<uuid>.jsonl`. Structure plus intent is not positive provider identity. [ADR-0013](0013-custom-provider-roots-are-inspection-only-in-python-v1.md) replaces it.

## Context

`validate_config_root()` rejects catastrophically broad roots (`/`, `$HOME`, very shallow paths) but cannot tell a provider installation from an ordinary directory. The 2026-08-27 baseline review filed this as CR-P0-02: an environment where `CODEX_HOME` or `CLAUDE_CONFIG_DIR` points at a project directory can turn unrelated files into cleanup candidates.

The first remediation attempt scored a root by the **presence of provider filenames**. Independent review rejected it: a directory containing `auth.json` and a `sessions/` tree earned `high` confidence, and those names are entirely plausible in unrelated projects. The reviewer escalated the underlying question rather than the bug:

> Robust custom-root authentication cannot safely rest on plausible provider filenames. Decide whether destructive custom roots require provider-native identity, a user-confirmed capability token, or are disabled in Python v1.

Three options were considered:

1. strengthen the filename heuristic;
2. require provider-native identity (ask the provider CLI which directory it owns);
3. require explicit operator intent for any non-default root.

Option 1 does not change the class of the problem. Option 2 is the right long-term answer but depends on a provider capability contract that does not exist yet (E05) and would make the Python reference depend on invoking provider binaries to decide deletion authority - which is itself an authority question (C-01).

## Decision

A non-default provider root is mutated only when **two independent conditions** both hold:

1. **Validated structure.** The directory contains at least one *identifying* marker plus a second marker, where an identifying marker must contain provider-shaped **content**, not merely a matching filename:
   - Codex: `auth.json` parsing as a JSON object, `session_index.jsonl` containing JSON objects, a non-empty `installation_id`, or a `sessions/` tree containing a real `rollout-<uuid>.jsonl`.
   - Claude: `settings.json` or `keybindings.json` parsing as a JSON object, or a `projects/` tree containing a real `<uuid>.jsonl` transcript.
2. **Explicit intent.** The operator passes `--allow-custom-root`.

The default roots (`~/.codex`, `~/.claude`) are authoritative by definition and carry their own intent, including on a fresh machine where they are empty or absent.

Neither condition is sufficient alone. Structure answers *is this really the provider*; intent answers *did an operator mean to point us here*. Inspection (`status`) is never gated by either.

`configure` writes provider configuration and is a mutation, so it routes through the same boundary.

## Consequences

Positive:

- a misconfigured environment variable can no longer convert unrelated files into cleanup candidates, regardless of which filenames they happen to use;
- the failure mode is visible and non-destructive: exit code 4 with a message naming the missing condition;
- structural validation is bounded (`MAX_ROOT_PROBE_ENTRIES`), so fingerprinting an untrusted directory cannot become an unbounded walk;
- the two conditions are independently testable and independently reviewable.

Costs:

- operators who deliberately relocate a provider root must add `--allow-custom-root` to scripts and cron entries. This is a breaking change and is recorded as such in the changelog;
- an exotic but genuine layout with no validated marker is refused rather than cleaned. That is the intended direction of failure (C-02, C-03);
- marker content validation reads small files during fingerprinting, before authority is granted. Reads are size-capped and never follow symlinks.

## Rejected alternatives

- **Stronger filename heuristics only:** rejected by independent review; filenames are not identity.
- **Provider-native identity in Python v1:** correct target, blocked on the provider capability contract (E05). Revisit for the Rust core; this ADR does not close that door.
- **Disable destructive work on custom roots entirely:** safest, but silently breaks a legitimate documented configuration with no path forward. The explicit flag preserves the capability while making the risk the operator's stated choice.
- **Interactive confirmation instead of a flag:** does not work for the automation path, which is exactly where a misconfigured root is most dangerous.
