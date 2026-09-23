Review-Scope: epic
Round: 3
Verifier: Codex

# Independent Verifier Review — E06, round 3

- Filed by the executor as round 3: E06's rounds 1 and 2 (2026-09-01/02) reviewed E06-S01..S03. Codex wrote this record over `E06-VERIFIER-REVIEW.md`; that historical record was restored unchanged and this one filed separately. For the cutover stories E06-S07..S13 this is their first independent pass; for E06-S06 its second.

- Reviewed commit: `85fed2f7c66baf9da3ed79f1bb870f4277eec434` on `review/e06-round1`, 2026-09-23.
- Scope: E06-S06 (its second story pass), E06-S07 through E06-S13. E06-S04 remains blocked and was not judged.
- Method: started from the committed briefs, contracts, invariants, ADR-0039, final code and current main CI. Reproductions below use synthetic temporary roots; no real provider data was touched. Executor claims were not used as proof.

## Briefs answered

| Story | Brief-Checksum |
| --- | --- |
| E06-S06 | `817de404f11c12559e5d15fc1f1c11e17aa4731db38847bbb757505ef9c96b65` |
| E06-S07 | `109e9df668259845181d2c5e79b6ffad84a4575702665fd3e292b266f5905350` |
| E06-S08 | `cf517c1d13964dbcdef87178e6188718da266910088bb8de8a965bf193971b53` |
| E06-S09 | `23b452f21a1d947034bbecf63ba33556975541f80ee529e1f9dd0a033619ecf3` |
| E06-S10 | `d8e38c5fdaf9a6cf5fec0ab16438c50fbe24eecea7add2d53c03a4cac92afa8b` |
| E06-S11 | `bde99837a9229b3695eddc8640ca40dd99cf399dcd69e0bee0be60a791309271` |
| E06-S12 | `74c57318540c3cddc9de9c795d078cdc82f0d051c2935ab0169c875153ce2006` |
| E06-S13 | `867c06801cf21e247190f5d88a7dbc7ea2ee9d08e699dcc6f61881702d181121` |

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S06 | PASS_WITH_RESIDUALS | Independent mode-000 Codex lineage and 70-error Claude companion reproductions in default and custom root origins: incomplete scopes, zero deletes, Python exit 4, artifact retained. The 14 normative fixtures match in both origins. E21-S07's held-parent unlink prevents intermediate-component redirection. The leaf-name race is the owner-accepted residual in the append-only prior verdict. |
| E06-S07 | PASS_WITH_RESIDUALS | `plan_actions` and `clean` call `apply_authority`; every actual delete also calls `containment::recheck` before `delete_one`. Install reads at most 256 KiB before parsing; replay verifies raw signed installs and sequence, ignores expiry only; corrupt history/trust caps at Recommend. Stable-channel containment CLI tests pass. The same-user edit/rollback/lift limit is explicit in SI-029, ADR-0039, `INCIDENT_RESPONSE.md` and the store module; it is no broader than that accepted owner-state limit. |
| E06-S08 | FAIL | The real stable release binary passed the 2,000-session check (0.296–0.322 s, 9.6–28.0 MiB versus reference 0.795–0.890 s). A wrapper that ran the real binary only for `plan` and exited 7 on `status`, `inspect`, and `clean --dry-run` still made `cutover_benchmark.py check --sessions 100 --runs 1` exit 0 and print “budget OK” (F-01). A 0.5 s delay wrapper correctly failed, so the gate detects slowness but not failed measured work. |
| E06-S09 | PASS | The focused stable-channel kill harness reached every enumerated marker, killed the child, checked artifacts and reran successfully; its planted half-written state test passed. The full stable CLI suite passed on rerun. Exact-main CI has successful kill-harness jobs on macOS, Linux and Windows. |
| E06-S10 | FAIL | Stable CLI tests confirm flag acceptance, history byte identity and verbose action parity; unsupported flags remain usage errors. A successful `clean --yes --json` deleting a stale Claude session without `--keep-claude-history` left history byte-identical but emitted no “left unchanged” note on either stream (F-02). The contract's output disclosure is missing on JSON runs. |
| E06-S11 | PASS | The shared early JSON branch emits a plan for dry-run and a result with `SAFETY_WITHHELD` or `NOT_ELIGIBLE` for no deletion. Stable CLI tests parse the empty, incomplete and dry-run cases; human no-op sentences remain. |
| E06-S12 | PASS | `read_codex_parent_session_id` now returns `io::Result`; open/read errors are recorded in the Codex `ReasonLog`, while successful no-parent parsing remains `Ok(None)`. Claude walker writes each error directly to a bounded `ReasonLog`. Independent 70-error and unreadable-rollout reproductions in both origins had exact error counts and zero delete actions; the reference exited 4. The owner disposition of the Unix leaf race is recorded in E06-S06's prior Safety Verdict. |
| E06-S13 | FAIL | Windows main CI `quality`, `kill-harness` and `cli-stable-channel` succeeded at the reviewed SHA; local Windows-target Clippy passed. Handle-bound listing and deletion code was read line by line. But `WindowsFileFacts` drops `nNumberOfLinks`, and `unlink_child_matching_windows_identity` compares only volume/file index before deletion. A file with another hard link is therefore admitted and deleted, contrary to the explicit hard-link refusal criterion (F-03). Native Windows execution of this new counterexample was unavailable locally. |

