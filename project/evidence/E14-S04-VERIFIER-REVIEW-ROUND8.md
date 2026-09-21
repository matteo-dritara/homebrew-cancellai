# E14-S04 Independent Verifier Review — Round 8 (test-coverage repair)

Review-Scope: epic
Round: 8
Review-Target: `2ce2391`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This is the one owner-authorized independent review round following the test-coverage repair
recorded in `project/evidence/E14-S04/EVIDENCE-ROUND5-TEST-COVERAGE-REPAIR.md`. It does
not replace any earlier review record.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | PASS_WITH_RESIDUALS | The new private hook closes the exact duplicated descriptor only after `readdir` has yielded a real child. The restored test passed six times on macOS; with only the `errno != 0` return temporarily bypassed, it failed with a truncated `Ok` list of 48 real entries. The source was restored byte-for-byte before all gates. This is a real regression pin for the `readdir` null/nonzero-errno branch, unlike round 7's pre-`try_clone` test. |

## Independent evidence and reproductions

### `readdir` errno branch

`list_child_names()` can only call `list_child_names_with_hook` with a no-op closure. The helper
first duplicates `self.dir`, transfers that duplicate to successful `fdopendir`, and calls its
hook only after a non-`.`/`..` entry has been read, classified, and appended. The test then closes
that exact duplicate. Thus neither `self.dir.try_clone()` nor `fdopendir()` can be the failure
point: both already succeeded, and a real entry was already produced.

On this macOS host, the committed test passed six consecutive times. For the required negative
control, I temporarily changed only `if errno != 0` to `if false && errno != 0`, ran the one
test, and restored the exact condition. The test then failed as required, reporting
`Ok([...])` with 48 collected real entries rather than `Err`. This proves the committed assertion
depends on the repaired null/nonzero-errno branch; it would not be satisfied by the prior
round-6 behavior.

The comment's stronger statement that 500 short names necessarily exceed every Linux `readdir`
buffer is not accurate for glibc: its directory stream allocation has a 32 KiB minimum, while
500 `file-NNNN` records occupy roughly 16 KiB. That does **not** defeat this test. Glibc refills
with `getdents64` whenever its internal buffer is exhausted; it does not infer EOF from a short
first fill. After all entries from even one fill are consumed, the next `readdir` needs that
fresh syscall to establish EOF. Because the test closed the descriptor after the first real
entry, that refill returns null with `EBADF`, which the restored code reads immediately and
returns as `SealError::Io`. On macOS the observed failure occurred before the complete listing;
on glibc Linux it may occur at the terminal refill, but in either case it is a genuine
`readdir` null/nonzero-errno result, not an earlier setup failure.

The production loop remains the correct POSIX discriminator:

```text
clear thread-local errno
entry = readdir(DIR*)
null + errno != 0 => Err(SealError::Io)
null + errno == 0 => EOF
```

No ordinary Rust operation lies between the clear, the `readdir` call, and the immediate
post-null errno read. The `DT_UNKNOWN` no-follow `fstatat` fallback still propagates its error
rather than guessing a non-directory result.

### Hook visibility and soundness

`list_child_names_with_hook` is private to `cancellai-sealedfs`' Unix implementation. Repository
search found exactly two uses: the public `list_child_names` wrapper with its no-op closure and
the in-module regression test. There is no public API that accepts a hook, no re-export, and no
caller of public `list_child_names` can inject one or affect the descriptor enumeration.

The hook itself executes in ordinary safe control flow after the `dirent` pointer has been copied
into owned Rust values and before the next `readdir`; it neither extends the `dirent` pointer's
lifetime nor changes `fdopendir` ownership. The production no-op does not touch the descriptor.
The test's raw `close` is explicitly unsafe and is confined to the crate's private test seam; no
subsequent operation opens a replacement descriptor before `DirGuard`'s one `closedir` attempt.
The existing `fdopendir` transfer, `DirGuard` sole ownership, errno-pointer locality, and
`fstatat(AT_SYMLINK_NOFOLLOW)` SAFETY arguments therefore remain sound.

## Invariant table

