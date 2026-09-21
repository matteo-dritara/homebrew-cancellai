# Evidence Packet - E14-S04 (round 5, TOCTOU repair)

- Commit/PR: repairs the finding in `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5-PASS2.md`
  (`FAIL`)
- Executor: Claude
- Independent verifier: Codex (pending)
- Change Risk: CR4
- Spec version/commit: `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-
  observation-type.md` ("Round 5, second self-correction" section)

## Outcome

PARTIAL (repair complete, awaiting independent review - the owner explicitly authorized this
specific repair and one further review round given the second review pass's finding; a further
FAIL requires a fresh owner decision, not a further self-authorized attempt)

## Context

The second independent review pass on the identity-binding repair
(`project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5-PASS2.md`) found `BoundLayoutObservation::
observe` still vulnerable: `SystemIdentityObserver.observe(root)` (`symlink_metadata`) and
`std::fs::read_dir(root)` are two separate path-based syscalls, so a same-user concurrent actor
could rename the real, drifted root away and substitute a recognized directory between them,
producing an observation that pairs the original root's identity with the replacement's markers.
The same review found the constructor did not refuse a symlinked root - identity observed the
link, `read_dir` followed it to the target.

The owner was consulted (given this repair's depth touches `cancellai-sealedfs`, the one crate
with `unsafe_code` permitted) and explicitly authorized implementing the fix and one further
review round.

## Repair

`rust/crates/cancellai-sealedfs/src/lib.rs` (Unix): two new `SealedRoot` methods.
`metadata()` returns the held descriptor's own `fstat` (`std::fs::File::metadata`) - never a
fresh path lookup. `list_child_names()` enumerates direct children via `fdopendir`/`readdir` on a
*duplicate* of the held descriptor (a duplicate because `fdopendir` takes ownership of the fd it
is given, and `self.dir`'s original fd must not be double-closed), classifying each entry's kind
via `d_type` with an `fstatat(..., AT_SYMLINK_NOFOLLOW)` fallback when the filesystem does not
report it. Both draw from the one descriptor `SealedRoot::establish`/`bind_existing` already
bound with a handle-relative, `O_NOFOLLOW`-at-every-component walk. Windows and the generic
fallback implementation return `SealError::Unsupported` for both - no verified handle-bound
implementation exists there yet, matching this crate's own established precedent for every other
capability without one.

`rust/crates/cancellai-platform/src/provider_layout.rs`: `BoundLayoutObservation::observe`
rewritten to call `cancellai_sealedfs::SealedRoot::bind_existing(root)` once, then
`.metadata()`/`.list_child_names()` on that one bound object, converting the resulting
`std::fs::Metadata` into a `cancellai_platform::IdentityToken` via a small Unix-only helper
(mirroring `identity.rs`'s own `observe_system_identity` conversion logic exactly). No
`canonicalize()` call was added before binding - deliberately, since resolving symlinks first
would silently follow exactly what `bind_existing`'s own no-follow walk exists to refuse; `root`
must already be absolute and normalized, matching `bind_existing`'s own contract.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| TOCTOU finding closed | `cancellai-sealedfs`'s `metadata_and_list_child_names_survive_a_root_rename_after_binding`: renames the bound root away and plants a decoy at the original path after binding; both `metadata()` and `list_child_names()` still report the original object | PASS |
| Symlinked-root refusal | `cancellai-platform`'s new `refuses_a_symlinked_root_rather_than_following_it`: a symlink root is refused, not followed | PASS |
| Existing round-5 guarantees preserved | Full workspace test suite re-run: 0 failures, including all prior `e14s04_*`/`provider_layout::tests::*`/new `cancellai-sealedfs::unix_impl::tests::*` | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                                    -> PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings -> PASS
cd rust && cargo test --workspace                                               -> PASS (0 failed)
cd rust && cargo deny check                                                     -> PASS (advisories ok, bans ok, licenses ok, sources ok)
python3 scripts/check_rust_workspace.py check                                   -> PASS (13 crates match TARGET.md, acyclic, model/safety isolated)
python3 scripts/check_mutation_boundary.py check                                -> PASS (unaffected - list_child_names/metadata are read-only)
python3 scripts/project_os.py check                                             -> PASS
```

## Documentation updated

- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
  ("Round 5, second self-correction" section added)
- Module docs in `rust/crates/cancellai-platform/src/provider_layout.rs` and
  `rust/crates/cancellai-sealedfs/src/lib.rs` (new method docs)

## Method defects

- **What happened**: the identity-binding repair (previous evidence file in this directory)
  reasoned that removing the *parameter* closed the forgeability gap, without examining whether
  the two internal syscalls it replaced the parameter with were themselves atomic - they were
  not. **Prevented by**: this codebase already states the general rule elsewhere
  (`cancellai-sealedfs`'s own module docs: "a re-check before use cannot provide [safety], and
  only a *retained* capability can"), but nothing prompted checking a *new* identity-observation
  primitive against that existing, already-articulated rule before treating a narrower repair as
  complete. **Disposition**: proposed - the `risk-gate`/`adversarial-cases` skills could name
  "does this primitive read multiple facts about one object via more than one path-based lookup"
  as an explicit check for any new CR4 observation primitive.

## Residual risks

- **Windows has no `BoundLayoutObservation::observe` implementation** (new, disclosed in
  ADR-0036): `SealedRoot::metadata`/`list_child_names` fail closed with `Unsupported` on
  non-Unix platforms. `observe` therefore only succeeds on Unix today. Accepted because no
  production caller exists yet on any platform; a future story bringing this to Windows needs a
  handle-bound `NtQueryDirectoryFile`-based implementation in `windows_sealed.rs`.
- Unchanged from ADR-0036: `known_signatures` has no trusted source yet, and no mutation-boundary
  call site consumes a permit yet.
- `cancellai-sealedfs::list_child_names`'s own disclosed residual: a `readdir` failure partway
  through a stream is not distinguished from reaching the end of it (both return a null entry).

## Verifier verdict

PENDING - awaiting the one independent review round the owner authorized for this repair.