## Findings and required repairs

### F-01 — E06-S08: failed commands can pass the performance gate

`scripts/cutover_benchmark.py::run_once` returns elapsed time, RSS and stdout but discards the child exit status. `measure` discards every timed stdout too. `prove_corpus_is_live` validates one earlier `plan`, not each measured invocation. A binary that does no successful work can therefore look faster and pass.

**Reproduction:** Build the stable release binary, then create an executable wrapper whose `plan` arm `exec`s that binary and whose other arms exit 7. Run `E06_REAL_BIN=<absolute stable binary> python3 scripts/cutover_benchmark.py check --rust-bin <wrapper> --sessions 100 --runs 1`. The verifier's run exited 0 and printed “cutover performance budget OK” despite the three measured commands failing. A wrapper sleeping 0.5 seconds before executing the real binary exited 1 with four BUDGET failures, proving the timing branch was active.

**Required repair:** Make every measured run require a successful exit and a valid command-appropriate result on the generated corpus; fail closed when status/inspect/plan/dry-clean output is absent or reports no resolved artifacts/work. Pin the failing-exit and empty-success wrapper cases as gate regressions. Update the story's verification contract if necessary so successful measured work is explicit.

### F-02 — E06-S10: JSON clean does not disclose the history divergence

`rust/crates/cancellai-cli/src/main.rs` prints the history note only in the non-JSON reporting branch (lines 922–943). The contract does not exempt JSON. A script accepting `--keep-claude-history` can therefore get a successful JSON clean with no indication that the no-flag run retained deleted sessions in `history.jsonl`.

**Reproduction:** In a canonicalized synthetic `$HOME`, create `.claude/projects/p/<UUID>.jsonl` with an old mtime and `.claude/history.jsonl` with known bytes. Use the stable release binary: `clean --yes --json --allow-running --keep-latest 0`, with no keep-history flag. This reviewer observed exit 0, session removed, history bytes unchanged, and no “left unchanged” note on stdout or stderr.

**Required repair:** Report the unchanged-history divergence for successful JSON runs without breaking the result document on stdout (for example a clear stderr note, or a versioned structured result field); add a CLI regression asserting the disclosure and byte identity with and without the flag. Preserve the existing non-JSON note and action set.

### F-03 — E06-S13: Windows hard links pass the plain-file delete path

