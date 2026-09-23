Review-Scope: epic
Round: 6
Verifier: Codex

# Independent verifier review - 2026-09-24

This is the first independent pass over **E06-S04** and **E33-S01** only. E06-S06..S13 were not re-reviewed. The reviewed tree was `2531746` plus the deliberately uncommitted E06-S04 verifier brief. No product code was changed and no release was published.

| Story | Verdict | Brief-Checksum |
| --- | --- | --- |
| E06-S04 | **FAIL** | `76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642` |
| E33-S01 | **FAIL** | `bb614fb4de0d31b814e7497488c4e83eac0c298b389fc899b433a3d92b1854a9` |

## Blocking findings and exact repairs

| ID | Story | Independent counterexample | Required repair |
| --- | --- | --- | --- |
| F-01 | E06-S04 | `release.check()` returned `[]` when a cutover formula's three engine archive URLs were changed to v1.20.0 and their digests corrupted while the source/formula tag stayed v1.21.0. `point_engine_resources` also accepted an extra `resource "engine"` whose URL did not match its regex. The checker does not inspect engine resources at all. | Parse the complete formula structure, require exactly one resource for each supported target and no additional engine resource, and verify each archive tag, filename target, and digest against the release manifest or downloaded archive before reporting consistency. Add negative tests for wrong version, digest, duplicate, missing, and malformed extra resource. |
| F-02 | E06-S04 | In an isolated copy of the live formula, rerunning `finalize('1.21.0')` adopted the Rust template for the already published Python v1.21.0 release. It uses absence of regex-matched resources as the sole cutover condition. A second isolated call with a forced post-write check failure raised `ReleaseError` but left the formula switched. | Gate template adoption on an explicit 2.0.0 cutover transition and owner-accepted migration verdict, refuse ambiguous/partly edited formulas, calculate and validate the entire new formula before writing, and replace it atomically only after validation. Verify a v1.21.0 retry cannot switch engines, successful same-version retries are idempotent, and any failure preserves the original bytes. |
| F-03 | E06-S04 | The published v1.21.0 macOS arm64 archive, independently downloaded and executed, prints `cancellai-cli 0.1.0`. `tests.yml` renders a v1.21.0 cutover formula and invokes `cancellai version` without checking its output; its formula `test do` expects the formula version. This test can therefore pass despite the very version mismatch the cutover is meant to prevent. | Make the throwaway tap job assert the exact engine version equals the archive/tag version and run the formula test. For the pre-cutover v1.21.0 archive, either test only properties it can actually satisfy and separately test the current built engine's version contract, or use a 2.0.0 candidate artifact; do not label the old artifact a successful cutover smoke test. |
| F-04 | E06-S04 | The Python CLI has `configure --claude-retention` on Windows; `docs/CLI_RUST.md` states the Rust `configure` refuses on every non-Unix platform. Unreleased notes claim Windows support but do not disclose this command regression. The Rust help/version output still identifies `cancellai-cli` and calls it a target-engine beta after the formula renames it `cancellai`. | State the Windows `configure` refusal and user-facing command/help/version differences in the 2.0.0 release notes, or implement supported behavior before cutover. Update the canonical Rust CLI documentation and installed help to describe the released command accurately. Compare all four shared commands and flags against the Python CLI in a checked release-note inventory. |
| F-05 | E33-S01 | A synthetic ledger line `{"install":"{\"not\":\"a signed notice\"}"}` and a fake curl returning the same malformed body with 200 made `containment refresh` exit 0, saying `already current`. `containment list` exited 4 because that history could not re-verify. `cmd_refresh` searches raw stored events before replay or signature checking. | Load and re-verify the ledger and trust policy before any already-current return. Return safety refusal for unknown history/trust. Only call a feed already current when the exact notice is the verified current publisher sequence; make stale/rolled-back input a refusal with unchanged history. Add corrupt-history, revoked/unreadable-trust, and older-sequence tests. |
| F-06 | E33-S01 | With `PATH` pointing at a temporary executable named `curl`, refresh executed that program and accepted its 404 output. This defeats the claim that the OS curl and its HTTPS-only switches were used; the substitute can perform arbitrary work or fetch over HTTP before producing output. The existing tests intentionally inject a fake curl through this same mechanism. | Resolve and execute a trusted platform curl path, or validate the selected executable's identity before launching it; do not let an untrusted PATH entry supply the transport. Keep feed tests injectable through a test-only seam and add a PATH-hijack regression. Keep `--proto =https` and `--proto-redir =https` for the actual curl. |

## Eleven falsification axes

