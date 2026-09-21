# E14-S04 Independent Verifier Review — Round 6 (TOCTOU repair)

Review-Scope: epic
Round: 6
Review-Target: `9a793f538e7c497a48aad1cbdc4c826a86bfb570`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This is the one owner-authorized independent review round following the TOCTOU repair recorded
in `project/evidence/E14-S04/EVIDENCE-ROUND5-TOCTOU-REPAIR.md`. It does not replace any prior
round's record.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | FAIL | The root-swap and final-symlink defects are closed, but the new `list_child_names()` treats a `readdir()` error as end-of-directory and returns a partial marker list as `Ok`. An exact known-layout comparison can therefore accept a favorable prefix and omit a real drift marker, preserving capability when the observation is incomplete. This is a fail-open implementation of a disclosed residual, not an acceptable residual under SI-004/C-02. |

## Independent evidence and reproductions

### Prior root-swap interleaving

The old interleaving cannot be reconstructed after `SealedRoot::bind_existing(root)` succeeds.
`BoundLayoutObservation::observe` has one path resolution: the handle-relative,
`O_NOFOLLOW | O_DIRECTORY` component walk in `bind_existing`. It then obtains identity through
`File::metadata()` (`fstat` of the held descriptor) and markers through `fdopendir`/`readdir` on
a duplicate of that descriptor. Neither operation re-resolves `root` by pathname.

I re-ran the committed adversarial primitive test:

```text
cargo test -p cancellai-sealedfs metadata_and_list_child_names_survive -- --nocapture
```

It passed. The test binds `root` containing `config.json`, renames that directory away, creates a
replacement at the old pathname containing only `decoy.txt`, then performs both reads on the
already-bound `SealedRoot`. The post-swap metadata retains the original inode and the listing is
exactly `[ ("config.json", false) ]`, not the decoy. This genuinely reproduces the required
rename-after-bind placement, rather than only testing a rename before binding.

I also audited the public call graph: `BoundLayoutObservation::observe` exposes no hook between
bind and its two descriptor-bound reads, and no remaining `canonicalize`, `symlink_metadata`,
`read_dir`, or other path-based lookup appears in that interval. A pathname replacement or
symlink substitution after bind can no longer pair one root object's identity with another root
object's marker list.

### Final symlink

I re-ran:

```text
cargo test -p cancellai-platform refuses_a_symlinked_root -- --nocapture
```

It passed. `bind_existing` reaches the final component via `openat` with `O_NOFOLLOW` and
`O_DIRECTORY`; an `ELOOP` is refused as `IsSymlinkOrReparsePoint`, with the macOS/BSD `ENOTDIR`
case separately classified using handle-relative `fstatat(..., AT_SYMLINK_NOFOLLOW)`. Thus the
final symlink is refused rather than followed. Intermediate components use the same no-follow
walk.

### New fail-open enumeration counterexample

`list_child_names()` breaks whenever `libc::readdir(guard.0)` returns null. POSIX specifies that
null means either end-of-directory or an error; callers that need the distinction must clear and
then inspect `errno`. The implementation does neither. `readdir` can fail, including with
`EOVERFLOW`, and a partial scan may already have populated `entries`.

The authority comparison is exact-set equality:

```text
known_signatures contains ["sessions/"]
actual bound root contains "sessions/" and "unexpected-drift"
readdir returns "sessions/", then fails before "unexpected-drift"
list_child_names returns Ok([("sessions", true)])
BoundLayoutObservation records ["sessions/"]
resolve_provider_execution_authority finds a matching known signature and adds no
provider_capability_authority Observe ceiling
```

This is not merely observational cosmetic loss. It can report an incomplete real provider root as
recognized at the point `resolve_provider_execution_authority` determines whether to add the
SI-004 layout ceiling. The existing absence/unreadable-root tests cover an error before
enumeration, not an error after a favorable prefix. The method documentation calls this
"reported as complete rather than as an error" and characterizes it as narrower than a full read
failure; that understates the authority consequence. An unknown/incomplete layout must lower
capability, not be treated as a complete recognized signature.

I attempted an external dynamic `readdir`-error interposer against a small temporary public-API
harness. On this macOS host the interposer loaded but did not intercept the Rust libc call, so it
is not claimed as a runtime reproduction. The counterexample is nevertheless directly compelled
by the POSIX `readdir` contract and the source control flow; no error discriminator is present.

The `DT_UNKNOWN` fallback has the same fail-open shape for its own I/O error: a failed
`fstatat(..., AT_SYMLINK_NOFOLLOW)` currently turns into `false` (a regular marker) rather than
`SealError::Io`. For a known regular-file marker, this too can claim a successful classification
without its required evidence.

## Unsafe-code audit

The new unsafe ownership and pointer-lifetime reasoning is sound:

| Operation | Result | Evidence |
| --- | --- | --- |
| `fdopendir` ownership transfer | PASS | `try_clone().into_raw_fd()` gives the call a distinct owned descriptor. On success, only `DirGuard::drop` calls `closedir`; the original `self.dir` remains open. On null, `fdopendir` did not take ownership and `File::from_raw_fd(dup_fd)` drops it exactly once. No leak or double-close path found. |
| `DirGuard` early returns | PASS | The guard is created immediately after successful `fdopendir`; every subsequent `?`/return and normal completion drops it once. Ignoring `closedir`'s diagnostic return cannot transfer ownership back or create a second closer. |
| `readdir` `dirent *` validity | PASS | Each returned pointer is read only until the next `readdir`/`closedir` call on that `DIR *`. `d_name`, `d_type`, and the fallback `fstatat` use occur before the loop's next call. |
| `fstatat` fallback containment | PASS_WITH_FINDING | It is relative to the held `self.dir` and uses `AT_SYMLINK_NOFOLLOW`, so it cannot escape/follow a child link. Its error result is nevertheless incorrectly collapsed to `false`, as described above. |
| `readdir` null result | FAIL | The `// SAFETY:` claim permits repeated calls, but the surrounding safe logic assumes every null is EOF. POSIX requires `errno` discrimination, and the omission is safety-relevant in this exact-signature consumer. |

