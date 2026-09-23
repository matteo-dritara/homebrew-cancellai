Review-Scope: epic
Round: 7
Verifier: Codex

# Independent verifier review - 2026-09-24

Second and final owner-authorized independent pass for E06-S04 and E33-S01 only. Reviewed `23bb14e` plus the round-count metadata commit `bfce0cd`. E06-S06..S13 were not re-reviewed. Both stories' published briefs were read independently of executor evidence. No release was installed, tagged, pushed, or published.

| Story | Verdict | Brief-Checksum |
| --- | --- | --- |
| E06-S04 | **FAIL** | `76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642` |
| E33-S01 | **FAIL** | `bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9` |

## Round 6 repair audit and round 7 findings

| ID | Story | Independent reproduction and disposition | Exact required repair |
| --- | --- | --- | --- |
| R7-01 (round 6 F-01) | E06-S04 | **Incomplete.** `formula_engine_problems` now rejects wrong URL versions, missing/duplicate/malformed engine blocks, but accepts three all-zero 64-digit digests. With a temporary formula and mocked v2.0.0 source state, `release.check()` returned `[]`. Exchanging the arm64 and Intel archive URLs between their platform blocks also returned `[]`: the checker counts a target set without binding each target to its platform block. `engine_sha256s` reads sidecars without verifying archive bytes or the release manifest. | Parse the formula's actual resource/platform structure; require the expected target inside each platform block. Compare each URL and SHA-256 to the release manifest and independently verified archive bytes (or a trusted equivalent), including the source archive. Refuse absence of engine resources once the cutover version is selected. Test wrong but well-formed digest, swapped platform resources, duplicate, missing, malformed and wrong-version resources. |
| R7-02 (round 6 F-02) | E06-S04 | **Incomplete.** A v1.21.0 retry no longer adopts Rust without `--adopt-cutover`, and a simulated failed post-write check restores bytes. But in an isolated temporary formula, `finalize('2.0.0', adopt_cutover=False)` succeeded with zero engine resources and `check()==[]`. With `adopt_cutover=True`, a formula containing an existing malformed `resource "engine"` was silently replaced by the template. The implementation checks neither an owner-accepted migration verdict nor a complete pre-adoption formula shape; it writes before its final `check()` and then restores, leaving a crash/race window. `docs/RELEASING.md` still says `finalize` adopts automatically, so its documented command takes the failing no-switch path. | Make 2.0.0 require an explicit, owner-accepted cutover state and the Rust template; reject partly edited/ambiguous live formulas before adoption. Validate the complete candidate, including artifact identity, before one atomic replace. Reject a 2.0.0 finalization that leaves the Python-only formula. Update the runbook command and prove retry idempotence and failure/crash preservation of original bytes. |
| R7-03 (round 6 F-03) | E06-S04 | **Incomplete.** CI now invokes `verify-installed`, but explicitly accepts the v1.21.0 engine's `0.1.0` version and skips `brew test` for that only published artifact. The job is still named a cutover install test. Release artifact packaging runs `cancellai-cli version` without asserting output. `verify-installed('2.0.0')` accepted a synthetic legacy executable printing `cancellai 12.0.0` because it uses substring matching. | Assert exact versions for both installed commands; test the current built engine's version contract and run `brew test` on a 2.0.0 candidate or make the pre-cutover smoke explicitly limited to install/launch. Do not count v1.21.0 with a version exception as proof of the 2.0.0 formula test. |
| R7-04 (round 6 F-04) | E06-S04 | **Incomplete.** Unreleased now states Windows `configure` refusal and the `cancellai-cli` help/version name. Direct comparison of all four command help surfaces found Rust-only `status --allow-running` and `version --source`; neither is in `project/cli_inventory.json` or Unreleased. `test_cli_inventory.py` only checks Python flags and treats a shared flag as semantically the same if its spelling appears in a committed Rust help golden. `docs/CLI_RUST.md` also says no package manager distributes this engine, contradicting its own canonical-from-2.0.0 introduction. | Complete the inventory in both directions for the four shared commands, including Rust-only flags and behavior, and disclose every intentional contract change in 2.0.0 notes. Make its check compare actual command semantics or specific behavioral probes rather than flag presence alone. Correct the canonical CLI document's package-channel statement. |
| R7-05 (round 6 F-05) | E33-S01 | **Incomplete.** A malformed stored notice now refuses after replay. An independent temporary integration test installed valid signed sequences 1 and 2 from one trusted publisher, then served the *exact* stored sequence 1 through test curl. Refresh returned exit 0, `already current`, with unchanged ledger bytes. Raw `events.contains(Install(text))` still treats any historically installed notice as current, including a rolled-back feed. The temporary test failed as expected and was removed; the tracked test file is unchanged. | Determine current verified publisher sequence from the replayed ledger. Return `already current` only for its exact current notice. Refuse any older sequence even when identical bytes occur in history, preserving byte-identical ledger state. Add a regression for exact old text, plus corrupt/revoked trust and local lift cases as appropriate. |
| R7-06 (round 6 F-06) | E33-S01 | **Confirmed repaired within tested scope.** Production selects fixed `/usr/bin/curl` or `/bin/curl` (Windows system path), retains both HTTPS protocol switches, and the PATH-hijack regression passed. The fake transport is reachable only under `test-curl`, absent from the release build's feature set. | None for this finding. Keep the production feature set and executable selection checked. |

