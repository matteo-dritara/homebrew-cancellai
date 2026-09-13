# ADR-0026: Raise the workspace MSRV to 1.88

- Status: Proposed
- Date: 2026-09-13
- Owners: project owner / cEOS
- Related: [ADR-0015](0015-rust-workspace-toolchain-and-repository-layout.md), [ADR-0019](0019-dependency-rings-per-crate.md), E09-S01, E09-S05, E16-S07, E22-S08, E17-S08

## Context

ADR-0015 fixed the workspace's minimum supported Rust version at **1.85.0**, the minimum that
edition 2024 requires. Nothing has argued against that number since, and nothing here argues that
1.85 was the wrong choice when it was made. What has changed is what it now costs, and the cost
became visible only when the MSRV CI leg started working again.

Three separate things are held back by it, and they were found in one afternoon:

**1. A security advisory that cannot be cleared.** `lru 0.12.5` is subject to
[GHSA-rhfx-m35p-ff5j](https://github.com/advisories/GHSA-rhfx-m35p-ff5j) (severity **low**:
`IterMut` violates Stacked Borrows by invalidating an internal pointer), fixed in `lru 0.16.3`. It
reaches this workspace through `ratatui 0.29`, which pins `lru ^0.12`. The version that takes a
fixed `lru` is `ratatui 0.30`, whose own `rust-version` is 1.88.0. Dependabot has opened a security
update for it three times and each run fails, because no resolution satisfies the advisory and the
MSRV at once.

Two qualifications, because the severity matters to the decision and overstating it would be the
easy mistake: it is an unsoundness report rather than a demonstrated exploit, and nothing in this
workspace calls `lru::IterMut` - it is ratatui's internal render cache, reached through a
crate this workspace forbids `unsafe` in. The finding is real; the exposure is not obviously
anything.

**2. An advisory the repository's own gate does not see.** `cargo deny check` reports
`advisories ok` on this same lock file, because it reads the RustSec database and this advisory is
in GitHub's. The gate is not broken, but its coverage is narrower than "we check advisories"
suggests, and the only reason anyone knows about `lru` is a Dependabot run failing in a way nobody
was reading.

**3. An unmaintained dependency already accepted for the same reason.**
`rust/deny.toml` ignores RUSTSEC-2024-0436 (`paste`, unmaintained) with a comment that already
names this decision: "no ratatui alternative ... without either bumping MSRV workspace-wide (a
separate, dedicated ADR-0015 decision) or forking `paste`".

**4. The workspace's own code has required 1.88 since 2026-09-09.** E16-S02 wrote three
let-chains - stable, idiomatic 2024-edition Rust, stable from 1.88 - in the knowledge-bundle
verifier. `cancellai-safety` has not compiled on the MSRV leg since. E16-S07 rewrote them with
`is_some_and` to restore the promise rather than renegotiate it under time pressure, which is the
right default for an executor and is not an argument that the promise is right.

That last one is the part worth sitting with. The declared MSRV and the real one had diverged for
four days without anyone noticing, across two attempted releases, because the leg that would have
said so was failing for an unrelated reason. A promise nothing can check is not a promise.

## Decision

**Proposed, not taken.** This ADR exists to put the decision in front of the owner with its
evidence, because it changes a published compatibility promise and that is not an executor's call.

The proposal: raise `rust-version` to **1.88.0** in `rust/Cargo.toml`, adopt `ratatui 0.30`, remove
the RUSTSEC-2024-0436 ignore from `rust/deny.toml` if 0.30 drops `paste`, and stop rewriting
idiomatic 2024-edition code to avoid a version nobody is asking for.

## Options

**A. Raise the MSRV to 1.88.0.** Clears the `lru` advisory by making `ratatui 0.30` adoptable,
probably clears the `paste` ignore, and lets the kernel use let-chains. Costs: anyone building from
source needs 1.88 (released mid-2025; current stable is 1.94, so the ask is a toolchain a year and
a half old), and the promise in ADR-0015 changes, which is a real thing to change rather than a
formality.

**B. Keep 1.85.0.** Costs: the `lru` advisory stays open and Dependabot keeps failing on it; the
`paste` ignore stays; every let-chain anyone writes has to be rewritten; and the MSRV leg stays a
gate that constrains the kernel's own source. Buys: distributions and users on older toolchains can
still build, which is the thing the promise is for. Nobody has asked for this, which is not the
same as nobody needing it.

**C. Keep 1.85.0 and vendor or fork around ratatui.** Rejected without much hesitation: forking a
rendering dependency to avoid a low-severity unsoundness in a cache it does not expose is a worse
trade than either A or B, and it puts this project in the business of maintaining someone else's
crate.

The story that would carry the change if this is accepted is **E17-S08**, which stays `planned`
until then.

## Consequences if accepted

- `rust/Cargo.toml`'s `rust-version`, ADR-0015's text, and the MSRV matrix in
  `.github/workflows/rust.yml` change together, in one story, with the `ratatui` bump.
- `docs/PLATFORMS.md` and any build-from-source instructions that name a toolchain change with it.
- E16-S07's rewrite stays. It is correct either way and the tests it added are worth keeping.

## Consequences if rejected

- The `lru` advisory is accepted as a recorded residual risk with the reasoning above, and the
  Dependabot security update for it is dismissed rather than left failing every day - a failing
  job nobody can act on is how the MSRV break went unnoticed for four days.

## What this ADR does not decide

Whether `cargo deny` should be supplemented with a second advisory source that sees GitHub's
database. That is a separate question with its own cost, and it is the reason this one was found
late.
