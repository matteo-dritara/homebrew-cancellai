# Safety Verdict - E06-S06

- Change: Independent adversarial verification of E21-S03 scan completeness and E21-S07 handle-relative unlink
- Risk: CR4
- Reviewed state: current working tree on 2026-09-23; Rust implementation rebuilt by the parity gate
Verifier: Codex
Brief-Checksum: 817de404f11c12559e5d15fc1f1c11e17aa4731db38847bbb757505ef9c96b65

## Verdict

`FAIL`

## Safety surface reviewed

Claude and Codex discovery produce completeness observations used by the Rust planning path. Confirmed deletion revalidates identity and delegates the final unlink to `cancellai-sealedfs`, where a held parent-directory handle prevents a later path-component swap from redirecting the unlink. The directory unreadability repairs reproduce correctly, but an unreadable Codex rollout's lineage read is silently downgraded to “no parent”, allowing a destructive plan from a scope reported complete. Claude's companion walker also accumulates an unbounded intermediate reason vector before the bounded scope log sees it.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-008 | A partial scope cannot produce irreversible actions | The synthetic unreadable Claude project, companion and nested directories, and Codex unreadable nested directories all produced incomplete scopes and no delete actions. Counterexample: a mode-000 Codex rollout remains in the inventory; `plan` emits a delete action because the lineage read error is omitted from completeness. | FAIL |
| SI-009 | Missing/unknown evidence never becomes absence or destructive permission | Python reference marks the unreadable Codex rollout scope withheld and exits 4. Rust `inspect` reports `codex-cli complete=true,error_count=0`; Rust `plan` emits the stale rollout as a delete action. | FAIL |
| SI-010 | Permission/I/O/disappearance failures remain visible | Adapter listing and metadata failures are generally logged. `read_codex_parent_session_id` uses `File::open(path).ok()?` and the bounded read loop turns read errors into `None`, without adding a `CompletenessReason`; Claude companion reasons also bypass the bounded `ReasonLog` while accumulated. | FAIL |
| SI-019 | Every mutation routes through one evidence-gated safety boundary | `python3 scripts/check_mutation_boundary.py check` passes; production confirmed deletion reaches the sealedfs path. The documented `fstatat`/`unlinkat` leaf-name race remains a residual: a replacement between those two syscalls can be unlinked before the held-file link-count check detects the mismatch. | PASS_WITH_RESIDUALS |

## Findings and required repairs

### F-01 - Codex lineage read errors are swallowed and do not make the scope incomplete

`rust/crates/cancellai-provider-codex/src/session.rs::read_codex_parent_session_id` opens the rollout using `fs::File::open(path).ok()?`; the reader loop likewise returns `None` on an I/O error. `walk_rollouts` still adds the artifact using its readable directory-entry metadata, with `parent_session_id: None`, and does not record the failed content read in the scope observation. The Python reference's `read_codex_parent_session_id(..., scan)` catches `OSError`, records it, and causes withholding.

**Reproduction:** On a synthetic default `$HOME/.codex` and a synthetic custom `CODEX_HOME`, create a stale rollout with readable metadata, then `chmod 000` the rollout file. Run Python `clean --yes --days 1 --keep-latest 0 --tool codex`: exit 4, file survives. Against the Rust binary, `inspect --json` reported the Codex scope `complete=true,error_count=0`, and `plan --json --tool codex --days 1 --keep-latest 0` contained a `delete` action for the unreadable rollout. This reproduces the divergence in both root-origin scenarios. A real `clean` attempt can then fail at later identity/open validation because the file itself is unreadable; that does not undo the unsafe destructive plan already emitted.

**Required repair:** Return lineage parse results separately from I/O failures. Record open/read failures, with path and cause, in the same Codex `ScopeObservation`/`ReasonLog` used by the directory walker; preserve “no parent” only for successfully read content that has no recognized parent. Add a native CLI regression proving unreadable rollout bytes make the scope incomplete, produce no delete action, and match the Python exit code in default and custom root scenarios. Include a deterministic permission-change/read-failure case after entry metadata has succeeded.

### F-02 - Claude companion traversal has an unbounded error buffer

`walk_companion_payload` in `rust/crates/cancellai-provider-claude/src/session.rs` pushes every nested failure into `Vec<CompletenessReason>`. Only after traversal completes does the caller transfer those entries into `ReasonLog`, whose retention cap is 64. A provider tree containing a very large number of unreadable nested directories therefore retains an unbounded number of allocated path and message values during the walk. The scope's final count may be truthful and the retained output bounded, but the intermediate allocation is not bounded; this reopens the resource-exhaustion class E21-S03 said it repaired.

