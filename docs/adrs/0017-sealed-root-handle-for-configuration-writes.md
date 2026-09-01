# ADR-0017: A sealed, no-follow root handle closes `configure`'s TOCTOU, in its own unsafe-isolated crate

- Status: Accepted
- Date: 2026-09-01
- Owners: project owner
- Related: ADR-0013, ADR-0015, E06-S01, E07-S07, SI-002, SI-003, SI-013, SI-019

## Context

E07-S07's outcome is "reject provider roots whose root object or path resolution crosses a
symbolic-link, junction, or reparse boundary before any Rust CLI mutation or provider
configuration write." A first closure session (recorded in `project/evidence/E06-S01/
EVIDENCE.md` and this story's own evidence packet) added `roots::is_symlink` classification-time
and execution-time re-checks to both `clean` (via `establish_verified_root`/`ApprovedRoot`) and
`configure` (a standalone check in `cmd_configure`, since `configure` writes Claude Code's own
vendor settings file and deliberately does not go through `ApprovedRoot`/`MutationExecutor` -
SI-019's own scope is provider-artifact deletion, not a vendor settings-key write).

The independent verifier's round-1 review (`project/evidence/E07-S07-VERIFIER-REVIEW.md`,
`project/evidence/E07-S07/SAFETY_VERDICT.md`) found that `configure`'s re-check did not close
the gap it was meant to: `cmd_configure` called `roots::is_symlink(&claude_resolved.path)`, and
on success passed the same raw path to `configure_claude_retention`, which then performed
`create_dir_all`, `read_to_string`, `OpenOptions::open`, and `rename` against
`claude_home.join(...)` - each a fresh path lookup. An attacker (a same-user, same-machine
process racing the CLI, not a hypothetical) who atomically replaces the real default root with
a symlink between the `is_symlink` check and the first of those path lookups causes every
following operation to silently follow the link and write outside the approved root, violating
SI-002/SI-003/SI-013/SI-019 exactly as `AGENTS.md`'s constitutional non-negotiables define them.
`clean`'s equivalent path does not have this specific gap: `ApprovedRoot::establish`/`bind`
(`cancellai-safety`) already canonicalizes and binds before mutation. `configure` had no
equivalent capability to bind to.

A path re-checked immediately before use is not enough to close this class of gap; only a
*retained* capability is, and this workspace already has one open, adjacent, disclosed instance
of wanting exactly this and not having it: `cancellai-platform::mutation`'s own module docs
(E03-S05) describe the identical need for its file-deletion path - "true prevention ... needs an
OS-specific handle-relative unlink (`openat`/`unlinkat` with `O_NOFOLLOW`) ... via `unsafe` or a
reviewed dependency ... that this workspace does not have." ADR-0015 anticipated this exact
shape of future need in the abstract ("a future story needs `unsafe` (for example OS-specific
identity/reparse-point handling in `cancellai-platform`, E07), it is isolated in a small,
dedicated crate whose only job is that unsafe boundary") without naming the crate, its
dependency, or its API, because none of those existed yet to decide.

## Decision

We add `cancellai-sealedfs`, a new workspace crate whose only job is a handle-relative
directory capability, `SealedRoot`:

- `SealedRoot::establish(path)` creates the directory if absent (safe against a pre-planted
  symlink - `mkdir(2)`'s underlying primitive fails outright against anything already at that
  path rather than following it), then opens it with `O_NOFOLLOW | O_DIRECTORY`. That open,
  not the presence check that precedes it, is the actual authority boundary: it is
  unconditional, and refuses regardless of what a caller saw microseconds earlier.
- `read_child_to_string`/`write_new_child_atomically` perform every subsequent operation via
  `openat`/`renameat` against the *retained* directory descriptor from that one `establish`
  call, never the original path again. The kernel resolves an `*at()` call relative to the
  descriptor's own bound object, not whatever name currently occupies the path that produced
  it - a rename or symlink-swap of the root's path at any later point cannot redirect either
  call.
- This needs real `openat`/`renameat` FFI, which `std` does not expose safely. `cancellai-
  sealedfs` is the one crate in the workspace exempted from `unsafe_code = "forbid"`
  (`[lints.rust] unsafe_code = "allow"` in its own `Cargo.toml`, not a workspace-wide relaxation
  - ADR-0015's `forbid` cannot be locally overridden by `#[allow]` in the same crate, by
  design, which is exactly why the exemption has to live at the crate boundary instead). Every
  `unsafe` block carries its own `// SAFETY:` comment.
