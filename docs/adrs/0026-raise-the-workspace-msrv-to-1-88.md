# ADR-0026: Raise the workspace MSRV to 1.88

- Status: Accepted
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

**Accepted by the owner on 2026-09-13**, and carried by E17-S08. `rust-version` is **1.88.0**,
`ratatui` is 0.30, `rust/deny.toml`'s ignore list is empty, and idiomatic 2024-edition code no
longer has to be rewritten to avoid a version nobody was asking for.

Option A, for the reasons in Options below: 1.88 was released in mid-2025 and current stable is
1.94, so the compatibility being given up is a toolchain a year and a half old, against three
concrete costs being paid for it today.

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

## What it actually cost

Recorded after the fact, because two of these were not predicted above and a decision record that
only lists the costs it guessed right is not worth keeping.

- **`lru` went to 0.18.4 and `paste` left the graph**, as expected. GHSA-rhfx-m35p-ff5j is closed
  and `rust/deny.toml` now has an empty `ignore` list - a supply-chain gate with no waivers.
- **Clippy's MSRV-gated lints woke up.** Clippy reads `rust-version`, so raising it enabled lints
  that had been silently skipped: four `collapsible_if` sites it now wants written as let-chains,
  and one `manual_is_multiple_of`. Two of them are in floored crates
  (`cancellai-platform/src/wsl.rs`, `cancellai-safety/src/knowledge_bundle.rs`), which is what
  makes E17-S08 a CR4 story rather than the CR2 configuration change it was filed as. This is the
  right direction - those lints were always applicable and the old MSRV was hiding them - but it
  means an MSRV bump is not a configuration change here, it is a code change in the kernel.
- **The dependency graph grew by 18 crates**, 99 to 117, from `ratatui 0.30`'s split into
  `ratatui-core`/`ratatui-widgets`/`ratatui-crossterm` plus `kasuari` replacing the old layout
  solver. `Cargo.lock` grew by 115 entries, which is the number a careless reading reports: most of
  those are optional dependencies of the new crates - the `termwiz`, `termion` and `termina`
  backends - that are never compiled, because `ratatui`'s default features select `crossterm`
  only. The compiled graph is the number that matters and it is 18.
- **`crossterm` had to be aligned.** `ratatui 0.30` uses `crossterm 0.29` while `cancellai-tui`
  declared 0.28, so both were compiled: two versions of the crate that owns raw mode and the event
  stream, in one process. `cargo deny` only warns on duplicates, so nothing would have stopped it.
  `cancellai-tui` now declares 0.29 and one version is compiled.
- **One API break**, in `cancellai-tui`: `border::Set` gained a lifetime parameter, so the four
  functions taking it now say `border::Set<'static>`. The values were always `&'static str`
  literals.
- **E16-S07's rewrite stays.** It replaced three let-chains in the bundle verifier with
  `is_some_and` to hold the old promise; clippy is content with it and reverting correct code for
  symmetry would be churn. The two tests it added for the expiry boundary are worth keeping
  regardless of which version this workspace compiles against.

## What this ADR does not decide

Whether `cargo deny` should be supplemented with a second advisory source that sees GitHub's
database. That is a separate question with its own cost, and it is the reason this one was found
late.