`rust/crates/cancellai-sealedfs/src/windows_identity.rs::observe_identity_of_handle` reads `BY_HANDLE_FILE_INFORMATION` but does not retain `nNumberOfLinks`. `cancellai-safety::mutation_executor::delete_operation_for` admits every Windows `FileKind::File`. `windows_sealed.rs::unlink_child_matching_windows_identity` checks reparse status and `(volume_serial_number, file_index)` only, then calls `SetFileInformationByHandle(FileDispositionInfo)`. Two names hard-linked to one object have the same compared identity; the repository's own Windows identity test establishes this fact. No link-count check exists at plan time or immediately before deletion.

**Reproduction on Windows:** Under a synthetic provider root create a stale eligible session file, then `std::fs::hard_link` it to another path. Run stable-channel `clean --yes --keep-latest 0 --allow-running` for that provider. The ordinary file classification admits the session and the handle delete removes its name; the AC requires refusal because the identity cannot distinguish the two hard-link names. A sharper race fixture replaces the planned leaf with a second hard-link name to that same file before deletion; volume/index and mtime still match. Native execution of this fixture is NOT RUN on this macOS host; the source path and existing real-Windows hard-link identity test establish the missing guard.

**Required repair:** Carry Windows link count from `BY_HANDLE_FILE_INFORMATION`; refuse a file with more than one link at planning and on the handle used for final deletion, including when a second link appears after planning. Add real-Windows CLI and mutation regressions for an initially hard-linked file and a hard-link swap between plan and deletion. Keep directories, reparse points and unknown kinds refused.

## Eleven-axis adversarial pass

Cells identify what was checked; F-01/F-02/F-03 are the findings above. “No new bypass” means source trace plus applicable tests, not native execution on another OS.

| Falsification axis | S06 | S07 | S08 | S09 | S10 | S11 | S12 | S13 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Path/identity changes | Held-parent unlink; leaf residual | Per-delete ledger reload; root binding | Temp-only corpus | Snapshot planned paths | History untouched | Same plan actions | Read after entry metadata | Handle open/bind; F-03 |
| Partial reads/permissions | Mode-000 rollout, 70 denied dirs pass | Bad log/trust caps | F-01 ignores failed commands | Marker failure fails harness | No action change | Incomplete JSON exit 4 | Errors counted and withheld | Listing errors propagate |
| Links/mounts/reparse | No-follow parent; leaf residual | State symlink refusal; owner-state limit | Synthetic local tree | No outside-plan change | History not rewritten | Root gate retained | Symlink walker traced | Reparse refused; hard link F-03 |
| Provider version/layout drift | Partial fixtures both origins | Unknown version matches containment; layout recheck | Fixed synthetic layout | Synthetic two-provider tree | Unsupported flags refused | No-op JSON works | Unknown lineage remains distinct | Bound marker listing checked |
| Concurrency | Intermediate swap contained | Append/replay and recheck; narrow post-recheck window | F-01 failed child pass | Actual kill at marker | Verbose no action delta | Shared plan path | Metadata-then-read error pass | Same-object hard-link swap F-03 |
| Crash/failure/retry | Failure withholds | Torn log unknown; expired install remains | Failed measured work F-01 | Kill/rerun and plant pass | History remains byte-identical | No-op result parses | Read failure records | Pending-delete corroboration traced |
| Boundary values | 70 > 64 reasons | 256 KiB notice; 16 MiB log | 2,000 sessions, 64 MiB cap; F-01 | All 10 markers | Flag combinations | Empty/no-delete paths | 70 reasons exact | 1,500-name listing; F-03 |
| Policy/trust conflicts | Partial scope loses delete | Forgery/replay/unknown trust source inspected | Preflight needs real plan | Stable build required | Divergence disclosed; F-02 | Safety code preserved | Partial scope withholds | Token kind check; F-03 |
| Platform differences | macOS local, parity both origins | CI tri-platform | Linux CI gates, macOS local report | CI tri-platform | CI tri-platform; JSON F-02 | CI tri-platform | macOS local, CI tri-platform | Windows CI success; native new fixture NOT RUN |
| Malformed/untrusted input | Unknown lineage no parent; I/O distinct | Signed bundle schema, publisher, sequence reverified | Invalid/failed output F-01 | Unreached point fails | Unsupported flags exit 2 | Valid JSON on no-op | Error paths retained | Names validated; F-03 |
| Performance/large data | Bounded lineage and reasons | Bounded notice/log/ledger | Real budget passes, false pass F-01 | Small synthetic crash matrix | Verbose observational | JSON document size follows actions | 70 errors retained at 64 | Listing spans buffers; link count absent |

