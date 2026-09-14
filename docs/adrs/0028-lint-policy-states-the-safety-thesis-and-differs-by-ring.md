# ADR-0028: The lint policy states the safety thesis, and differs by ring

- Status: Accepted
- Date: 2026-09-14
- Owners: project owner / cEOS
- Related: [ADR-0015](0015-rust-workspace-toolchain-and-repository-layout.md), [ADR-0017](0017-sealed-root-handle-for-configuration-writes.md), [ADR-0019](0019-dependency-rings-per-crate.md), E27-S01

## Context

Until this decision, `[workspace.lints]` held exactly one line: `unsafe_code = "forbid"`. `cargo
clippy -- -D warnings` ran clippy's default set and nothing else.

That is a defensible default for most projects and a strange one here. This workspace's entire
argument is that a tool which deletes files must never act on an unproven assumption, and clippy's
default set says nothing about the two ways that argument fails in Rust: an `unwrap` that aborts
mid-operation, and an `unsafe` block whose soundness argument was never written down.

Measuring it first, which nobody had done:

- **41 `unsafe` blocks, 30 `SAFETY:` comments.** Eleven blocks carried no written justification,
  including one added the previous day under ADR-0027. Four of those were on the platform the
  executor's machine compiles; the other seven only exist behind `cfg(windows)` and were invisible
  to any local run.
- **Thirteen panic sites in production code**, six of them indexing or slicing strings that come
  from third-party provider manifests. None was reachable in practice; each was sound because a
  human had checked a chain of two to four interacting facts.
- **First-ever coverage measurement: 94.5% regions overall**, and the worst kernel-ring file was
  `cancellai-sealedfs/src/lib.rs` - the crate holding every `unsafe` block and the mutation
  boundary - at 92.6%.

And one structural defect the measurement exposed. Cargo's `[lints]` table is **all-or-nothing**:
a crate that declares its own replaces the workspace table rather than extending it.
`cancellai-sealedfs` must declare one, because ADR-0017 lifts `unsafe_code` and `forbid` is the one
level an inner attribute cannot lift. While the workspace table held a single lint this cost
nothing. It meant that the moment the workspace held a real policy, **the one crate containing
every `unsafe` block in the repository was the only crate that policy could not reach.**

## Decision

`[workspace.lints.clippy]` denies the lints that state the thesis: `undocumented_unsafe_blocks`,
`unwrap_used`, `panic`, `todo`, `unimplemented`, `dbg_macro`, `mem_forget`, `indexing_slicing`, and
`unsafe_op_in_unsafe_fn` from the `rust` set.

Three deliberate boundaries.

**Tests are exempt.** `rust/clippy.toml` sets `allow-unwrap-in-tests`, `allow-expect-in-tests`,
`allow-panic-in-tests` and `allow-indexing-slicing-in-tests`. A test that unwraps is asserting;
production code that unwraps is choosing to abort a run part-way through. Integration tests live
outside `#[cfg(test)]`, so those options do not reach them and each such file carries the
equivalent inner attribute with the reason written above it.

**`expect_used` is deliberately not denied.** `CString::new("/").expect("literal has no embedded
NUL")` is the correct idiom for an invariant that holds by construction, and denying it pushes
authors toward swallowing the case silently instead of naming why it cannot happen.

**The policy differs by ring, and the difference is declared in source.** `indexing_slicing` is
denied because a panic in code deciding what may be deleted is a wrong answer arriving as a crash.
`cancellai-tui` decides nothing - it renders what the engine already decided - and carries
`#![allow(clippy::indexing_slicing)]` at its crate root with that reasoning. An exemption belongs
where a reader of the code will see it, not in a manifest.

`scripts/check_rust_workspace.py` now fails if a crate's local lint table drops a workspace lint.
`unsafe_code` is the one permitted difference, because it is the one ADR-0017 exists to grant.

## Consequences

- Every `unsafe` block in the workspace carries a `SAFETY:` comment, and the compiler keeps it that
  way. 41 of 41, up from 30.
- Thirteen production panic sites are gone. Two were real hazards rather than theoretical: a plan
  action naming no target artifact aborted the CLI mid-run, and the Windows rename path inside the
  mutation boundary indexed a buffer whose size was computed three lines earlier.
- The glob matcher that parses provider manifest patterns is panic-free by construction and is now
  checked against a naive reference implementation over every pattern and input up to length four
  across an alphabet containing a multi-byte character - differential testing, the method this
  repository already uses between its Python reference and its Rust port, applied to a parser that
  had never been tested that way.
- A crate can no longer silently opt out of the policy.
- `cancellai-sealedfs`'s manifest repeats the clippy table. That duplication is real and is the
  price of `forbid` being unliftable; the gate above is what keeps it from drifting.

## Why there is no `rust-toolchain.toml`

Recorded here because it is the obvious next step and it is wrong.

Pinning the toolchain in the tree looks like the missing piece: it would make every contributor
compile with the version this workspace promises, and it would have caught the MSRV drift that
went unnoticed for four days. It would also **collapse the MSRV matrix, silently**.
`.github/workflows/rust.yml` selects toolchains through `dtolnay/rust-toolchain`'s `toolchain:`
input, and a `rust-toolchain.toml` under `rust/` takes precedence over rustup's default for any
`cargo` invoked there. Every leg would compile with the pinned version while the job names still
read `stable` and `1.88.0`, and the matrix that exists to catch exactly this class of drift would
report green on one toolchain tested three times.

Pin the toolchain in CI, where the pin is visible in the job name. What is genuinely missing
locally is a 1.88 toolchain to check against, and installing one is an owner decision, not an
executor's.

## Alternatives considered

**Enable `clippy::pedantic` or `clippy::restriction` wholesale.** Rejected: both include lints that
fight the house style, and a policy whose violations are mostly noise gets `#[allow]`-ed at the
crate root within a month, which is worse than not having it.

**Apply the strict lints only to the kernel ring.** Rejected as the default, kept as the exception:
starting strict everywhere and relaxing where a crate can argue for it produced exactly one
exemption, and it is written down. Starting narrow would have left the CLI's panics unexamined.

**Deny `expect_used` too, with `#[allow]` at each proven site.** Rejected: it converts a readable
justification into an attribute plus a comment, for no additional proof.