| Axis | E06-S04 | E33-S01 |
| --- | --- | --- |
| Path/identity changes | Formula adoption keyed to a regex absence; partial and extra resources evade identity checks (F-01/F-02). | PATH-selected executable changes fetch identity (F-06); state root uses `LocalStateRoot`. |
| Partial reads/permissions | Source archive and sidecar reads error, but a post-write consistency failure leaves the live formula changed (F-02). | Missing curl and unreadable override refuse; unreadable ledger does not enter the raw-event shortcut, while a parseable but unverifiable one does (F-05). |
| Links/mounts/reparse | Formula targets fixed release URLs; no installer link attack was reproduced. Existing gate evidence for filesystem mutation was checked, not re-reviewed. | State-root/path handling was source-inspected; no new link or mount bypass reproduced. |
| Provider version/layout drift | G1-G4 prerequisite stories are `done`; their owner-accepted prior verdicts exist. The cutover does not change provider adapters. | Notice targets still pass through kernel verification and the same `install_text` path; no provider-layout authority widening found. |
| Concurrency | A formula rewrite is not atomic (F-02); a concurrent edit can leave a partially adopted formula. | Refresh uses the existing append-only install path; simultaneous decisions are not serialized and can produce duplicate/stale events, a pre-existing ledger limitation requiring a targeted regression before claiming concurrent refresh safety. |
| Crash/failure/retry | Re-finalizing the old tag switches engines, and a failed final check leaves changed bytes (F-02). | Network/error paths leave the log unchanged in the suite; raw already-current return masks a corrupt prior history (F-05). |
| Boundary values | Three valid resources are required by the rewrite regex, but a fourth malformed resource is accepted (F-01). | `take(MAX_NOTICE_BYTES + 1)` refuses oversized stdout; 256 KiB plus 10 bytes was exercised in the stable suite. |
| Policy/trust conflicts | The cutover template installs a stable-channel binary; latest published v1.21.0 archive still reports 0.1.0 (F-03). | Signed install path enforces trust, but fast-path bypasses replay/trust checks before reporting success (F-05). |
| Platform differences | Rendered macOS arm64 archive ran locally; Linux and Windows native cutover installs were **NOT RUN** here. Windows-target Clippy passed. Windows `configure` gap is undisclosed in Unreleased (F-04). | macOS fake-feed integration ran; Linux/Windows native refresh tests were **NOT RUN** here. |
| Malformed/untrusted input | Wrong-version/wrong-digest and extra-resource formulas passed checks (F-01). | Malformed ledger plus matching malformed feed returned success; fake PATH curl executed (F-05/F-06). HTTP override was refused; actual curl arguments restrict initial and redirect protocols to HTTPS. |
| Performance/large data | No new packaged 2.0.0 benchmark was available; prior accepted E06-S08 budget evidence remains the gate claim. | Body cap is enforced during read. No unbounded-body acceptance found; stderr/process behavior under a malicious executable remains tied to F-06. |

## Compatibility, rollback, and release evidence

The live `Formula/cancellai.rb` still installs Python as `cancellai`; the template is outside `Formula/`. The template installs the tagged Python script as `cancellai-legacy`, with a Python dependency, so the rollback command is mechanically present through the proposed 2.1.0 window. The archived Python source remains available by tag. No 2.0.0 artifact or owner-accepted migration verdict exists yet. The Unreleased notes cover the removed flags, JSON shape, history behavior, containment, release channel, and Windows deletion, but omit the Windows configure refusal and canonical help/version presentation (F-04).

The rendered v1.21.0 template passed Ruby syntax, `brew style`, and `brew audit --strict` using an existing throwaway tap whose formula bytes matched the render. **No brew install was run on this host.** The released arm64 archive's SHA-256 matched its published sidecar (`d17a7cdef4c2311a930c05e001494b091222673ffb66ff064932fd95560f49cf`), yet the executable reported 0.1.0 (F-03). This is an artifact/version counterexample, not a signature-forgery claim.

## Gates

| Gate | Result |
| --- | --- |
| `cargo fmt --check` (rust/) | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Windows-target workspace Clippy, `x86_64-pc-windows-gnu` | PASS; target installed |
| `cargo test --workspace` | PASS, including doctests |
| `CANCELLAI_CHANNEL=stable CARGO_TARGET_DIR=target/stable-channel cargo test -p cancellai-cli --features kill-points` | PASS |
| `python3 -m pytest tests` | PASS; 705 passed, 3 skipped |
| `python3 scripts/release.py check` | PASS at baseline; F-01 shows a false negative |
| `python3 scripts/project_os.py check` | PASS at baseline |
| `python3 scripts/verifier_handoff.py check` | FAIL at baseline: the older E06-S04 verdict predates the newly rendered brief and lacks its checksum; this appended verdict supplies the checksum. Post-write result recorded below. |
| `python3 scripts/check_process.py check` | PASS at baseline with recorded prior-round exceptions |
| Render v1.21.0 formula, Ruby syntax, named-tap `brew style` / `brew audit --strict` | PASS; no installation |
| Latest `main` CI at final review | PASS: governance, tests, rust, and CodeQL all completed successfully for `2531746`; this does not exercise the rejected counterexamples. |
| Native Linux/Windows cutover install and refresh reproduction | **NOT RUN**: no native hosts in this verifier workspace; target Clippy and prior CI evidence are distinct. |

### Post-evidence gates

After setting both rejected stories to `in_progress`, running `project_os.py generate` and `process_metrics.py generate`: `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check`, `process_metrics.py check`, and `check_evidence.py check` all **PASS**. The process check reports its previously recorded E06 round-count exception; the evidence check reports only documented pre-convention warnings. The newly appended E06-S04 checksum resolves the baseline handoff failure.

Method defect (proposed): these are both stories' first independent passes, but `process_metrics.py` counts first-pass rejection only from files numbered `Round: 1`; its generated round-6 yield correctly shows two of two failures while its first-pass denominator omits both. A per-story first-pass metric would prevent this measurement gap. This observation does not change either product verdict.

### Owner decision

PENDING. Both stories return to `in_progress` for executor repair and another independent pass. The owner retains the migration Safety Verdict acceptance decision.
