# Architecture: As-Is Python v1

Baseline analyzed: repository `matteo-dritara/homebrew-cancellai`, attachment at commit `4b2df0130e62d83e3a10caaae73daa456211f92d` on 2026-08-27.

## Current shape

The shipping implementation is a single stdlib-only Python CLI. This was an appropriate v1 distribution choice, but it is now a **reference implementation**, not a permanent target architecture.

### Pipeline

Every command follows the same three stages:

```text
discover_*()   -->   build_plan()   -->   execute_plan()
(scan disk)          (decide what is      (the only stage that
                      safe to delete;      deletes anything, and
                      no writes)           only with dry_run=False)
```

- `discover_codex_sessions` / `discover_claude_sessions` / `discover_*_aux` walk `~/.codex` and `~/.claude` (or `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR`) and return `Action` records: candidate path, size, mtime, and where known the session id. Discovery never deletes.
- `build_plan` applies the age cutoff (`--days`), the keep-latest rail (`--keep-latest`), the Codex subagent graph, and the protected-name barrier. The result is a `Plan`: pure data, nothing touched on disk.
- `execute_plan` is the only function that mutates. It re-checks running providers, deletes through `safe_remove`, and trims `history.jsonl` only for sessions that were *actually* deleted.

### The safety-critical core

Seven things constitute the security boundary. Any change to them needs matching test coverage before it merges, not after:

1. **`validate_config_root`** - refuses to operate when `$CODEX_HOME` / `$CLAUDE_CONFIG_DIR` resolves to `/`, the user's home, or anything shallower than a few path segments.
2. **`fingerprint_root` / `RootAuthority`** - only the provider's own default directory may be mutated. The structural fingerprint survives as *reported information* - it tells the operator what cancellAI sees - and is explicitly non-authoritative: nothing observable in a filesystem proves a directory belongs to a provider, and two schemes that assumed otherwise were rejected by independent review. Any relocated root is fully inspectable and never mutated. See [ADR-0013](../adrs/0013-custom-provider-roots-are-inspection-only-in-python-v1.md).
3. **`Scan` / `observe`** - the completeness channel. Size and mtime helpers answer with numbers that cannot express "I could not look", so unreadable paths are recorded separately. Every discovery guard goes through `observe()` rather than `Path.exists()`, which answers False for both "absent" and "unreadable" and would reintroduce the collapse at the guard. An incomplete scope withholds destructive authority for that tool. A path that vanished mid-scan is recorded as a race, not as blindness, and the error list is bounded so a governance tool cannot become an unbounded log producer.
4. **`safe_remove`** - the only function allowed to call `unlink`/`rmtree`. It never follows a symlink outside the approved root and re-resolves the path immediately before deleting, so a filesystem change between planning and acting is caught at the last possible moment rather than trusted from stale information.
5. **`protected_component`** - the executable form of `CLAUDE_PROTECTED_NAMES` / `CODEX_PROTECTED_NAMES`. Applied twice: when the plan is assembled and again inside `safe_remove`. The name is checked both lexically and after resolution, so a protected entry that is itself a symlink keeps its protection instead of falling out of the relative-path computation. Comparison uses the Unicode canonical caseless form (UAX #15: NFD, casefold, NFD), because APFS is case-insensitive *and* stores decomposed filenames - folding alone compared `plügins` and its decomposed spelling as different names. Over-inclusive is the safe direction here. `--aggressive` widens which categories are eligible; it can never reach a protected name.
6. **`active_processes`** - reports *whether it could observe* provider activity separately from what it found. An unusable `ps` marks the observation incomplete, and unknown activity blocks mutation exactly as a detected running provider does.
7. **`normalize_argv`** - destructive intent must be typed. A leading flag resolves to the read-only `status` view; an unknown verb is a usage error.

### Data model

- **`Action`** - one candidate deletion: tool, category (`session`, `old-log`, `file-history`, ...), path, size, mtime, and for Codex the session/parent-session id used to resolve the subagent graph.
- **`Plan`** - the output of `build_plan`: selected actions plus bookkeeping (cutoff, notes about refused or skipped work, which `history.jsonl` lines are linked to selected sessions).
- **`CleanResult`** - what actually happened: succeeded/failed/skipped counts, blocked tools, deferred work, bytes freed, and which Claude session ids were really deleted.
- **`CoverageBucket`** - how much of a provider root this build can classify at all, including the entries it cannot.
- **`RootAuthority`** - a root's origin (`default`/`custom`), the provider markers found in it, its confidence, and whether it may be mutated. Canonical name: [`ProviderRoot`](DOMAIN_MODEL.md#providerroot); see [Legacy vocabulary](DOMAIN_MODEL.md#legacy-vocabulary).
- **`Scan`** - one discovery scope's completeness plus the paths that could not be read.
- **`ProcessObservation`** - which provider processes were found, and whether the enumeration worked at all.

### Codex subagent graph

Codex rollouts reference a `parent_thread_id`. `choose_codex_old_sessions` walks that graph so that `--keep-latest` counts root session trees rather than individual subagent files, the official `codex delete --force` backend receives one action per root (Codex cascades), and the explicit filesystem fallback removes every rollout of a selected tree individually (raw unlinking does not cascade).

### Exit taxonomy

| Code | Meaning |
| --- | --- |
| 0 | requested work completed |
| 1 | user declined the confirmation prompt |
| 2 | invalid usage, or a refused/invalid configuration root |
| 3 | at least one mutation failed |
| 4 | safety blocked, withheld or deferred requested work; nothing may be assumed cleaned |

Exit 4 covers a live provider process, provider activity that could not be determined, a non-default root, a root that lost its authority between planning and execution, an incomplete scan, and a deferred or failed `history.jsonl` trim.

The taxonomy is total: `main()` converts any escaping exception into exit 3. Leaving one uncaught would surrender the process to Python's own exit code 1, which automation cannot distinguish from the operator declining the confirmation prompt.

### Where things live

| Path | What |
| --- | --- |
| `cancellai.py` | The entire v1 implementation. |
| `tests/test_cancellai.py` | Behavioral and trust-floor regression tests. |
| `scripts/gen_docs.py` | Regenerates `docs/CLI.md` from the real argparse definitions. |
| `Formula/cancellai.rb` | The Homebrew formula (this repo doubles as its own tap). |

Supporting assets:

- GitHub Actions - test/lint/type/docs/governance/Homebrew validation.

## Strengths to preserve

- discovery/planning/execution separation;
- read-only default intent;
- dry-run and explicit confirmation;
- preference for vendor-native Codex deletion;
- subagent tree awareness;
- symlink non-following intent;
- atomic JSON configuration writes;
- protected Claude auto-memory intent;
- standard-library runtime and inspectability;
- generated CLI documentation;
- small public release surface.

## Verified P0 defects

The baseline code review in [`../audits/2026-08-27-CODE_REVIEW.md`](../audits/2026-08-27-CODE_REVIEW.md) found defects that must be repaired before the Python reference is frozen.

| Defect | Story | State |
| --- | --- | --- |
**E00 is closed.** Three independent review rounds were performed before closure; the fourth was waived by the owner, who accepted the residual risk explicitly. Each CR4 story carries an owner acceptance recording that decision beside the reviewer's earlier verdicts, which are retained unaltered.

| Story | Round 1 | Round 2 | Round 3 | State |
| --- | --- | --- | --- | --- |
| E00-S01 protected-name barrier (CR4) | FAIL - protected symlink escaped | FAIL - case variant `Plugins` escaped | FAIL - Unicode form escaped | done |
| E00-S02 provider-root authority (CR4) | FAIL - filenames treated as identity | FAIL - validated lookalike accepted | PASS | done, ADR-0013 |
| E00-S03 aggressive retention (CR3) | PASS | - | - | done |
| E00-S04 CLI and exit taxonomy (CR3) | FAIL - refusal escaped as an exception | FAIL - `OSError` still escaped | PASS | done |
| E00-S05 scan completeness (CR4) | FAIL - three helpers swallowed errors | FAIL - `exists()` guards collapsed errors | PASS | done |
| E00-S06 concurrent metadata rewrite (CR3) | FAIL - CRLF bytes normalised | FAIL - symlink followed and replaced | PASS | done |
| E00-S08 coverage vocabulary (CR1) | new | FAIL - container labelled cleanable | PASS | done |
| E00-S09 activity observation (CR4) | new | FAIL - unrelated `ps` line accepted | PASS | done |

The pattern across the rounds is the most useful thing this epic produced. Every round-1 repair closed the reported *instance* and left the defect *class* open; round 2 was scoped to falsify classes and found one in each story; round 3 found one more, in the story that had already been repaired twice. Rejection counts fell 6/7, then 7/7, then 1/7.

Two consequences are permanent. `docs/development/WORK_ITEM_MODEL.md` makes `ready_for_review` the executor's exit state, so an implementer never grades its own work. And `scripts/project_os.py` refuses a handoff without evidence and a CR4 closure without a verdict that records a pass - the gate that made the owner's acceptance an explicit, committed decision rather than a silent status change.

E00-S08 and E00-S09 were opened during remediation rather than folded into an existing story.

Provider layout drift is tracked separately by E00-S08: `status --coverage` classifies every top-level provider entry and names the ones this build does not understand. Unknown entries are reported, never cleaned.

### P0-01 Protected-name constants are not an executable barrier

`CLAUDE_PROTECTED_NAMES` and `CODEX_PROTECTED_NAMES` are documented as load-bearing, but the mutation path does not consult them. Their protection currently depends on scanners never emitting those paths.

Required correction: enforce protection both during candidate construction and immediately before mutation.

### P0-02 Custom provider roots are insufficiently fingerprinted

`validate_config_root()` rejects `/`, `$HOME`, and shallow paths but cannot distinguish a real provider root from an ordinary directory that happens to contain `tmp/` or `log/`.

Required correction: provider-root fingerprinting. Unknown custom roots remain inspectable but non-destructive.

### P0-03 Aggressive mode can bypass retention semantics

Some legacy/cache aggressive candidates are added as whole roots/files without applying the age cutoff consistently.

Required correction: aggressive broadens categories only; age policy remains independent.

### P0-04 CLI normalization can imply destructive intent

`normalize_argv()` converts otherwise unrecognized first arguments into `clean`, so `cancellai --days 14` can mean clean rather than an explicit read-only operation/error.

Required correction: destructive behavior requires an explicit `clean` verb.

### P0-05 Incomplete scans can collapse into zero/empty facts

Several filesystem helpers convert permission/I/O failures into `0` or empty results. This is acceptable for a best-effort analyzer only if completeness is carried separately. It is unsafe if absence of evidence becomes deletion eligibility.

Required correction: COMPLETE/PARTIAL/UNKNOWN inventory status propagated into authority.

### P0-06 Blocked cleanup can still look operationally successful

A running provider may cause actions to be skipped while the process-level return code remains success when no individual mutation failed.

Required correction: stable exit taxonomy distinguishes success, safety-blocked/partial, mutation failure, compatibility failure, and invalid usage.

### P0-07 Concurrent history rewrite risk

Claude history trimming rewrites a shared mutable file. `--allow-running` can allow a concurrent provider writer and therefore risks lost updates.

Required correction: stream rewrite; never rewrite shared provider metadata while active even if cleanup of independent artifacts is allowed.

## P1/P2 risks intentionally deferred to the Rust safety kernel

These are not justification to expand the Python monolith. They are requirements for the target core:

- sealed identity-bound plans and stronger TOCTOU defense;
- mount/volume/reparse boundaries;
- cross-platform object identity;
- single-pass inventory and allocated/reclaimable accounting;
- typed capability/authority lattice;
- provider layout/version confidence;
- local event ledger and reversible lifecycle operations.

## Freeze rule

After E01 completes, Python v1 accepts only:

- critical safety patches;
- test/fixture work required to preserve the oracle;
- changes necessary to keep the reference runnable during migration.

New product capability belongs in Rust.
