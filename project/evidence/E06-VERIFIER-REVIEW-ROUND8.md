Review-Scope: epic
Round: 8
Verifier: Codex

# Independent verifier review — 2026-09-24

Owner-authorized extra pass over exactly E06-S04 and E33-S01 at `351295d`. E06-S06..S13 were not re-reviewed. Both verdicts are **FAIL**. No release was installed on this host, tagged, pushed, or published. Temporary adversarial test code was restored after execution.

| Story | Verdict | Brief-Checksum |
| --- | --- | --- |
| E06-S04 | FAIL | `76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642` |
| E33-S01 | FAIL | `bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9` |

## Round 7 repairs and round 8 findings

| ID | Story | Independent result | Exact required repair |
| --- | --- | --- | --- |
| R8-01 (R7-01) | E06-S04 | **Incomplete.** Moving the arm resource outside `on_arm` but after its closing `end` left `formula_engine_problems(..., "2.0.0") == []`; `enclosing_platform` remembers opening text, not Ruby block nesting. A synthetic 2.0.0 Python-only formula also made `check() == []`. Replacing the formula's source SHA with another 64-digit value passed `check()`. Engine digests are compared to published sidecars, never to downloaded archives or the release manifest. | Parse the actual platform/resource nesting; require exactly one correct resource in each live block and a Rust engine at/after 2.0.0. Compare the source SHA to a freshly downloaded tagged source archive; compare each engine SHA to independently downloaded archive bytes and the release manifest, with tag and target identity checked. Test outside-block, absent-engine, wrong source digest, altered sidecar/archive, malformed, duplicate and swapped resources. |
| R8-02 (R7-02) | E06-S04 | **Incomplete.** The new status guard refuses adoption while E06-S04 is not `done`, v1.21.0 cannot adopt, a partially converted formula is refused by its marker check, and a normal same-version retry was byte-identical in an isolated run. But a temporary template with its `resource("engine").stage { bin.install "cancellai-cli" => "cancellai" }` line removed was adopted by `finalize("2.0.0", adopt_cutover=True)` and passed its post-write `check()`. Validation is still after the atomic replace, with a crash/reader window before rollback. | Validate the complete candidate formula's install mapping, legacy rollback mapping, source and resource identities and digests before writing. Reject malformed or altered pre-cutover shapes, not just marker strings. Perform one atomic replacement after all checks; make a failed check leave original bytes without a post-write rollback window. |
| R8-03 (R7-03) | E06-S04 | **Repaired within tested scope.** A fake `cancellai-legacy` reporting `cancellai 12.0.0` was refused for 2.0.0. `tests.yml` now builds the current stable-channel engine, renders a local throwaway formula, asserts exact installed versions and runs `brew test`. Its latest main workflow passed. No published 2.0.0 archive exists to test. | None for the prior version assertion; retain a tag-artifact check when 2.0.0 is cut. |
| R8-04 (R7-04) | E06-S04 | **Incomplete.** Both Rust-only flags are now in the inventory and Unreleased notes, and `CLI_RUST.md` names the Homebrew channel. But the inventory still calls `status/clean --days` the same. With synthetic roots, Python `status --days 0` and `clean --days 0 --dry-run` refused with exit 2; the stable-channel Rust `status --days 0` exited 0, and `plan --days 0 --keep-latest 0` proposed one `Delete` action for an old synthetic session. This contract change is absent from Unreleased, and the inventory test checks flag names/help text rather than values and behavior. | Either reject zero days in Rust for all shared commands or mark and disclose the changed accepted range and its retention consequence. Add behavioral probes for every shared flag labeled `same`, including boundary values, to the checked inventory. |
| R8-05 (R7-05) | E33-S01 | **Repaired within tested scope.** A separate temporary integration test installed valid signed sequences 11 and 12, served the exact older 11, and saw exit 4 with byte-identical history. Serving 12 returned `already current`; corrupting trust then made refresh refuse without changing history. | None for the prior rollback shortcut. |
| R8-06 (new) | E33-S01 | **FAIL.** Sixteen concurrent `containment refresh` processes fetched the same valid signed sequence 1 from a synthetic fake curl. Two install lines landed. `containment list` then exited 4 because event 2 replayed as a duplicate; later refreshes refused an unusable ledger. The temporary test failed as intended and was removed. The existing append-only store documents unsynchronized decisions, but a simultaneous feed refresh turns a valid notice into persistent unknown state. | Serialize the replay/current-sequence decision and append across processes with a crash-safe mechanism compatible with the ledger and safety boundary; recheck under that serialization, then append at most once. Add a multi-process same-notice regression and prove the resulting log replays, one process reports installation, the rest report current, and a crash leaves a usable or explicitly recoverable ledger. |
| R7-06 | E33-S01 | **Still repaired within tested scope.** Production chooses fixed system curl paths; the test-only override is feature-gated. The stable suite's PATH-hijack regression passed and fetch retains both HTTPS-only protocol switches. | None. |