| Claim | Result | Evidence |
| --- | --- | --- |
| SI-004: incomplete/unreadable layout lowers capability | PASS | A real post-entry `readdir` failure is `Err`, not a successful listing. The negative control returns the old successful prefix/list, proving the test pins this property. |
| Round-7 errno and `DT_UNKNOWN` closure | PASS | The errno clear/check is immediately adjacent to `readdir`; the fallback still returns `SealError::Io` on failed no-follow `fstatat`. |
| Round-6 root/marker TOCTOU closure | PASS | `BoundLayoutObservation::observe` binds once through `SealedRoot`; held-fd `metadata` and duplicate-fd enumeration perform no post-bind root-path lookup. Workspace tests include the retained-root rename/replacement case. |
| Final/intermediate symlink refusal | PASS | The handle-relative `O_NOFOLLOW` walk remains the binding primitive; the workspace suite includes the final-symlink refusal fixture. |
| Round-5 observer-injection closure | PASS | `BoundLayoutObservation::observe` accepts only `&Path`; its compile-fail doctest rejecting a supplied observer passed in the workspace suite. |
| Round-4 `EffectiveAuthority`-to-permit separation | PASS | `AuthorityInputs` has no layout field; permit fields are private and the only mint requires `&BoundLayoutObservation`. No constructor/conversion/bypass was found. |
| `known_signatures` residual | PASS_WITH_RESIDUALS | It remains caller-supplied, with no trusted non-empty production source, exactly as ADR-0036 discloses. |
| No mutation-boundary permit consumer | PASS_WITH_RESIDUALS | `mutation_executor` still consumes no `ProviderExecutionPermit`; this remains ADR-0036's explicitly scoped residual. |
| Windows/non-Unix scope | PASS_WITH_RESIDUALS | The handle-bound listing/observation path returns `Unsupported` outside verified Unix implementation rather than fabricating an empty successful observation. |

## Adversarial cases tried

- Ran the new targeted test six times; all passed on macOS.
- Temporarily bypassed only the nonzero-errno return; the targeted test failed with a successful
  truncated list, then restored the exact production condition before further work.
- Audited glibc's `opendir` allocation and `readdir` refill implementation: its 32 KiB minimum
  means 500 short entries do not necessarily need two nonterminal fills, but buffer exhaustion
  still unconditionally causes the next `readdir` to make a fresh `getdents64` call and observe
  the injected `EBADF`.
- Searched hook visibility and every workspace use; no public injection or alternate production
  caller exists.
- Re-audited adjacent unsafe ownership/lifetime boundaries, errno accessor timing, and
  `DT_UNKNOWN` fallback propagation.
- Reconfirmed observer closure, sealed-root TOCTOU/symlink closure, authority-to-permit
  separation, and the stated residuals through source search and the workspace suite.

## Gate results

| Command | Result |
| --- | --- |
| Targeted new readdir regression (six runs) | PASS on macOS |
| Targeted negative control with only errno return bypassed | FAIL as required; returned `Ok` prefix/list; temporary change reverted |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo test --workspace` | PASS |
| `cd rust && cargo deny check` | PASS — existing unmatched-license and duplicate-crate warnings only; advisories, bans, licenses, and sources passed |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `gh run list --branch main --limit 5` | NOT GREEN — the configured remote is `matteo-dritara/homebrew-cancellai`; its latest scheduled `rust-benchmark` run failed in that different repository. The four latest project workflow runs listed were successful, but this is not evidence for the unpushed review target. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `project/epics/E14.json`
- `project/evidence/E14-S04/VERIFIER_BRIEF.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND6.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND7.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-READDIR-ERRNO-REPAIR.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-TEST-COVERAGE-REPAIR.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-TOCTOU-REPAIR.md`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/WORK_ITEM_MODEL.md`
- `docs/development/RELEASE_GATES.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/PLATFORM_MODEL.md`
- `docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`
- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
- `rust/crates/cancellai-sealedfs/src/lib.rs`
- `rust/crates/cancellai-sealedfs/src/windows_sealed.rs`
- `rust/crates/cancellai-platform/src/provider_layout.rs`
- `rust/crates/cancellai-safety/src/authority.rs`
- `rust/crates/cancellai-safety/src/provider_layout.rs`
- glibc `opendir.c` and `readdir64.c` source (directory-buffer allocation/refill semantics)

## CR4 Safety Verdict — E14-S04

## Verdict

`PASS_WITH_RESIDUALS`

The round-8 test repair closes round 7's verification defect. It reaches `readdir` only after
descriptor duplication, `fdopendir`, and a real entry have succeeded; it fails under a minimal
temporary removal of the errno branch and passes with the branch restored. The private hook
offers no public production injection or alternate enumeration path, and it leaves the reviewed
unsafe ownership and pointer-lifetime arguments intact. SI-004's incomplete-enumeration case is
therefore independently pinned.

The remaining `known_signatures`, absent mutation-boundary consumer, and
Windows/non-Unix-unsupported residuals are unchanged, accurately disclosed, and fail closed in
the current scope. This review authorizes E14-S04 to close subject to the owner accepting this
CR4 Safety Verdict and the repository's normal closure/release process.
