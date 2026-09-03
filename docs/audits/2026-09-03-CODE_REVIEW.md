# Target Engine Review - 2026-09-03

## Scope

Reviewed the repository at commit `c00f16f` on `main`, a clean tree at released version
`v1.7.0`, with E00-E05 and E07 closed, E06 `in_progress`, and E06-S04 (the Rust cutover)
`blocked`.

This is a review of the **target engine**, not of the Python reference. The 2026-08-27
[baseline review](2026-08-27-CODE_REVIEW.md) covered `cancellai.py` and produced E00; that
work is closed and its repairs hold. This review asks the question the baseline could not:
whether the Rust engine that is meant to replace the reference has, today, the property the
whole product rests on - that unknown state never becomes destructive permission (C-02).

Review dimensions:

- destructive safety and authority boundaries;
- Python/Rust semantic parity and the machinery that proves it;
- concurrency and TOCTOU;
- resource behaviour under realistic input;
- gate integrity - whether a green gate constrains the shipped path;
- supply chain and release verification;
- suitability of the current plan for the remaining roadmap.

Findings carry stable IDs `CR-TE-01` through `CR-TE-13` (Code Review, Target Engine). Each
names the work item that carries it. Findings are converted into story contracts in
[E21](../BACKLOG.md) and [E22](../BACKLOG.md) rather than left as prose, following the
precedent the baseline review set.

### Method

Every gate in the repository contract was executed before any code was read, so that findings
would be measured against a passing baseline rather than against a broken checkout. All of
them passed:

| Gate | Result |
| --- | --- |
| `python3 -m pytest tests` | 179 passed, 22 subtests |
| `cargo test --workspace` | 298 passed, 1 ignored (scheduled benchmark) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | clean |
| 13 governance checkers under `scripts/` | all OK |
| `scripts/rust_python_parity.py check` | 10 NORMATIVE fixtures, both root-origin scenarios |

Reproductions in CR-TE-01, CR-TE-04 and CR-TE-07 were run against synthetic trees in a
temporary directory, never against real provider data, per `AGENTS.md`'s test rules. No
repository file was modified during the review itself.

## Executive assessment

**The engineering system is of rare quality, and it is not the problem.** A constitution with
numbered invariants; executor/verifier separation with adversarial rounds committed even when
they rejected the work; a machine-readable control plane from which the planning documents are
generated; thirteen governance checkers running in pre-commit and CI; a differential gate that
runs both engines over a synthetic corpus; exactly one `unsafe` crate, isolated by ADR, with a
`SAFETY` justification per block. Nothing in this review was found to be fabricated: where the
project does not know something, it writes that down.

**The target engine has not yet inherited the property that system exists to protect.** On an
ordinary filesystem tree - a directory without read permission - the frozen Python reference
withholds every destructive action and exits 4, and the Rust engine deletes and exits 0, while
reporting the scan as complete and its knowledge as verified. This was reproduced end to end
(CR-TE-01). It is not a theoretical race or a rare interleaving.

**Every critical finding has the same shape.** The gate exists, the gate is green, and the gate
does not observe the shipped path. The differential gate is rigorous in its mechanics and thin
in its corpus. The performance gate measures a crate the binary never calls. An entire closed
epic's completeness model is unreachable from production - and the exact defect its independent
reviewer rejected and forced repaired has reappeared in the code that replaced it. Adding a
fourteenth checker does not address this; the finding is about what the existing ones look at.

The recommendation is therefore **not** to slow down or add process. It is to point the
existing process at the binary that ships, and to treat E06-S04 as blocked on the authority
defect rather than on packaging alone.

## Critical findings

### CR-TE-01 - An unreadable directory does not make the scan incomplete

Severity: critical. Violates SI-008, SI-009, SI-010 and constitutional C-02.

Both Rust provider adapters discard a directory they cannot open, and record nothing:

- `rust/crates/cancellai-provider-codex/src/session.rs`, in `walk_rollouts`:
  `let Ok(entries) = fs::read_dir(&dir) else { continue; };`
- `rust/crates/cancellai-provider-claude/src/session.rs:86`, on project directories:
  `let Ok(children) = fs::read_dir(&project_path) else { continue; };`