## Eleven falsification axes

| Axis | E06-S04 | E33-S01 |
| --- | --- | --- |
| Path/identity changes | Swapping platform resource URLs passes the checker (R7-01). Live formula remains Python until an explicit release edit. | Fixed curl path prevents the reproduced PATH hijack (R7-06); test-only override is feature-gated. |
| Partial reads/permissions | `finalize` accepts and replaces a partially edited live formula (R7-02); source/sidecar download errors refuse. | Corrupt history is now replay-refused for a 200 response. Unreadable override/trust paths are refused by source path and integration coverage. |
| Links/mounts/reparse | No new filesystem escape reproduced in the packaging change; no host installation was attempted. | State-root controls are inherited; no new link/mount counterexample reproduced. |
| Provider version/layout drift | E06-S06..S13 are `done`; CR4 verdicts and the E06-S08 ceiling record were present. They were not re-reviewed. | Notice still enters the kernel's existing signed install path; no provider-layout authority widening reproduced. |
| Concurrency | Post-write `check` plus rollback is not one atomic validated transition; another reader may see the candidate before its check (R7-02). | Two refreshes are not serialized; the exact-old-notice case already demonstrates a stale decision path (R7-05). |
| Crash/failure/retry | v1.21.0 retry no longer switches; v2.0.0 without switch succeeds and partial formula adoption succeeds (R7-02). | Invalid feed paths preserve history in the passing suite; old exact text reports false success (R7-05). |
| Boundary values | All-zero 64-digit SHA values and wrong architecture association pass release check (R7-01). | The 256 KiB plus 10 byte case refuses; the bounded read is in the production path. |
| Policy/trust conflicts | No machine gate enforces owner verdict before cutover adoption (R7-02); tag/source/engine version text checks exist. | Replay is verified before `already current`, but the code then searches historical raw events rather than current sequence (R7-05). |
| Platform differences | macOS rendered formula syntax passed; Windows target Clippy passed. Native Linux/Windows cutover installation was not run here. | macOS integration ran; Linux/Windows native refresh was not run here. |
| Malformed/untrusted input | Malformed resource blocks are counted, but well-formed wrong digests and platform swaps are accepted (R7-01). | HTTP override, malformed/untrusted notice, failed curl and corrupt ledger tests refuse; older exact signed text falsely succeeds (R7-05). |
| Performance/large data | No new packaged 2.0.0 artifact or benchmark exists; accepted E06-S08 evidence remains a prerequisite claim. | Body read stops at 256 KiB plus one. No over-cap acceptance found. |

## Compatibility and rollback

The committed live `Formula/cancellai.rb` still installs Python as `cancellai`; the Rust template remains outside `Formula/`. Rendering v1.21.0 produced three engine resources and `cancellai-legacy`, with a Python dependency, and passed `ruby -c`. This is a mechanical rollback path in the proposed formula, while the tagged Python archive remains available. It is not proof that the 2.0.0 adoption or install gate is safe. The disclosed Windows `configure` refusal and help/version name are now present, but the Rust-only flags above are absent from the claimed complete inventory. No native 2.0.0 candidate exists for this pass. The owner's migration Safety Verdict acceptance remains separate and **PENDING**.

## Gates

| Gate | Result |
| --- | --- |
| `cargo fmt --check` (rust/) | PASS |
| Workspace Clippy `--all-targets --all-features -- -D warnings` | PASS |
| Windows-target workspace Clippy, `x86_64-pc-windows-gnu` | PASS; target installed |
| `cargo test --workspace` | PASS, including doctests |
| Stable-channel CLI suite, `--features kill-points,test-curl` | PASS; the separate temporary old-sequence counterexample failed as expected |
| `python3 -m pytest tests` | PASS; 714 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check` | PASS at baseline; release check has the demonstrated false negatives |
| Post-evidence `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check`, `process_metrics.py check`, `check_evidence.py check` | PASS after both statuses became `in_progress` and both generators ran; process/evidence reported only recorded baseline warnings |
| Render v1.21.0 cutover formula and `ruby -c` | PASS; no Homebrew installation |
| Homebrew style/audit on a throwaway tap | NOT RUN in round 7; round 6 passed this on a byte-identical render, and the round 7 render's syntax passed |
| Native Linux/Windows cutover install and feed refresh | NOT RUN; this verifier has only a macOS host |
| Latest main CI at time of review | Governance, tests, CodeQL and Rust all PASS for `bfce0cd` and `23bb14e`. CI does not exercise the reproduced counterexamples. |

### Owner decision

PENDING. Both stories return to `in_progress`. This is the second and last pass under the owner's stated limit; further handling requires an owner decision, not an implied third verifier round.