**Evidence/reproduction:** Source-level path: every `read_dir`, `DirEntry` and `metadata.modified()` failure in `walk_companion_payload` pushes to the unbounded vector; `ReasonLog::MAX_RETAINED_REASONS` is applied only after the helper returns. A synthetic Claude companion tree with more than 64 denied nested directories reaches this path (mode-000 directories are used in the successful partial-scan reproductions below); source inspection confirms all are retained before truncation.

**Required repair:** Have the walker record each failure directly into a bounded/counting reason accumulator (or return a bounded `ReasonLog`/`ScopeObservation`) rather than a `Vec`. Add an adapter-level regression with more than 64 independently failing nested paths asserting retained reasons stay capped, the total error count stays exact, the scope is partial, and every action is withheld.

## Adversarial cases

| Falsification axis | Pass performed and result |
| --- | --- |
| Path / identity changes | Reviewed root establishment, observation identity, pre-delete revalidation and the sealedfs child identity checks. No path-to-unlink bypass found in production call paths. The known leaf-name TOCTOU is recorded as residual below. |
| Partial reads / permissions | Claude unreadable project directory, companion payload directory and nested companion directory reproduced. Codex unreadable first-level scan directory, nested payload-shaped directory and nested directory reproduced. All cases were synthetic and run with default and custom roots: Python and Rust exit 4 and preserve the eligible readable artifact. The unreadable Codex rollout-file content case is F-01 and fails. |
| Links / mounts / reparse points | Reviewed no-follow traversal and `bind_existing` component walk. Symlink entries are not descended by Claude; Codex symlink directories are not descended; sealedfs opens each component without following symlinks. No native mount/reparse substitution test was available on this macOS host. |
| Provider version / layout drift | Both provider adapters and their dispatch/resolution callers were traced. Missing provider layout roots remain distinct from unreadable roots; the unreadable Codex rollout content gap is F-01. |
| Concurrency | Reviewed observation-to-plan flow and delete-time identity revalidation. Directory descriptors prevent a rename/symlink swap after `bind_existing` from redirecting unlink. No deterministic hook exists for revoking file read permission between `entry.metadata()` and lineage parsing; the direct unreadable-file case exposes the same swallowed-error branch. |
| Crash / failure / retry | Read/identity failure before unlink returns an error; post-unlink link-count failure can report after a link was removed. No crash-injection campaign was requested or run. |
| Boundary values / capacity | `ReasonLog` retention is bounded at 64 with a separate count, but Claude's companion helper builds an unbounded vector before that boundary (F-02). Codex lineage parsing remains bounded by 10 records / 512 KiB. |
| Policy / trust conflicts | Traced `ProviderResolution` and planning view into policy and action construction; incomplete scope normally downgrades actions to Observe. F-01 bypasses that guard by incorrectly supplying Complete. |
| Platform differences | Current host is macOS. Rust workspace gates compile/test host configuration only; Linux and Windows native reproductions were NOT RUN. Unix sealedfs uses `fstatat`/`unlinkat`; non-Unix behavior is separately implemented and was not natively exercised here. |
| Malformed / untrusted input | Malformed/unknown lineage content is treated as no parent as intended; an actual read error is indistinguishable from that result today (F-01). UUID and filename filtering remains in place. |
| Performance / large datasets | Bounded Codex parent reader reviewed and full gates pass. Claude's per-failure path/message vector can grow without limit (F-02); no large-scale memory benchmark was run. |

### E21-S07 unlink check

`confirmed_delete_file_inner` calls `SealedRoot::bind_existing(parent)`, then `unlink_child_matching_unix_identity`; the latter uses one held directory descriptor for `fstatat(..., AT_SYMLINK_NOFOLLOW)` and `unlinkat`. The directory path is therefore not re-resolved at the unlink call, and a later intermediate-directory swap cannot redirect it. The remaining documented limitation is narrower: POSIX cannot combine identity comparison and unlink atomically. If the leaf name is replaced after `fstatat` but before `unlinkat`, the replacement can be unlinked; checking the held original file's link count afterward detects the mismatch only after that side effect. This is already disclosed in E21-S07 and ADR-0017, but it is not equivalent to preventing every substituted leaf target from being unlinked. Owner disposition remains required before treating this residual as accepted for cutover.

No alternate production deletion path was found: the mutation-boundary checker passes and raw production file-removal primitives remain constrained to the sealedfs/platform boundary.

## Gates