## Eleven falsification axes

| Axis | E06-S04 | E33-S01 |
| --- | --- | --- |
| Path/identity changes | Resource placed after `on_arm`'s closing `end` is misidentified as inside it (R8-01); live formula still installs Python before cutover. | Fixed curl path prevents PATH selection; test override only under `test-curl`. |
| Partial reads/permissions | Wrong source SHA and template with no engine install pass checks (R8-01/02). Archive download errors fail; no published 2.0.0 candidate exists. | Corrupt trust and corrupt history refuse; a missing system curl refuses. |
| Links/mounts/reparse | No new link or mount escape reproduced in packaging; no host installation. | Existing local-state-root handling was inspected, not re-reviewed. No new link escape reproduced. |
| Provider version/layout drift | Accepted E06-S06..S13 gate records are present; these stories were not re-reviewed. | Notice payload still enters the kernel's signed install/replay path. |
| Concurrency | Post-write `check()` exposes an unvalidated formula before possible rollback (R8-02). | Concurrent refresh of identical valid notice wrote a duplicate and made ledger replay fail (R8-06). |
| Crash/failure/retry | Normal retry idempotent; bad template adoption succeeds, and a crash between replace and final check can leave it live (R8-02). | Duplicate refresh race persists as unknown state; network/parse refusals leave bytes unchanged. |
| Boundary values | Zero days is accepted by Rust and refused by Python, with a Rust Delete proposal (R8-04). | Over-cap `MAX_NOTICE_BYTES + 1` read refused in stable suite; exact old sequence now refused. |
| Policy/trust conflicts | `done` story status gates adoption, but formula/install contract is not verified (R8-02). | Corrupt trust refuses current response; signed duplicate race cannot replay and fails closed. |
| Platform differences | Windows-target Clippy passed; macOS render has valid Ruby syntax. Native 2.0.0 Homebrew install and native Linux/Windows installation were not run here. | macOS integration passed; native Linux/Windows refresh not run locally. |
| Malformed/untrusted input | Wrong nesting, absent engine, altered source digest and missing install command evade release checks (R8-01/02). | Malformed, untrusted, HTTP override, and old sequence refuse; concurrent valid input corrupts history (R8-06). |
| Performance/large data | Accepted E06-S08 budget evidence exists; no 2.0.0 packaged benchmark is available. | Bounded body read is present; sixteen short concurrent requests suffice for R8-06, without large input. |

## Compatibility, rollback and release evidence

The live `Formula/cancellai.rb` still installs Python as `cancellai`; the cutover template is outside `Formula/`, so this commit does not itself switch the public tap. The rendered v1.21.0 candidate installs the tagged Python script as `cancellai-legacy`, and Ruby syntax passed. That is a mechanical rollback command through 2.1.0, contingent on a correctly adopted template; R8-02 shows adoption does not validate the mapping. The Python source remains tag-archiveable. The release notes inventory is incomplete at `--days 0` (R8-04). The owner migration Safety Verdict acceptance is separate and remains pending.

## Gates

| Gate | Result |
| --- | --- |
| `cargo fmt --check` (rust/) | PASS |
| Workspace Clippy `--all-targets --all-features -- -D warnings` | PASS |
| Windows-target workspace Clippy `x86_64-pc-windows-gnu` | PASS; target installed |
| `cargo test --workspace` | PASS, including doctests |
| Stable-channel CLI `--features kill-points,test-curl` | PASS; separate old-sequence/trust test PASS, separate concurrency test FAILED as expected |
| `python3 -m pytest tests` | PASS; 719 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check` | PASS at baseline; release check has reproduced false negatives |
| v1.21.0 render and `ruby -c` | PASS; no installation |
| Homebrew style/audit on throwaway tap | NOT RUN; no tap was registered on this host in this round |
| Native Linux/Windows cutover installation and feed refresh | NOT RUN; macOS verifier host only |
| Latest main CI | Governance, tests, Rust and CodeQL PASS for `351295d`; these workflows did not expose R8-01/02/04/06 |

### Post-evidence gates

After both statuses became `in_progress`, `project_os.py generate` and `process_metrics.py generate` completed. `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check`, `process_metrics.py check` and `check_evidence.py check` all **PASS**. `check_process.py` warns that E06 now has nine review records under its recorded exception; that exception's explanatory text stops at round 7 despite this expressly owner-authorized extra pass. The evidence checker reports only recorded pre-convention warnings. Python pytest was rerun after generation and remained green (719 passed, 3 skipped); the Rust source and test file bytes were unchanged after their earlier gates.

### Owner decision

PENDING. Both stories return to `in_progress`. E06-S04 needs release candidate/CLI contract repairs and independent recheck; E33-S01 needs the refresh race repaired and independently verified. The owner retains the migration Safety Verdict acceptance decision.
