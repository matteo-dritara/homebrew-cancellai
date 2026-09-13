# Evidence Packet - E17-S08

- Original executor: Claude; original review target: `05defd4`, `31206c2`, `4c2801b`.
- Independent verifier and owner-authorized repair author: Codex, 2026-09-13.
- Change Risk: CR4. The six lint sites include three in floored crates: WSL, Windows
  process observation, and bundle signature hex decoding.
- Contract: `project/epics/E17.json`; accepted decision:
  [ADR-0026](../../../docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md).

## Outcome

Independent verification in progress. This packet replaces the executor's disproved claims
with measured results. The original implementation and the verifier's repairs are distinguished
below; a final Safety Verdict is separate.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - consistent minimum | Cargo, CI matrix, ADR-0015, ADR-0024, SUPPLY_CHAIN, AGENTS, kernel-guard skill and both source comments agree on 1.88.0. Historical 1.85 references remain explicitly historical. The prior claim that a broad grep returned no matches was false. | PASS |
| AC2 - adopt the dependency and remove its waiver | `ratatui 0.30.2`, `lru 0.18.4`, no `paste` in the lockfile or selected graph; `ignore = []`. Verifier additionally set `unsound = "all"`; the old graph then fails specifically on RUSTSEC-2026-0002, and the new graph passes. | PASS |
| AC3 - satisfy enabled lints | Five `collapsible_if` rewrites (manifest x2, manifest provider, WSL, Windows process) and one parity rewrite. No new lint suppression in the diff. Original source preserves guard order and fallthrough. Verifier added malformed hex/octal regressions after mutations survived the original suite. | PASS |
| AC4 - one terminal-state crate version | Exactly one crossterm (0.29.0) on macOS, Linux and Windows GNU normal graphs. The two-version state described previously was an intermediate upgrade resolution; the committed pre-upgrade graph had one crossterm 0.28.1. | PASS |
| AC5 - rejected branch not taken | The owner accepted ADR-0026. No advisory-risk acceptance or Dependabot dismissal is claimed. | PASS |
| AC6 - strict verification rejects weak keys/signatures | Original permissive `verify` accepted a synthetic forged bundle under a weak local-policy key, contrary to ADR-0024. Repair `28c29a7` uses `verify_strict`; the regression fails on the original call and preserves all current-store fields on refusal. | PASS |

## Safety Evidence

- SI-017: WSL digit validation still gates byte parsing; `i += 4` and `continue` remain inside
  successful decoding. Tests now span all 512 three-digit octal strings, overflow, plus/minus
  signs, Unicode and truncated/mixed escapes. Windows name matching/deduplication was also
  tested using the exact extracted production function with synthetic enumeration results;
  this is a logic probe, not a native Windows runtime test.
- SI-021: empty environment names and escaping subdirectories still refuse with the original
  error variants. Provider matching remains guarded by `strip_prefix`; the tree walker and
  trust/capability ceiling are unchanged.
- SI-022/SI-029: parity and ASCII checks still precede byte slicing. Seven malformed public
  signature cases, every byte in lower/upper hex, and odd/non-ASCII decoder input are tested.
  The weak-key defect was a pre-existing cryptographic contract violation, not a result of
  the parity rewrite; the owner-authorized repair deliberately narrows signature acceptance.
- No new package reaches the kernel ring through the upgrade. For every introduced
  name/version pair, reverse trees were queried using normal/build edges across all targets.
  Each selected new pair reaches workspace consumers only via cancellai-tui. Forward trees
  of model, safety, platform and sealedfs are identical before/after, including build edges.

## Dependency measurements

Compare `31206c2^` with `4c2801b`, locked/offline, default features. Count unique nonblank
package identities from `cargo tree --workspace --edges normal --prefix none --format '{p}'`.
These are selected normal-dependency packages, including 13 workspace packages, not all rustc
compilation units; build/dev edges are excluded from this particular count.

| Target | Before | After |
| --- | --- | --- |
| aarch64-apple-darwin | 98 | 116 |
| x86_64-unknown-linux-gnu | 100 | 118 |
| x86_64-pc-windows-gnu | 98 | 115 |