- `libc` (MIT OR Apache-2.0, already inside `rust/deny.toml`'s allow-list) supplies the FFI
  signatures rather than hand-written `extern "C"` declarations - `mode_t`'s width alone
  differs between Linux and macOS, and getting a hand-rolled ABI declaration subtly wrong in a
  security-boundary crate is a materially worse risk than depending on the Rust project's own
  zero-dependency crate that already gets this right on every platform it supports. It is
  scoped to `[target.'cfg(unix)'.dependencies]` - non-Unix builds do not pull it in at all.
- Non-Unix platforms have no verified reparse-safe equivalent yet. `SealedRoot::establish`
  there always returns `Err(SealError::Unsupported(..))` - fails closed, mirroring
  `cancellai-platform::identity`'s own `IdentityObservation::Unsupported` precedent for the
  identical reason (SI-017: an unverified safety-critical implementation is a worse outcome
  than an honest refusal). `configure` therefore currently refuses on every non-Unix platform,
  the same posture `clean` already has there via `ApprovedRoot::establish` failing closed on
  `Unsupported` identity - this is a real, disclosed capability change (`configure` previously
  attempted the raw path operations there too, unprotected, whenever `$HOME` happened to
  resolve), not a regression introduced by an oversight.
- `cancellai-cli::configure_claude_retention` is rewritten against this capability in place of
  the previous raw `std::fs`/path sequence. `cmd_configure`'s pre-existing `is_symlink`
  diagnostic check is kept (fast, legible error text on the common case) but is no longer the
  thing actually closing the race; `SealedRoot::establish` is.
- A deterministic adversarial unit test (`cancellai-sealedfs::unix_impl::tests::
  establish_rejects_a_root_swapped_to_a_symlink_after_final_validation_but_before_the_bind`)
  reproduces the verifier's exact scenario via a test-only hook between the presence check and
  the authoritative open (mirroring `cancellai-platform::mutation`'s own
  `confirmed_delete_file_inner` hook pattern for its analogous race), and proves an outside
  sentinel file is never created.

`read_child_to_string` deliberately does **not** use `O_NOFOLLOW`: a pre-existing
`settings.json` that is itself a symlink is a distinct, already-verified case (E06 verifier
review round 1's `configure_never_writes_through_a_preexisting_settings_json_symlink_to_an_
outside_file`) whose accepted safety property lives entirely on the write side (`O_EXCL` +
`renameat`, which never follows a symlink at either name regardless of what the read did) -
matching `std::fs::read_to_string`'s own follow-symlink behavior and `cancellai.py`'s
`settings.read_text()`. Refusing to follow a settings-file-level symlink was never this story's
gap and is out of scope for this ADR; only the *root directory's* link safety is what changed.

## Alternatives considered

### Re-check the path again, closer to the write

This is exactly the shape that produced the round-1 finding (`is_symlink` immediately before
`configure_claude_retention` was already "closer to the write" than the classification-time
check E06 fixed). Any check-then-path-operate design has a non-zero gap between the two
syscalls; only binding to an already-open descriptor removes the gap rather than shrinking it.
Rejected as insufficient by construction, independent of how tightly the check is placed.

### Extend `ApprovedRoot`/`MutationExecutor` to cover `configure`

`ApprovedRoot` binds to an artifact/root pair for a `SealedPlan`-driven deletion; `configure`
neither deletes a provider artifact nor derives a plan (SI-019's own text: "all
filesystem/*vendor* mutations route through the safety executor" already carves this out
explicitly in `docs/security/SAFETY_INVARIANTS.md`, and `docs/CLI_RUST.md` documents `configure`
as bypassing the mutation boundary on that basis). Rejected: retrofitting the artifact-deletion
kernel to also model an arbitrary vendor-file write would be a larger, less coherent change than
a small, dedicated capability for exactly what `configure` needs.

### A reviewed `openat`-capable dependency (`rustix`, `nix`) instead of `libc` + local `unsafe`

Both wrap the same syscalls in a safe API, removing the need for this crate's own `unsafe`
blocks entirely. Considered seriously - either would satisfy ADR-0015's dependency-review bar
on functional grounds. Rejected for now in favor of `libc` + narrow, individually-justified
`unsafe`: this crate needs exactly three calls (`openat`, `renameat`, and the flags on an
already-safe `open`), each with a short, checkable safety argument; adding a larger
syscall-wrapping crate's own transitive surface and API-stability commitment for three calls is
more dependency than the problem needs. This is not a rejection of either crate on principle -
a future story that needs a broader handle-relative surface (e.g. closing
`cancellai-platform::mutation`'s own disclosed unlink race) may reasonably reach a different
conclusion, and should re-evaluate at that point rather than inherit this one by default.

### Hand-written `extern "C"` syscall declarations, no new dependency at all

Avoids adding `libc` to the dependency tree. Rejected: `openat`/`renameat`/`mkdir`'s C
signatures include platform-varying types (macOS's `mode_t` is `u16`; Linux's is `u32`, among
other differences across the BSDs), and getting one of these subtly wrong in a
security-boundary crate risks silent, hard-to-detect undefined behavior for no benefit over
depending on the Rust project's own crate that already encodes every platform's correct ABI.
"Do not add a dependency merely to reduce implementation effort" (`AGENTS.md`) is about effort,
not about correctness in an unsafe FFI boundary - the two are not the same tradeoff here.

### Leave `configure` on Windows attempting the raw path operations it always did

Silently continues whatever `configure` already did on Windows (which needed `$HOME` to be set
at all - `roots.rs` only reads `$HOME`, not `%USERPROFILE%`, disclosed separately). Rejected:
the independent verifier's required repair explicitly names "implement or explicitly fail
closed" as the two acceptable outcomes for Windows; silently keeping the unprotected path is
neither, and is inconsistent with `clean`'s own existing fail-closed posture there via
`IdentityObserver::Unsupported`.

## Consequences

### Positive

- The exact race the round-1 review demonstrated is closed, not narrowed: once `establish`
  returns, no operation this crate performs can be redirected by anything that happens to the
  root's original path afterward.
- `git grep unsafe rust/crates` now returns hits from exactly one crate, matching ADR-0015's
  stated goal for the workspace's trusted computing base.
- The read/write split (`O_NOFOLLOW` only where the write path's safety actually depends on it)
  preserves every previously-accepted, tested behavior (E06 round 1's settings.json-symlink
  case) rather than silently tightening something this story was not scoped to change.

### Negative / cost

- A new workspace crate and a new external dependency (`libc`), both requiring the review this
  document constitutes.
- `configure` now fails closed on every non-Unix platform outright, not only when the root is
  actually a link - a real, disclosed capability reduction there until a future story
  implements a verified Windows reparse-safe handle (E07-S02 is the natural home for that).
- `SealedRoot::establish`'s create-if-absent branch still has a small window between
  `create_dir_all` succeeding and the subsequent `O_NOFOLLOW` open (an attacker would need to
  delete the just-created empty directory and install a symlink in that instant) - inherent to
  `mkdir` having no atomic "create and return a handle" primitive on any platform this crate
  targets, and a materially narrower window than the one this ADR closes (which spanned an
  entire read-modify-write cycle, not one syscall pair). Disclosed as a residual, not treated as
  closed.

### Neutral / follow-up

- `cancellai-platform::mutation`'s own disclosed unlink-race residual is unrelated to and not
  closed by this crate; a future story that wants to close it may reuse `cancellai-sealedfs` or
  reach its own decision on `rustix`/`nix` versus `libc` for that different operation shape.
- A genuine Windows reparse-safe handle implementation (E07-S02) should extend or replace
  `cancellai-sealedfs`'s non-Unix fallback rather than leaving `configure` permanently refused
  there.

## Safety and compatibility impact

- Change Risk implication: CR4 (E07-S07's own classification) - this changes the authority
  boundary for a provider-configuration write.
- Safety Invariants affected: SI-002, SI-003, SI-013, SI-019 - see `docs/security/
  SAFETY_INVARIANTS.md`'s updated SI-002/SI-003/SI-013 entries for the implementation
  cross-reference.
- Migration/rollback: reversible at the code level (revert to the previous path-based
  `configure_claude_retention`, reintroducing the closed gap) but not a decision to take
  lightly given what round-1 review found; no persisted state or on-disk format changes.

## Supersession

If replaced later (e.g. a genuine Windows reparse-safe handle, or a switch to `rustix`/`nix`),
keep this ADR and mark it superseded by the ADR that replaces it.