The reference records the failure through `os.walk`'s `onerror` hook into its `Scan` object and
withholds the entire tool. The Rust engine reports `scan_complete: true`, keeps
`knowledge_confidence: verified` on everything it did see, and proceeds.

Reproduction, identical tree and parameters:

```text
$HOME/.codex/sessions/2026/01/01/rollout-…-1111.jsonl   readable, mtime 2020
$HOME/.codex/sessions/locked/                            chmod 000
$HOME/.codex/sessions/locked/inner/rollout-…-2222.jsonl  not observable

$ cancellai.py clean -y --days 1 --keep-latest 0 --tool codex --codex-backend filesystem
SCAN INCOMPLETE: 1 unreadable path(s) in codex
NOTE: Refusing destructive work on codex: the scan was incomplete, so absence of
      evidence cannot mean absence of data…
Nothing was cleaned: safety withheld the requested work.                 exit 4

$ cancellai-cli clean --yes --days 1 --keep-latest 0 --tool codex
1 artifact(s) deleted, 89 bytes reclaimed.                               exit 0

$ cancellai-cli inspect --json    ->  scan_completeness:
[{"scope":"claude-code","complete":true,"error_count":0},
 {"scope":"codex-cli","complete":true,"error_count":0}]
```

The Claude branch behaves the same way with an unreadable project directory under `projects/`:
the reference withholds, the engine reports `complete: true`.

**Disclosure status - partial, and at the wrong severity.** The Codex side *is* disclosed, in
[`RELEASE_GATES.md`](../development/RELEASE_GATES.md)'s cutover checklist - but under **G1
Functional**, described as a missing feature. The reproduction above shows it is an invariant
violation: it belongs under G2 Safety, and it is the strongest present reason E06-S04 cannot
close. The **Claude project-directory case is disclosed nowhere**; the same document states the
Claude side was repaired by E06-S02, which is true only for companion payload directories.

Required work: E21-S03, with the disclosure correction in E21-S01.

### CR-TE-02 - The single-pass inventory engine is unreachable from production

Severity: critical, architectural. Root cause of CR-TE-01's recurrence.

`cancellai-inventory` - epic E04, four stories, all `done`, providing `scan_scope`,
`derive_completeness`, `PlanningView` and the `Complete`/`Partial`/`Unknown` model - is not
referenced by any production crate. The only mention outside the crate is a comment in
`cancellai-provider-claude/src/lib.rs:8` explaining that `scan_scope` is *not* reused.

Three consequences compound:

1. **The defect returned.** E04-S03's independent reviewer rejected the story precisely because
   "a child returned by `read_dir` but unreadable to observation" was dropped rather than
   degrading completeness. The repair was made, verified and closed - inside `scan_scope`.
   CR-TE-01 is that same defect, alive again in the adapters, because the adapters do not pass
   through it.
2. **The performance gate measures code that never runs.** `performance_micro.rs`,
   `performance_scheduled.rs` and `.github/workflows/rust-benchmark.yml` all exercise
   `scan_scope`. Nothing measures the CLI's real discovery path - which G4 of the cutover
   checklist already concedes.
3. **"One traversal per scope" does not hold for the shipped pipeline.** The adapters walk the
   tree and then re-walk each companion directory for size.

The v1.5.0 release note recorded honestly that "nothing in this epic is wired into a shipping
CLI surface yet". The issue is that E05 and E06 then built the alternative instead of wiring
it, and the question was never reopened.

Resolved by [ADR-0018](../adrs/0018-scope-completeness-is-a-shared-type-not-a-shared-traversal.md):
completeness becomes a shared *type* obligation, the adapters keep their layout-specific
traversal. Required work: E21-S04, with the benchmark retarget in E21-S05.

## High findings

### CR-TE-03 - The differential gate is blind by construction to this class

Severity: high.

`scripts/rust_python_parity.py` is carefully built - two root-origin scenarios, approved
divergences bound to fixture, field and a cited ADR that must itself name the fixture. It
compares the engines only over the ten `NORMATIVE` fixtures, and the corpus has a hole shaped
exactly like CR-TE-01:

| Fixture category | Claude | Codex |
| --- | --- | --- |
| `normal_session`, `protected_state`, `symlink` | present | present |
| `subagent_tree` | n/a | present |
| `active_data` | present | absent |
| `layout_drift` | absent | present |
| `partial_tree` - companion directory | present | **absent** |
| `partial_tree` - project directory | **absent** | **absent** |

A differential gate is worth exactly what its corpus is worth. With one partial-scan fixture,
on one branch, of one provider, the gate could be green while the engine deleted data the
reference protects - which is what happened.

A related method observation: the gate has a good defect-injection self-test for *itself*
(`rust silently skipping a candidate python would delete must be caught`). There is no
equivalent for the *corpus* - nothing verifies that each declared safety invariant has at least
one fixture exercising it on each provider.

Required work: E21-S02.

### CR-TE-04 - Rollout metadata reading loads the whole transcript

Severity: high. Contradicts the function's own documented contract.

`read_codex_parent_session_id` documents reading "without scanning the whole file - bounded to
the first 10 lines / 512KiB". It calls `fs::read(path)` - the entire file - then
`String::from_utf8_lossy`, and only then applies the bound. The reference streams with a capped
`readline()` loop and is correct.

Measured, `status --tool codex` against a single 287 MB rollout:

| | peak RSS | wall clock |
| --- | --- | --- |
| Python reference | 27.8 MB | 0.16 s |
| Rust target engine | 303.0 MB | 0.35 s |

The cost scales with the largest transcript on disk, and agentic session transcripts grow
without bound. It is also the only point in this review where the Rust engine measured *slower*
than the Python one.

For context, `provider-api/src/root_probe.rs` has the same shape, but reads small configuration
markers - acceptable there, and worth stating explicitly rather than leaving as a coincidence.

Required work: E21-S06.

### CR-TE-05 - The deletion TOCTOU residual rests on a superseded premise

Severity: high.

`cancellai-platform/src/mutation.rs` justifies its unclosed unlink race by stating that true
prevention needs a handle-relative `unlinkat`, "via a reviewed `rustix`/`nix` dependency, or
`unsafe` libc calls, that this workspace does not have".

That is no longer true. `cancellai-sealedfs` exists (E07-S07/S09,
[ADR-0017](../adrs/0017-sealed-root-handle-for-configuration-writes.md)), depends on `libc`, is
the sole crate exempted from `unsafe_code = "forbid"`, and already implements
`openat`/`renameat`/`mkdirat` with `O_NOFOLLOW` plus a component-by-component handle-relative
walk. Adding `unlinkat` is a small extension well inside the crate's stated mandate.

The risk ordering is inverted: writing one JSON key into Claude Code's configuration is
protected by a sealed handle; irreversibly deleting a user's file is not. The current path is
narrow rather than open - it performs three identity checks around a held descriptor and
*refuses* rather than deleting the wrong object - but it remains detection where prevention is
now available, and the comment leads a reader to believe otherwise.

Related, in the same seam: `MutationOperation::Quarantine` uses a bare `fs::rename` and
`DeleteDirectoryTree` a bare `remove_dir_all`, neither identity-confirmed. No production caller
requests them and `mutation_executor` refuses both upstream - but they are armed, unconfirmed
variants inside the one file in the workspace permitted to delete, and E12 makes them live.

Required work: E21-S01 for the honest note now, E21-S07 for the prevention.

## Medium findings

### CR-TE-06 - The release workflow re-runs fewer than half the gates it claims to

Severity: medium.

`.github/workflows/release.yml` carries the right intent: "Re-run every gate at the tagged
commit. A release verified against whatever `main` looked like afterwards is not evidence about
the artifact users install." The job runs pytest, ruff, a subset of mypy, and four checkers.

Absent from the tagged-commit verification: `check_fixtures`, `check_schemas`, `characterize`,
`diff_harness`, `rust_python_parity`, `check_mutation_boundary`, `check_provider_compatibility`,
`check_rust_workspace`, `scripts/release.py check`, and every Rust check - `cargo test`,
`clippy`, `cargo deny`.

In practice the differential gate - the mechanism meant to prevent a release inconsistent with
the reference - never runs on the tagged commit. It runs on `main` via
`pre-commit run --all-files`, which covers nearly everything, but that is not the claim the
workflow makes about itself. This is the one place in the review where the repository was found
to be *less* rigorous than it writes.

