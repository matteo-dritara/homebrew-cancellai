# E14-S04 Independent Verifier Review — Round 7 (readdir errno repair)

Review-Scope: epic
Round: 7
Review-Target: `b1b94d4`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This is the one owner-authorized independent review round following the readdir errno repair
recorded in `project/evidence/E14-S04/EVIDENCE-ROUND5-READDIR-ERRNO-REPAIR.md`. It does not
replace any earlier round's record.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | FAIL | The production errno and `DT_UNKNOWN` repairs close round 6's fail-open path, but the committed regression test closes `root.dir` before `list_child_names()` calls `try_clone()`. It therefore returns through `try_clone`'s `EBADF` error path and never reaches `fdopendir`, `readdir`, the errno clear/check, an accumulated prefix, or the `DT_UNKNOWN` fallback. It is a test of a different early failure, not a deterministic reproduction of the repaired class. |

## Independent evidence and reproductions

### errno boundary

On this macOS host, the BSD-family cfg arm selects `libc::__error()`. That is the Darwin C
library accessor for the calling thread's `errno` storage; its result is a valid thread-local
`*mut c_int`. The implementation neither forms nor retains a Rust reference to that storage: it
writes through the raw pointer immediately before `readdir`, obtains the same thread-local
storage again immediately after a null result, reads it, and discards both pointers. There is no
aliasing or lifetime escape.

The Linux/Android arm uses the corresponding libc accessor, `__errno_location()`, with the same
thread-local contract. The BSD cfg includes macOS, iOS, FreeBSD, NetBSD, OpenBSD, and DragonFly,
where `__error()` is the appropriate libc spelling. Each covered platform has one accessor only,
and the helpers are private to the Unix implementation.

The relevant loop is now exactly:

```text
*errno_location() = 0
entry = readdir(guard.0)
if entry == NULL:
    errno = *errno_location()
    errno != 0 => Err(SealError::Io(...))
    errno == 0 => EOF
```

No Rust, allocation, string conversion, `fstatat`, guard drop, or other ordinary call occurs
between the clear, `readdir`, and null-result read. This is the documented `readdir` idiom:
null with unchanged zero is end-of-directory; null with nonzero errno is an error. An arbitrary
application-installed signal handler that clobbers errno in the tiny post-call interval could
defeat any use of this libc idiom; no such handler exists in this call path, and a conforming
handler preserves errno. In the normal execution model, the attribution is sufficient.

### Round-6 counterexample, before and after

The prior code had no errno discriminator. A directory whose first visible entry was the known
`sessions/` marker, then whose next `readdir` returned null with `EIO`/`EBADF`/`EOVERFLOW`, returned
`Ok([("sessions", true)])`. Exact signature equality could then call that favorable prefix
recognized while omitting a real later drift marker, preserving capability contrary to SI-004.

The current production loop cannot return those collected entries after such a null: the nonzero
errno produces `SealError::Io`. The `DT_UNKNOWN` branch likewise now returns `SealError::Io` on a
failed descriptor-relative, no-follow `fstatat`, rather than converting unknown evidence to
`false`/not-directory. These are the correct fail-closed outcomes.

I re-ran the committed test:

```text
cargo test -p cancellai-sealedfs \
  list_child_names_fails_rather_than_silently_succeeding_on_a_broken_descriptor -- --nocapture
```

It passed, but it is not a faithful reproduction of the claimed class. `root.dir` is closed
*before* the method begins. The first operation in `list_child_names` is
`self.dir.try_clone()`; a closed descriptor makes that operation fail with `EBADF`, returning
before `fdopendir` and the loop. The test does not demonstrate a null `readdir`, a nonzero errno
read, an error after a favorable prefix, or an inability to mint a matching truncated marker list.
It would also pass if the newly-added errno logic and `readdir` check were removed while the
initial `try_clone` error propagation remained.

### Unsafe, fallback, and test-lifetime audit

| Operation | Result | Evidence |
| --- | --- | --- |
| `errno_location` pointer | PASS | The covered cfg branch calls the platform libc's thread-local errno accessor. Its raw pointer is used immediately and never stored or converted to a reference. |
| errno timing | PASS | Only the clear, `readdir`, and immediate null-result read occur on this path. A real read error becomes `SealError::Io`; EOF remains the sole zero-errno null outcome. |
| `DT_UNKNOWN` fallback | PASS in production; unpinned by test | `fstatat(dirfd, name, ..., AT_SYMLINK_NOFOLLOW)` remains bound to `self.dir`, does not follow a child link, and now propagates a nonzero return as `SealError::Io`. No committed deterministic test forces this branch's failure. |
| `fdopendir`/`DirGuard` ownership and dirent lifetime | PASS | The duplicated fd transfers once to `fdopendir`; `DirGuard` alone invokes `closedir`; every entry pointer is consumed before the next `readdir`/`closedir`. Round-6's ownership conclusions remain sound. |
| `ManuallyDrop` in the test | PASS | The only fd was deliberately closed already. Suppressing `File`'s later destructor prevents I/O-safety's correct double-close abort; it leaves no live kernel fd or heap resource for this process to leak or later reuse. The test performs no later operation through `root`. |

