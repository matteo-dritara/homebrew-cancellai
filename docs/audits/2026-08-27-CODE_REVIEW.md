# Code Review Baseline - 2026-08-27

## Scope

Reviewed the attached `cancellai.zip` and compared it with the public repository `https://github.com/matteo-dritara/homebrew-cancellai` at baseline commit `4b2df0130e62d83e3a10caaae73daa456211f92d`.

Review dimensions:

- destructive safety;
- correctness and failure semantics;
- concurrency/TOCTOU;
- provider compatibility;
- performance;
- testability;
- packaging/release;
- documentation consistency;
- suitability as a foundation for the product roadmap.

The original suite compiled and its 18 tests passed during the baseline review. The review did not treat passing tests as proof of safety; several defects below were reproduced through code-level scenarios.

This orchestration baseline intentionally does **not** patch the P0 runtime defects. The owner requested that application implementation be performed later by the executor/verifier agents. The defects are converted into E00 story contracts with Acceptance Criteria, Safety Invariants, and adversarial verification requirements so they cannot disappear into prose.

## Executive assessment

The v1 is a strong prototype/foundation: conservative intent, small inspectable runtime, discovery/plan/execute separation, generated CLI docs, CI and Homebrew packaging. It should **not** be discarded before its behavior becomes a reference contract.

It should also **not** become the long-term architecture. The main reason is not Python performance by itself; the product now requires formal authority, provider capability/trust, cross-platform identity semantics, persistent lifecycle, TUI/Guardian, and differential verification. These concerns would turn the one-file architecture into a high-risk monolith.

## P0 findings

### CR-P0-01 - Protected constants are documented but not enforced

Severity: critical trust defect.

`CLAUDE_PROTECTED_NAMES` and `CODEX_PROTECTED_NAMES` are described as unconditional barriers, but references in `cancellai.py` show they are not consulted by planning/execution. Safety currently depends on scanners not producing those paths.

Impact: a future scanner expansion can invalidate a documented invariant without touching the named protection lists.

Required work: E00-S01.

### CR-P0-02 - Config-root validation lacks provider identity

Severity: critical destructive-boundary defect.

`validate_config_root()` rejects obvious catastrophic roots but accepts an ordinary sufficiently deep directory. If it contains names such as `tmp`/`log`, provider auxiliary discovery can treat them as candidates.

Impact: an environment misconfiguration could convert unrelated project files into provider cleanup candidates.

Required work: E00-S02.

### CR-P0-03 - Aggressive category expansion can ignore age cutoff

Severity: high contract defect.

Claude aggressive legacy roots and selected cache files are appended without consistently applying cutoff filtering.

Impact: `--days` no longer means what the user expects for all aggressive artifacts.

Required work: E00-S03.

### CR-P0-04 - Non-subcommand flags can normalize to `clean`

Severity: high privilege/UX defect.

`normalize_argv()` prepends `clean` when the first argument is not a known command. This contradicts the stronger principle that destructive intent must be explicit.

Required work: E00-S04.

### CR-P0-05 - Filesystem observation errors can collapse to empty/zero

Severity: critical safety-model defect.

Helpers such as size/traversal functions return zero/empty values on permission/I/O errors. Without a separate completeness channel, unknown can be confused with absent.

Required work: E00-S05.

### CR-P0-06 - Safety-blocked work can still return success

Severity: high automation defect.

`execute_plan()` tracks skipped actions when providers appear active, but `cmd_clean()` returns non-zero only for `failed`, not material safety skips.

Impact: cron/automation can believe requested cleanup succeeded when it did not execute.

Required work: E00-S04.

### CR-P0-07 - Concurrent Claude history rewrite

Severity: high integrity defect.

History trimming is an atomic rewrite, which is good, but an allow-running execution can race a concurrently writing Claude process and lose new lines.

Required work: E00-S06.

## P1/P2 findings and target-architecture implications

### TOCTOU remains path-centric

`safe_remove()` re-resolves at execution, which is better than trusting discovery, but it does not bind a plan to a filesystem object identity. A target can be swapped between observations.

Target: E03 Artifact identity + Sealed Plan.

### Mount/volume boundaries are not modeled

`shutil.rmtree` and path containment do not constitute a complete mount/reparse policy.

Target: E03/E07 platform boundary capabilities.

### Repeated filesystem traversal

Status/build/report operations can recursively walk the same tree multiple times for plan sizing, root sizes and top entries.

Target: E04 single-pass inventory.

### Logical size is not guaranteed reclaimable size

Summing `st_size` does not model APFS clones/sparse/shared blocks or equivalent filesystems.

Target: E10 reclaimability estimator with explicit uncertainty.

### Process detection is best-effort

Exact process-name matching is a useful signal but cannot prove absence of a writer.

Target: treat activity as evidence, not sole authority; combine provider/native/file identity signals where available.

### Capability detection executes provider CLI

Status can invoke Codex capability detection. This is not necessarily unsafe, but it means "read-only" does not mean "filesystem reads only" and should be represented as a provider observation capability.

Target: E05 provider capability contract.

### Current docs overstate vendor cleanup absence

README says provider data is never automatically cleaned. Claude Code now has built-in retention and other vendors are adding lifecycle commands.

Impact: product positioning and docs can become stale quickly.

Target: reposition cancellAI above vendor-native cleanup and maintain a current research/compatibility layer.

### Single-file architecture is at its natural limit

The v1 design optimized auditability and Homebrew simplicity. Target needs separate domains for inventory, provider knowledge, policy, safety, persistence, platform, and clients.

Target: E02 Rust workspace after reference contract freeze.

## Positive engineering findings

- safe-by-default product intent is clear;
- official Codex deletion is preferred over raw session unlinking;
- Codex parent/subagent relationship is handled explicitly;
- symlink traversal is deliberately avoided;
- Claude settings writes are atomic;
- no runtime dependencies reduce v1 supply-chain surface;
- generated CLI docs reduce command/documentation drift;
- CI checks test/lint/type/docs/Homebrew;
- changelog and release runbook exist;
- AGENTS/CLAUDE instructions already recognize coding-agent development.

## Recommendation

Do not start Rust feature development before E00 and E01. Do not spend time extensively refactoring the Python monolith first. Repair the trust floor, extract its normative behavior into synthetic fixtures and versioned contracts, freeze it, and then build the target core against an independent oracle.