Required work: E22-S01.

### CR-TE-07 - The Rust CLI has no `--help`, `-h` or `--version`

Severity: medium. Undisclosed.

```text
$ cancellai-cli --help      exit 2   [INVALID_INPUT] unrecognized flag '--help'
$ cancellai-cli -h          exit 2   [INVALID_INPUT] unrecognized flag '-h'
$ cancellai-cli --version   exit 2   [INVALID_INPUT] unrecognized flag '--version'
$ cancellai-cli help        exit 2   [INVALID_INPUT] unrecognized command 'help'
```

The reference has a full `argparse` surface, `docs/CLI.md` is generated from it, and the
Homebrew formula's smoke test asserts `cancellai --version`. `docs/CLI_RUST.md`'s "Known gaps"
section is otherwise scrupulous; this is missing from it.

This is where the uniform zero-dependency rule produced a real cost: the hand-rolled parser in
`main.rs` also silently accepts `--dry-run` on `status`, and must grow with every future
command. Addressed by [ADR-0019](../adrs/0019-dependency-rings-per-crate.md).

Required work: E22-S03, with disclosure in E21-S01.

### CR-TE-08 - The Rust supply chain has neither updates nor static analysis

Severity: medium.

- `.github/dependabot.yml` covers `github-actions` and `pip`. It does **not** cover `cargo`:
  `serde`, `serde_json`, `unicode-normalization` and `libc` - the last inside the only `unsafe`
  crate - receive no update proposals. `cargo deny check` runs and would report a RustSec
  advisory, but nothing proposes the fix.
- `.github/workflows/codeql.yml` analyses **Python only**. All the new security-critical code -
  authority kernel, FFI boundary, provider adapters - has no SAST. CodeQL supports Rust; it is
  one matrix line.

Both are small, and both sit oddly beside an epic titled "Verifiable Supply Chain and
Distribution".

Required work: E22-S02.

### CR-TE-09 - Test coverage is inverse to parity risk

Severity: medium.

| Crate | Lines in `src/` | `#[test]` | Role |
| --- | --- | --- | --- |
| `cancellai-safety` | 2,423 | 67 | safety kernel |
| `cancellai-policy` | 1,040 | 9 | classification and retention |
| `cancellai-provider-codex` | 1,356 | 32 | adapter |
| `cancellai-provider-claude` | 893 | 22 | adapter |

The kernel is well covered, correctly. But `cancellai-policy` - of which `retention.rs` alone is
968 lines - holds the port of `build_plan`, `choose_old_sessions` and
`choose_codex_old_sessions`: the highest density of hand-translated rules in the workspace, and
therefore the surface most exposed to semantic divergence. Its verification is effectively
delegated in full to the differential gate, which CR-TE-03 shows is fed by ten fixtures.

Required work: E22-S04.

## Low findings

### CR-TE-10 - Contract honesty details

Severity: low, but each is a claim a consumer could act on.

- `scan_completeness[].error_count` is computed as `u32::from(!scan_complete)`: always 0 or 1,
  never the real number of unreadable paths, which the reference does enumerate.
- `codex_delete_supported` / `NativeDeleteSupport` are implemented carefully in the Codex
  adapter but **not wired to `clean`**: the Rust CLI always deletes at the filesystem level,
  while the reference prefers `codex delete --force` specifically to avoid leaving orphaned
  metadata in Codex's SQLite indexes. This is a behavioural divergence on user data, and it is
  not among the "Known gaps".
- The parity gate's pre-commit `files:` pattern excludes `cancellai-provider-api` and
  `cancellai-safety`, so editing protected-name matching (`protection.rs`) does not re-run the
  gate locally. CI covers it via `--all-files`.

Required work: E21-S03 (`error_count`), E22-S05 (native delete), E22-S01 (hook pattern).

### CR-TE-11 - Unconfirmed mutation variants in the safety seam

Severity: low today, rising with E12. Recorded under CR-TE-05 and carried by E21-S07.

### CR-TE-12 - The control plane misstates the current phase

Severity: low, but it is a self-description defect in a system whose premise is that the
repository is the contract.