Lockfile package entries: 123 -> 225, net +102. Package-identity delta: 127 introduced
name/version pairs and 25 removed. Of those 127 new pairs, 33 are selected by normal/build
edges over all targets and 94 are unselected. The old 99 -> 117 and “grew by 115 entries”
claims were incorrect. The textual diff itself adds 122 `name` lines and removes 20; textual hunk matching is not
a package-set count either.

`hashbrown` 0.16.1/0.17.1 remain selected in rendering/cache paths. `syn` 2.0.119/3.0.4 remain
for distinct proc-macro dependencies; the 3.x path predates this story via the reviewed
cryptographic dependency. They increase build cost, but neither duplicates terminal state.
Cargo-deny warns on duplicate versions under the existing policy; it does not ban them.

## Advisory coverage correction

GHSA-rhfx-m35p-ff5j aliases
[RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002.html), patched in lru
0.16.3. Cargo-deny 0.20.2 defaults the unsoundness scope to direct workspace dependencies;
its omission of transitive lru was not evidence that RustSec lacked the advisory. Reproduced:
old configuration + old graph passes, `unsound = "all"` + old graph fails on that advisory,
and the stronger policy + new graph passes. No component was installed or updated during review.

## Compatibility and rendering

Eight private TUI signatures, not four, gained `border::Set<'static>`. All callers pass either
static border constants or string literals; no public borrowing API became over-constrained.
The functions may be generalized if dynamic borders are introduced, but no such input exists now.

The verifier rendered 80 populated screen/size/Unicode/help combinations against TestBackend
on ratatui 0.29.0 and 0.30.2. Every character buffer matched. The four populated 100x30 screens
were visually inspected; borders, incomplete-scan warning, explanations and irreversible plan
prompt remain readable. These checks do not verify colors, terminal escape transport, or live
Windows/Linux raw-mode restoration. Existing navigation/render tests also pass.

## Verification Commands

The verifier ran the entire AGENTS Python command list and all seven requested Rust commands.
Baseline pytest: 483 passed plus 444 subtests. System Python lacked ruff/mypy; the already
installed repository virtual environment supplied them and passed. Rust fmt/clippy/check/test
and both cross-target clippy runs passed. Cargo-deny initially could not lock its read-only
sandboxed database, then passed with the required filesystem permission. Final rerun results
are recorded in the separate verifier review, rather than treating these baseline results as
proof of the repairs.

## Mutation evidence

- Original suite caught both expiry-boundary `>=` -> `>` mutants, dropping the publisher
  guard, inverted hex parity, disabled empty-env and subdir guards, disabled glob matching,
  WSL advance-by-three and missing `continue`.
- Original suite missed deleted hex parity and deleted octal-digit validation. Regressions in
  `5c36b3b` catch both. Reverting strict signature verification also fails its new regression.
- Swapping the two pure publisher/sequence comparisons survives legitimately: both are
  total, side-effect-free comparisons, so the truth value is unchanged. Dropping a guard is
  materially different from swapping them.
- Windows-only dedup mutation survives macOS cargo tests because that branch is not compiled.
  The exact-function synthetic harness catches both deleted deduplication and disabled matching.

## Residual risks

- No 1.88 toolchain is installed locally. The original target's nine Rust CI jobs passed,
  including 1.88.0 checks on macOS/Linux/Windows. Fresh CI must cover the repaired revision.
- Native Windows process-mutation execution and live Windows/Linux TUI testing remain unrun
  locally. Cross-target clippy verifies compilation/lints, not runtime behavior.
- E17-S11 records the unrelated toolchain-report false positive (approved members are omitted
  from report's managed set). The enforcing check is correct; no tooling approval was missing.
- RustSec/GitHub coverage is not a proof against undisclosed vulnerabilities. The now-explicit
  transitive unsoundness policy closes this observed omission.
- A dependency rollback needs a reviewed compatible graph; do not restore vulnerable lru/paste
  or weaken strict verification just to restore an older MSRV. No data migration is involved.

## Safety Verdict

Independent verdict and final gate evidence are issued in the story-scoped review and
`SAFETY_VERDICT.md` after the final rerun. Neither epic is closed by this review.