## Gates and provenance

| Gate | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| Windows-target Clippy, `x86_64-pc-windows-gnu` | PASS; target installed |
| `cargo test --workspace` | PASS, including doctests |
| `CANCELLAI_CHANNEL=stable CARGO_TARGET_DIR=target/stable-channel cargo test -p cancellai-cli --features kill-points` | PASS on full rerun. Initial concurrent run failed two kill tests because the child did not pause; the focused harness and full rerun passed. CI focused kill harness passed on three OSes. |
| `cargo deny check` | PASS using an offline, writable copy of the existing advisory DB. Direct run was blocked by the sandbox's read-only `~/.cargo/advisory-dbs/db.lock`; warnings only for existing duplicate dependencies and unmatched allowances. |
| `python3 -m pytest tests` | PASS: 698 tests and 665 subtests |
| `python3 scripts/rust_python_parity.py check` | PASS: 14 NORMATIVE fixtures, both origins, including partial-scan fixtures |
| `python3 scripts/cutover_benchmark.py check` after stable release build | PASS: 2,000 sessions per provider, three runs; figures above |
| `python3 scripts/project_os.py check` (before changes) | PASS |
| `python3 scripts/verifier_handoff.py check` (before changes) | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/check_docs.py check` | PASS |
| Latest-main `rust.yml`, SHA `85fed2f7…` | PASS: Windows `quality`, `kill-harness`, `cli-stable-channel`; corresponding Linux/macOS jobs passed. This proves committed tests ran, not F-03's missing hard-link test. |
| Native Windows reproduction of F-03 | NOT RUN: no Windows host in this working session; reviewed unsafe and exact-main CI instead. |

## Residual risks and review disposition

- E06-S06's Unix `fstatat`/`unlinkat` leaf-name race remains owner-accepted in its prior Safety Verdict. The held parent prevents intermediate path redirection; it cannot make the leaf comparison and unlink atomic.
- E06-S07's history is private owner state, not a tamper-resistant authority source against a same-user actor. The accepted limit includes delete, valid-prefix truncate and appended lift. The recheck immediately precedes each delete but does not serialize a concurrent install with the final syscall; no claim of atomic instant containment is made here.
- E06-S09's first local full-suite attempt did not reach kill markers; the repeat, focused run and three-platform exact-main CI passed. Preserve this transient in the evidence instead of treating either one run as dispositive.
- E06-S13's native Windows CI is green, but no current Windows test asserts the hard-link refusal. F-03 remains open pending native regression and repair.
- F-01, F-02 and F-03 require executor repair in their story contracts before a further epic review. Three of eight judged stories failed (37.5% rejection yield), so ADR-0025 requires another round. E06-S04 remains blocked.
- `PROCESS_METRICS.md` mechanically pairs this newly requested `Round: 1` record with the older E06 `ROUND2` file from the 2026-09-01 S01–S03 campaign. Their zero finding overlap reflects different story sets and review campaigns, not independent disagreement about these eight stories. The older round-1 report is preserved verbatim below because this task required reusing its original path.

## Owner decision

`PENDING` for this review's CR4 Safety Verdicts and any future epic closure; the recorded owner-state and Unix leaf-race dispositions are acknowledged above.

## Archived prior E06 review (2026-09-01)

The earlier S01–S04 review occupied this required path. It is preserved verbatim below as a blockquote; the current round header and verdict table above apply only to S06–S13.

> # E06 Independent Verifier Review - Round 1
>
> - Review target: `4024ab8..16a44b0`
> - Verifier: Codex (`/root`), independent verifier
> - Date: 2026-09-01
> - Epic: E06 - Rust CLI Parity and Cutover
>
> All four E06 stories were `ready_for_review` before this review began.  This review used the
> story contracts, architecture/security requirements, final diff, and independently constructed
> counterexamples; executor rationale was not treated as evidence.
>
> ## Per-story verdicts
>
> | Story | Verdict | Concrete evidence |
> | --- | --- | --- |
> | E06-S01 | FAIL | A stale session beneath a root supplied through `CLAUDE_CONFIG_DIR` is reported as `origin=default`, `confidence=default`, then permanently deleted by `clean --yes`.  The Python reference refuses every custom root, including a low-confidence root containing only `projects/`, with exit 4.  `configure` follows its predictable `settings.json.cancellai-tmp` symlink and overwrites a file outside the configured root.  A malformed `settings.json` is silently replaced.  A mixed partial Claude scan plus a clean Codex candidate makes `clean --dry-run` exit 0 while a provider is withheld; the partial-scope inventory emits `knowledge_confidence: observed`, exceeding JSON_CONTRACTS' `LOW/UNKNOWN` ceiling. |
> | E06-S02 | FAIL | The supposedly ADR/RFC-cited allow-list accepts arbitrary text: setting `INTENTIONAL_DIVERGENCES = {"fx": "uncited free text"}` makes `_compare_results("fx", ..., ({"a"}, False), ({"b"}, True))` return `[]`.  The actual comparator observes only delete-UUID sets and one withheld bit.  It cannot detect the normative corpus' root-confidence/origin, protected/unknown coverage, or other classified artifact semantics; `codex-layout-drift`'s normative coverage assertion is one concrete blind spot. |
> | E06-S03 | PASS_WITH_RESIDUALS | `cargo test -p cancellai-cli` independently passes the five side-by-side tests: `version` identifies `cancellai-cli` and its concrete version, read-only commands leave synthetic home unchanged, real clean changes only its selected artifact, and invocation is CWD-independent.  Neither engine has cancellAI-owned persisted state today.  Residual: this is a source-built beta smoke, not a packaged installer/upgrade/uninstall test; E17 owns the release factory.  It is blocked from closure this round because E06-S02 failed. |
> | E06-S04 | FAIL | The submitted gate assessment is correct that cutover is not ready, but that means AC1 (an accepted owner-visible migration Safety Verdict) is not met.  G1--G4 remain explicitly not ready; the S01/S02 failures additionally invalidate the claimed CLI parity and SI-019 posture.  Python remains available in tag `v1.6.0`, and no canonical-engine switch was made.  See `project/evidence/E06-S04/SAFETY_VERDICT.md`. |
>
> ## Required repairs
>
> ### E06-S01
>
> Reproduction, on a new synthetic tree, was:
>
> ```text
> CLAUDE_CONFIG_DIR=<custom-root> cancellai-cli clean --tool claude --keep-latest 0 \
>   --allow-running --yes --json
> ```
>
> where `<custom-root>/projects/project-a/<uuid>.jsonl` was stale and was its only provider
> marker.  The command emitted `succeeded` and removed the file.  The retry also pre-created
> `<claude-root>/settings.json.cancellai-tmp` as a symlink to `<outside-file>`; `configure
> --claude-retention 30` replaced the outside file and left `settings.json` as that symlink.
>
> Required repair:
>
> 1. Determine default-versus-custom roots from their resolved paths, retain the provider
> fingerprint's actual origin/confidence in classification, and independently refuse mutation of
> custom/unverified roots at execution.  Do not hard-code `is_default_root = true` or a verified
> `RootFingerprint` when sealing a plan.  Add low/high/unknown custom-root and absent-home
> regressions.
> 2. Treat `configure` as a safety-relevant provider mutation: reject custom roots, malformed or
> non-object settings, and links; use a unique no-follow temporary file and an atomic durable
> replace that cannot write outside the approved root.  Extend boundary governance or add a
> dedicated configuration-write boundary test so this cannot be omitted from SI-019 checks.
> 3. Propagate partial/unknown scan completeness to every emitted artifact's confidence, and make
> both dry-run and execution return exit 4 whenever requested work is withheld for incomplete
> state.  Strictly reject invalid arguments for every command.
> 4. Revalidate the recorded `process_not_running` precondition immediately before deletion; the
> current sealed plan revalidates only filesystem identity.
>
> This violates E06-S01 AC2/AC3, SI-007, SI-008/SI-009/SI-014, SI-019, C-02, C-03, C-06, and C-07.
> The irreversible production deletion/configuration surface also needs a reviewed CR-level
> assessment rather than relying on its current CR3 label.
>
> ### E06-S02
>
> Required repair: make every allow-list entry validate a real accepted ADR/RFC identifier and
> fail if it does not.  Compare a canonical semantic projection for each NORMATIVE fixture,
> including root authority, scan completeness, protected/unknown coverage, discovered identity
> records, and proposed actions, not only deletion UUIDs and one Boolean.  Add injected-divergence
> tests for each projected field and a real custom-root fixture outside the characterization
> helper's default-root patch.  This violates E06-S02 AC1 and AC2, and M6's requirement that an
> unexplained semantic divergence blocks cutover.
>
> ### E06-S04
>
> Required repair: keep Rust non-canonical; first close E06-S01/S02 and rerun their independent
> verification.  Then supply real G1--G4 evidence, a passing independent CR4 Safety Verdict, and
> owner acceptance before any switch.  The control plane must also resolve the prerequisite cycle:
> the current checklist requires tier-1/install evidence assigned to E07/E17, while E07/E17 depend
> on E06.  This violates E06-S04 AC1 and SI-019's CR4 evidence-gating requirement.
>
> ## Gate status
>
> | Command | Result |
> | --- | --- |
> | `cargo test -p cancellai-cli` | PASS (re-run after retry) |
> | `cargo fmt --check` | PASS |
> | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
> | `cargo check --workspace --all-targets` | PASS |
> | `cargo test --workspace` | PASS |
> | `cargo deny check` | PASS (three existing unmatched-license-allowance warnings; required advisory-cache access granted) |
> | `.venv/bin/python -m pytest tests -v` | PASS: 179 tests, 22 subtests |
> | `.venv/bin/python -m ruff check .` / `ruff format --check .` | PASS |
> | `.venv/bin/python -m mypy` over every required target including `rust_python_parity.py` | PASS |
> | Generated docs, project OS, docs, workflows, fixtures, schemas, characterization, differential harness, Rust workspace, mutation boundary, provider compatibility, process, and release checks | PASS |
> | `python3 scripts/rust_python_parity.py self-test` / `check` | PASS: comparator self-test; 10 current NORMATIVE fixtures |
> | Adversarial custom-root deletion, temporary-symlink configuration escape, malformed settings, partial-scan exit/schema, and uncited allow-list probes | FAIL as described above |
>
> The system Python 3.14 lacks pytest, Ruff, and mypy; the pinned repository `.venv` supplied the
> successful Python-tool gates.  A static mutation-boundary pass is not evidence that the new
> configuration writer observes SI-019: it does not scan `std::fs::write`, `rename`, or
> `create_dir_all`.
>
> ## Overall verdict
>
> **FAIL — round 1 of at most 2.** E06-S01 returns to `in_progress`; E06-S02, E06-S03, and
> E06-S04 are blocked by the failed dependency chain.  The epic remains `in_progress` with one
> review round remaining.  No cutover, release, or owner Safety Verdict acceptance is recommended.