`project/roadmap.json` declared `current_phase: "P0"` while E00 and E01 were both `done` and P1
stood one epic from closing. `project/generated/PROJECT_STATUS.md` therefore generated
"Current phase: **P0**". Corrected to `P1` as part of registering this review.

Required work: E21-S01 (already applied).

### CR-TE-13 - The review-round ceiling has a blind spot

Severity: low.

`scripts/check_process.py` enforces ADR-0014's two-round bound with
`^(E\d{2})-VERIFIER-REVIEW.*\.md$`. Story-scoped records do not match it. E07 therefore counts
as one round while four were run - `E07-S07-VERIFIER-REVIEW.md`,
`E07-S07-VERIFIER-REVIEW-ROUND2.md`, `E07-S09-VERIFIER-REVIEW.md` and the epic record. This was
not an evasion: all four are committed, honest, and were the right thing to do. But the bound
ADR-0014 believes it is applying is not being applied.

Noted here because it also constrains this review's own remediation: **E06 has two counted
rounds and is at the ceiling**, which is why E21 and E22 are new epics rather than additions to
E06.

Required work: E22-S06.

## Positive engineering findings

Recorded because a review that only lists defects misrepresents the object under review.

- The safety kernel's construction is genuinely good. `SealedPlan` cannot be executed against a
  target bound under a different root; authority and reversibility are re-checked at execution
  against the plan's own recorded values; the process guard is re-evaluated immediately before
  deletion rather than trusted from plan time; `execute_all` cannot drop a result.
- `cancellai-sealedfs` is a model of how to isolate `unsafe`. One crate, one mandate, per-block
  `SAFETY` justification, fail-closed on unverified platforms, and an ADR that anticipated its
  existence before it was written.
- Failure states are honest rather than convenient. `IdentityObservation::Unsupported`,
  `NativeDeleteSupport::ProbeFailed` as distinct from `Unsupported`, and
  `SizeMetric` refusing to fabricate an allocated size are all cases where a plausible guess
  would have been easier and a refusal was chosen.
- The parity gate's structured `ApprovedDivergence` - fixture, scenario, named field, and a
  citation that must itself mention the fixture - is a better mechanism than most projects have
  for approved deviations, and it exists because a verifier round found the free-text version
  suppressing an unrelated failure.
- The review history is committed, including the rounds that rejected the work. That is rare
  and it is what made this review possible at the depth it reached.

## Recommendation

1. **Treat CR-TE-01 as the blocking cutover defect it is**, not as a functional gap. Reclassify
   it in the cutover checklist before repairing it, so the record is honest mid-repair
   (E21-S01).
2. **Write the fixtures before the fix** (E21-S02 before E21-S03). Both new fixtures must fail
   the differential gate against the current engine; a fixture that passes on the unrepaired
   engine is not exercising the defect.
3. **Repair the authority defect** (E21-S03), then make it structurally unrepeatable via the
   shared completeness type (E21-S04, ADR-0018), then point the performance gate at the shipped
   path (E21-S05).
4. **Adopt one standing rule for gates:** a gate must declare which production code it
   exercises, and fail if that code is not reachable from the shipped binary. This is what
   CR-TE-02, CR-TE-03 and CR-TE-06 have in common, and no additional checker addresses it.
5. **Resolve the cutover deadlock explicitly.** E06-S04 sits in P1 and depends on E20-S01
   (Windows native backend) and E17 (packaging), both later phases, and E20-S01 depends in turn
   on access to a real Windows environment. An ADR separating the *cutover perimeter* from the
   *tier-1 perimeter* - Rust canonical on macOS and Linux, Windows explicitly out of scope until
   E20 - would unblock it, and since the shipped product is macOS-only today, that is a widening
   rather than a reduction. **This decision is still open and belongs to the owner**; it is
   recorded here rather than pre-empted.
6. **Add a user-visible cadence measure.** Documentation, evidence and control plane are 40.7%
   of the repository; the artefact users install is 4.5% of it, frozen, and macOS-only. Nothing
   in the plan is wrong, but nothing in it currently answers how long the shipped product stands
   still. Among planned epics, E10 (storage accounting) and E09 (TUI) produce perceivable value
   substantially earlier than E08, E11 or E13 - and cross-provider visibility, the stated
   positioning, is what a TUI shows and an engine does not.
