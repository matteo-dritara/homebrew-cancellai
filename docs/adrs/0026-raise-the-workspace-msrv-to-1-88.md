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

**2. An advisory the repository's own gate did not report.** Independent review corrected
this explanation on 2026-09-13: GHSA-rhfx-m35p-ff5j also exists in RustSec as
[RUSTSEC-2026-0002](https://rustsec.org/advisories/RUSTSEC-2026-0002.html). The omission was
scope, not database coverage. `cargo-deny 0.20.2` defaults `advisories.unsound` to
`"workspace"`, which only covers direct workspace dependencies. `lru` is transitive.
Re-running the old graph returned success with the old configuration and failed specifically
on this advisory with `unsound = "all"`. E17-S08 now sets that scope explicitly while
keeping `ignore = []`. The updated graph passes the stronger check.

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

Option A, for the reasons in Options below: 1.88 was released in mid-2025 and the executor's local compiler was
1.94, so the compatibility being given up is the older 1.85 toolchain, against three
concrete costs being paid for it today.

## Options

**A. Raise the MSRV to 1.88.0.** Clears the `lru` advisory by making `ratatui 0.30` adoptable,
probably clears the `paste` ignore, and lets the kernel use let-chains. Costs: anyone building from
source needs 1.88 (released mid-2025; the executor's local compiler was 1.94), and the promise in ADR-0015 changes, which is a real thing to change rather than a
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
  that had been silently skipped: five `collapsible_if` sites it now wants written as let-chains,
  and one `manual_is_multiple_of`. Three sites are in floored crates
  (`cancellai-platform/src/wsl.rs`, `cancellai-platform/src/process.rs`,
  `cancellai-safety/src/knowledge_bundle.rs`), which is what
  makes E17-S08 a CR4 story rather than the CR2 configuration change it was filed as. This is the
  right direction - those lints were always applicable and the old MSRV was hiding them - but it
  means an MSRV bump is not a configuration change here, it is a code change in the kernel.
- **The macOS normal dependency graph grew by 18 packages**, 98 to 116, measured with
  `cargo tree --locked --offline --workspace --edges normal --prefix none --format '{p}'`,
  excluding blank lines and deduplicating package identities. This includes the 13 workspace
  packages and excludes build/dev edges; it is not a count of all rustc compilation units.
  Linux is 100 to 118 and Windows GNU 98 to 115 under the same target-specific method.
  `Cargo.lock` grew from 123 to 225 package entries: net +102, comprising 127 introduced
  name/version pairs and 25 removed pairs. The earlier packet
  reported 115, which is neither net growth nor the complete package-identity delta. Of the 127 new
  pairs, 33 are reachable through normal/build edges across all targets; 94 are unselected
  under those features. Reverse trees for every new pair and unchanged kernel-ring trees
  confirm that selected new dependencies reach workspace consumers only through the TUI.
- **`crossterm` had to be aligned.** `ratatui 0.30` uses `crossterm 0.29` while `cancellai-tui`
  still declared 0.28 during the upgrade, so an intermediate resolution compiled both: two versions of the crate that owns raw mode and the event
  stream, in one process. `cargo deny` only warns on duplicates, so nothing would have stopped it.
  `cancellai-tui` now declares 0.29 and one version is compiled.
- **One API break**, in `cancellai-tui`: `border::Set` gained a lifetime parameter, so the eight
  functions taking it now say `border::Set<'static>`. The values were always `&'static str`
  literals.
- **E16-S07's rewrite stays.** It replaced three let-chains in the bundle verifier with
  `is_some_and` to hold the old promise; clippy is content with it and reverting correct code for
  symmetry would be churn. The two tests it added for the expiry boundary are worth keeping
  regardless of which version this workspace compiles against.

## What this ADR does not decide

Whether a second advisory source is useful remains a separate decision. It is not needed to
see this particular advisory: the correction above demonstrates that RustSec already had it.

## Independent safety repair

The E17-S08 verifier also reproduced a pre-existing mismatch with ADR-0024 in the edited
bundle module: permissive signature verification was called instead of the required strict
method. The owner authorized repairs found during review; AC6 records this additional scope.
The repair calls `verify_strict`, rejects weak local-policy keys/signatures and preserves the
current bundle on refusal. This is an intentional security behavior correction, separate
from the six behavior-preserving lint rewrites.