## Invariant table

| Claim | Result | Evidence |
| --- | --- | --- |
| SI-004: incomplete layout lowers capability | FAIL (verification incomplete) | The repaired implementation fails closed, but no deterministic regression proves the actual `readdir`-error-after-prefix or `DT_UNKNOWN`-error path. The only supplied regression test misses both. |
| Round-6 root/marker TOCTOU closure | PASS | `SealedRoot::bind_existing` retains the no-follow descriptor; `metadata()` uses `fstat` and listing uses a duplicate descriptor, with no post-bind path lookup. The rename-after-bind test passed. |
| Final/intermediate symlink refusal | PASS | The handle-relative `O_NOFOLLOW | O_DIRECTORY` walk refuses the final symlink; the platform symlink test passed. |
| Round-5 observer injection closure | PASS | `BoundLayoutObservation::observe` accepts only a root path; the public observer seam and alternate production constructor remain absent, and the compile-fail doctest passed in the workspace suite. |
| Round-4 `EffectiveAuthority`-to-permit separation | PASS | `AuthorityInputs` carries no layout fact. The opaque permit is minted only by `resolve_provider_execution_authority` from `&BoundLayoutObservation`; no public constructor or conversion was found. |
| `known_signatures` residual | PASS_WITH_RESIDUALS | It remains caller-supplied and has no trusted non-empty production source, as ADR-0036 discloses. |
| No mutation-boundary permit consumer | PASS_WITH_RESIDUALS | Repository search still finds permit construction/use only in the safety API and its tests, not at the mutation executor. |
| Windows/non-Unix scope | PASS_WITH_RESIDUALS | The new descriptor-bound layout observation remains `Unsupported` where not verified, rather than returning an empty/default successful observation. |

## Adversarial cases tried

- Re-ran the committed broken-descriptor test; it passed, and source-order analysis established
  that it exercises `try_clone` failure before `readdir`.
- Re-ran the retained-root rename/replacement primitive; it retained the original directory
  identity and markers after the old pathname received a decoy replacement.
- Re-ran final-symlink refusal; it was refused rather than followed.
- Audited every operation between errno clear and post-null read; no ordinary call can overwrite
  errno there.
- Audited the `DT_UNKNOWN` failure result, `fdopendir` ownership, `DirGuard`, temporary dirent
  pointer use, and test-only `ManuallyDrop` lifetime.
- Searched authority/permit callers, constructors, conversions, observer seams, and mutation
  consumers; prior permit, observer, and TOCTOU closures remain intact.
- Checked disclosed residuals for `known_signatures`, lack of a mutation consumer, and
  Windows/non-Unix refusal; all remain accurately stated.

## Required repair and owner decision

This is a `FAIL` in the one owner-authorized review round. No further patch is self-authorized.

The required repair is precise: add deterministic, in-process fault injection that makes
`readdir` first yield an entry belonging to a recognized signature and then return null with a
nonzero errno. Assert `list_child_names` returns `Err`, and through
`BoundLayoutObservation::observe` cannot produce a marker list usable to mint a recognized
permit. Add a separate deterministic `DT_UNKNOWN`/`fstatat`-failure test that asserts the same
error result. The test seam must reach those calls after `fdopendir` succeeds; invalidating the
descriptor before `try_clone` is not sufficient. Re-run the CR4 gate set and obtain a fresh owner
decision before another repair/review cycle.

## Gate results

| Command | Result |
| --- | --- |
| Targeted broken-descriptor regression test | PASS, but it exercises `try_clone`/`EBADF`, not `readdir` errno handling. |
| Targeted retained-root rename test | PASS |
| Targeted platform final-symlink test | PASS |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo test --workspace` | PASS — includes the observer-injection compile-fail doctest. |
| `cd rust && cargo deny check` | PASS — existing unmatched-license and duplicate-crate warnings only; advisories, bans, licenses, and sources passed. |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `gh run list --branch main --limit 5` | UNKNOWN — GitHub API was unreachable at session start; CI is not treated as green. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `project/epics/E14.json`
- `project/evidence/E14-S04/VERIFIER_BRIEF.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND6.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-READDIR-ERRNO-REPAIR.md`
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
- Linux `readdir(3)` manual: https://man7.org/linux/man-pages/man3/readdir.3.html

## CR4 Safety Verdict — E14-S04

## Verdict

`FAIL`

The errno and fallback changes themselves correctly turn the round-6 incomplete-enumeration
counterexample into an error, preserving SI-004's fail-closed posture. However, the alleged
deterministic regression test fails before the code it claims to verify. It cannot detect a
removal or regression of the errno discriminator, the null-result error handling, the
recognized-prefix case, or the `DT_UNKNOWN` fallback propagation. The CR4 fix lacks the required
adversarial regression proof; E14-S04 cannot close on this verdict.

## Owner decision

The required test repair needs a fresh owner decision before implementation and another
independent review. This verifier has not modified implementation, project control-plane state,
or any file other than this verdict record.