| Gate | Result |
| --- | --- |
| Synthetic Claude project / companion / nested unreadable-directory comparison, Rust and Python, default + custom roots | PASS: 6 runs; all exit 4, eligible readable session survives |
| Synthetic Codex unreadable first-level, nested payload-shaped and nested directories, Rust and Python, default + custom roots | PASS: 6 runs; all exit 4, eligible readable session survives |
| Synthetic Codex unreadable rollout-content case, default + custom roots | FAIL: Python exits 4 and withholds; Rust reports complete with zero errors and emits a delete action |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/rust_python_parity.py check` | PASS: 13 NORMATIVE fixtures, both root-origin scenarios |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS: workspace tests and doctests passed |
| `cargo deny check` | PASS: advisories, bans, licenses and sources OK; existing unmatched-license/duplicate dependency warnings |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/verifier_handoff.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `python3 scripts/check_docs.py check` | PASS: 518 Markdown files; local links and safety IDs consistent |
| Linux and Windows native permission / unlink scenarios | NOT RUN: this verifier host is macOS; no remote native runner was invoked |

## Owner decision

`PENDING`

## Residual risks

- F-01 is carried as backlog story E06-S12; E06-S04 now explicitly waits on it. Cutover must remain blocked until repair and independent recheck.
- F-02 is carried with F-01 by backlog story E06-S12; memory consumed by companion failure reasons remains unbounded until that story is repaired.
- E21-S07's documented `fstatat`/`unlinkat` leaf-name race can unlink a replacement before post-check detection; owner disposition is pending.
- Native Linux and Windows behavior was not exercised in this session.

FAIL

## Owner disposition - E21-S07 leaf-name race (2026-09-23)

`ACCEPTED` as a residual, recorded by the executor at the owner's direction. On macOS and Linux
there is no unlink-by-descriptor, so a process running as the same user can always swap the leaf
between `fstatat` and `unlinkat`; a rename-verify-unlink sequence narrows the window but cannot
close it, and adds unsafe code and a new intermediate state. Whoever can win the race already has
the permissions to delete that file directly, so the race grants no capability the attacker lacks.
Windows deletes by handle and is not affected. This disposition covers only the residual; the
round-1 `FAIL` stands until E06-S12's repairs are independently rechecked.

## Independent verification — round 2 (2026-09-23)

Verifier: Codex
Brief-Checksum: 817de404f11c12559e5d15fc1f1c11e17aa4731db38847bbb757505ef9c96b65

### Invariants

| Invariant | Adversarial evidence | Result |
| --- | --- | --- |
| SI-008 | Independent synthetic Codex mode-000 rollout-content failure and Claude companion with 70 unreadable descendants: both scopes incomplete, zero delete actions, artifacts retained, in default and custom roots. The 14 normative fixtures match the Python reference in both origins. | PASS |
| SI-009 | Unreadable lineage is no longer interpreted as no parent/complete; read errors lead to exit 4 and withholding, as in Python. | PASS |
| SI-010 | Codex's open/read errors enter the scope `ReasonLog`; Claude walker records straight into its bounded log, with exact error count 70 and retention capped at 64 in the adapter test. | PASS |
| SI-019 | `check_mutation_boundary.py` passed. E21-S07 uses a held no-follow parent descriptor for the final unlink; an intermediate component swap cannot redirect it. | PASS_WITH_RESIDUALS |

### Adversarial cases

- Repeated the round-1 F-01 reproduction with a readable directory entry and unreadable rollout bytes. `inspect` reported complete=false/error_count=1, `plan` emitted zero deletes, Python exited 4, and the file survived in both root origins.
- Repeated the round-1 F-02 resource case with 70 denied companion directories. `inspect` reported 70 errors, zero deletes, Python exit 4 and file survival in both root origins. Source confirms no intermediate vector of reasons remains.
- Reviewed no-follow root/parent binding and `fstatat`/`unlinkat` placement. The leaf-name race remains exactly the prior owner-accepted residual; no claim of atomic leaf unlink is made.
- Cross-platform native execution beyond this macOS host was unavailable. Exact-main `rust.yml` quality and CLI jobs passed on macOS, Linux and Windows; those jobs do not directly reproduce this new permission fixture on every filesystem.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, workspace tests | PASS |
| Windows-target Clippy | PASS |
| Stable-channel CLI suite with kill-points, full rerun | PASS |
| `cargo deny --offline check` with writable advisory DB copy | PASS |
| Python tests; Rust/Python parity; mutation boundary | PASS |
| Native Windows hard-link and Unix leaf race reproductions | NOT RUN locally; Windows CI jobs passed, and leaf race was owner-dispositioned |

### Owner decision

PENDING

The existing owner acceptance above covers only the Unix leaf-name residual; it does not accept any new defect or close the cutover gate.

PASS_WITH_RESIDUALS