## Invariant table

| Claim | Result | Evidence |
| --- | --- | --- |
| Round-5 root/marker binding | PASS | One retained no-follow descriptor binds the root; `fstat` and descriptor-relative listing describe that retained object after a root rename. |
| Symlinked root is refused | PASS | `bind_existing` no-follow walk and targeted platform test. |
| Round-4 `EffectiveAuthority`-to-permit bypass is closed | PASS | `AuthorityInputs` has no layout field; `effective_authority` returns `EffectiveAuthority`, while the only permit mint requires `&BoundLayoutObservation`; permit fields remain private. |
| Observer injection closure from round-5 first pass | PASS | `observe` takes only `root`; the compile-fail doctest rejecting `SyntheticIdentityObserver` ran in the workspace suite. No alternate constructor/builder/conversion was found. |
| `known_signatures` residual | PASS_WITH_RESIDUALS | It remains caller-supplied and no production call has a non-empty source, as ADR-0036 discloses. This does not excuse an I/O failure being classified as a successful complete signature. |
| No mutation consumer | PASS_WITH_RESIDUALS | Repository search finds no `mutation_executor`/real mutation consumer of `ProviderExecutionPermit`; this is the ADR-0036 residual, not a reason to weaken the observation primitive. |
| Windows/fallback scope | PASS_WITH_RESIDUALS | Windows methods return `Err(SealError::Unsupported(...))`; generic fallback instances are unconstructible and `bind_existing` itself returns `Unsupported`. No platform returns an empty/default `Ok`, so the narrowing is honestly fail-closed. |
| Unknown/incomplete layout lowers capability (SI-004) | FAIL | A `readdir` or fallback `fstatat` error can be silently represented as a complete, matching marker signature. |

## Adversarial cases tried

- Bound root renamed away and decoy directory placed at the old name: passed; original identity
  and original direct children were retained.
- Final symlink root: refused, not followed.
- Walk audit for intermediate links and post-bind path lookup: no bypass found.
- Old observer argument and external construction closure: compile-fail doctest passed; fields
  remain private with no secondary production constructor/conversion.
- `fdopendir` null branch, `DirGuard` ownership, `readdir` pointer lifetime, and descriptor
  fallback audit: ownership/lifetime safe; logical error handling fails open as above.
- Mid-stream `readdir` error and `DT_UNKNOWN` `fstatat` error: source-level counterexamples
  found. External interposition attempt was inconclusive and is not counted as a reproduction.
- Windows/non-Unix implementation audit: explicit `Unsupported` errors only.

## Required repair and owner decision

This is a `FAIL` in the one owner-authorized review round. No further patch is self-authorized.

The required repair is precise:

1. In `list_child_names`, clear the platform `errno` immediately before every `readdir` call;
   when it returns null, return `Err(SealError::Io(...))` if `errno` is nonzero and end normally
   only when it remains zero. The implementation needs appropriately documented, platform-scoped
   unsafe access to `errno`.
2. When the `DT_UNKNOWN` fallback `fstatat` returns nonzero, return `Err(SealError::Io(...))`;
   do not convert a failed classification into `(name, false)`.
3. Add deterministic fault-injection tests proving that a failure after a recognized prefix, and
   a fallback classification failure, make `BoundLayoutObservation::observe` fail rather than
   mint markers that can match a known layout. Re-run the full CR4 gate set and obtain a fresh
   owner decision before another repair/review cycle.

## Gate results

| Command | Result |
| --- | --- |
| Targeted sealedfs rename test | PASS |
| Targeted platform symlink test | PASS |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo test --workspace` | PASS — includes the platform compile-fail closure doctest. |
| `cd rust && cargo deny check` | PASS — existing unmatched-license and duplicate-crate warnings only; advisories, bans, licenses, and sources passed. |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `gh run list --branch main --limit 5` | UNKNOWN — GitHub API could not be reached from this session; CI is not treated as green. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `project/epics/E14.json`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `docs/architecture/PLATFORM_MODEL.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/adrs/0017-sealed-root-handle-for-configuration-writes.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND4.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5-PASS2.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-TOCTOU-REPAIR.md`
- `rust/crates/cancellai-sealedfs/src/lib.rs`
- `rust/crates/cancellai-sealedfs/src/windows_sealed.rs`
- `rust/crates/cancellai-platform/src/provider_layout.rs`
- `rust/crates/cancellai-platform/src/identity.rs`
- `rust/crates/cancellai-safety/src/authority.rs`
- `rust/crates/cancellai-safety/src/provider_layout.rs`
- POSIX `readdir(3)` specification: https://pubs.opengroup.org/onlinepubs/7908799/xsh/readdir.html

## CR4 Safety Verdict — E14-S04

## Verdict

`FAIL`

The owner-authorized TOCTOU repair correctly binds root identity and marker enumeration to one
retained, no-follow descriptor and correctly refuses a symlink root. Its new enumeration API,
however, converts I/O failure into an apparently complete layout. Because its immediate consumer
uses exact marker-signature equality to decide whether to omit the SI-004 ceiling, a partial
favorable prefix can preserve capability for an unknown/incomplete layout. This violates SI-004,
C-02, C-05, and TM-05.

## Owner decision

E14-S04 cannot close on this verdict. The exact repair above requires a fresh owner decision
before implementation and another independent review; this verifier has not modified the
implementation.
