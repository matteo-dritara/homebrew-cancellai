# ADR-0015: Rust workspace toolchain and repository layout

- Status: Accepted
- Date: 2026-08-28
- Owners: project owner
- Related: PD-010, ADR-0007, ADR-0011, E02-S01, E02-S02, C-12, C-15, C-17

## Context

E02 ("Rust Workspace Bootstrap") is the first epic of phase P1 and the first story to write
any Rust code in this repository. `docs/architecture/TARGET.md` already fixes the crate list,
naming (`cancellai-*`), and dependency direction - those are not re-litigated here. What
`TARGET.md` does not fix, and what has to be decided once, in one place, before the first
`Cargo.toml` exists, are the toolchain and repository parameters every later crate inherits:
Rust edition, MSRV policy, `unsafe` policy, where the workspace physically lives in the
repository, the initial CI platform matrix, and the dependency license policy. Getting any of
these wrong is expensive to unwind once E03-E06 have crates depending on the choice.

These were worked out with the owner as a structured brainstorm (one question, three-plus
options, before E02-S01 implementation began) rather than picked unilaterally, because they
propagate to every subsequent Rust epic and are exactly the kind of decision this document
exists to record durably (C-17, C-18).

## Decision

### Rust edition: 2024

The workspace targets Rust edition 2024. There is no legacy Rust code in this repository to
migrate, so there is no reason to start on an older edition and pay a migration cost later.

### MSRV policy: pinned, bumped only by deliberate decision

The workspace declares an explicit `rust-version` in the workspace `Cargo.toml`. It is never
raised implicitly by a dependency bump; raising it is its own reviewed change with a
CHANGELOG entry, mirroring how `docs/development/MIGRATION_PYTHON_RUST.md` and this project's
evidence-gated culture already treat every other durable constraint.

**MSRV = 1.85.0** - the minimum Rust version edition 2024 itself requires. This is the widest
compatible floor available under the edition already chosen, which matters for a tool
distributed through Homebrew and (eventually) other system package managers whose bundled
toolchain lags the latest stable release.

### `unsafe` policy: forbidden workspace-wide, exceptions live in a dedicated crate

Every crate in the workspace sets `#![forbid(unsafe_code)]` by default (enforced via the
workspace-level lint table, not copy-pasted per crate). If a future story needs `unsafe` (for
example OS-specific identity/reparse-point handling in `cancellai-platform`, E07), it is
isolated in a small, dedicated crate whose only job is that unsafe boundary, with explicit
justification and reinforced review - never scattered into `cancellai-model`,
`cancellai-safety`, or any crate that has no OS-binding reason to need it. This keeps the
safety kernel's TCB (trusted computing base) for memory safety auditable at a glance: `git
grep unsafe` across the workspace should return nothing outside that one crate, for as long
as no such crate exists yet.

### Repository layout: a dedicated `rust/` top-level directory

The Cargo workspace lives at `rust/Cargo.toml`, with crates under `rust/crates/cancellai-*/`
(the `crates/` name TARGET.md already uses, one level deeper). It does not sit at the
repository root alongside `cancellai.py`/`pyproject.toml`.

Two reasons, both already implied by existing documents rather than invented here:

- `docs/architecture/AS_IS.md` and `AGENTS.md`'s Python reference freeze treat `cancellai.py`
  as a temporary reference, and the Rust workspace as the target. Keeping them in visibly
  separate top-level directories makes that relationship legible in the repository itself,
  not only in prose.
- `docs/RELEASING.md`'s "Repository topology transition" section already anticipates E17-S06
  evaluating a split into a canonical `cancellai` source repository once the Rust core and
  release factory are proven. A self-contained `rust/` directory is what makes that future
  split a directory move instead of a repository archaeology exercise. ADR-0011 (defer
  canonical repository split) still holds - this ADR does not schedule that split, it only
  avoids making it harder later.

### CI platform matrix: macOS, Linux, and Windows from E02-S02

Rust CI (`fmt`, `clippy`, `cargo test`, `cargo deny check`, RustSec audit) runs on all three
platforms starting with the quality-baseline story, even though the bootstrap code is not yet
platform-specific. This is a direct application of C-12 ("a platform is supported only when
... tested at the authority level claimed"): if a future crate quietly introduces a
Unix-only assumption, the failure needs to surface the same epic it lands in, not three
epics later when E07 (Cross-Platform Operating-System Layer) finally turns the CI matrix on
and inherits a backlog of platform bugs baked into the model/safety layer's early design.
Windows-specific reparse-point/identity semantics are still E07's job; this decision is only
about which platforms *build and run the test suite*, not which platforms have real adapters
yet.

### Dependency license policy: strict permissive allow-list

`cargo-deny`'s license check uses an allow-list, not a deny-list: `MIT`, `Apache-2.0`,
`BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Unicode-3.0`, `Zlib`. Everything else - including weak
copyleft (`MPL-2.0`) and strong copyleft (`GPL-*`, `LGPL-*`, `AGPL-*`) - is denied by default.
A dependency whose only available license falls outside this list is not added without a
separate reviewed decision to widen the list, matching C-15 (open local control - the local
safety/governance capability stays open and unencumbered) and the project's stated preference
for the widest possible redistribution (Homebrew today, other system package managers later).

## Alternatives considered

### Edition 2021

Wider ecosystem familiarity and more examples assume it, but this is a brand-new workspace
with zero migration cost either way, and edition 2024's capture/ergonomics improvements are a
net win for code being written from scratch. Rejected: no offsetting benefit for a fresh
start.

### Rolling MSRV (latest stable, or latest stable minus a few releases)

Lower friction for adopting new language features; no explicit bump ceremony. Rejected: this
project already runs everything else (safety invariants, ADR/RFC gates, release evidence) on
explicit, reviewed decisions rather than automatic drift, and cargo-deny/RustSec benefit from
a known, stable floor rather than a target that moves under CI without a corresponding commit.

### `deny(unsafe_code)` with local opt-out per crate, or no workspace policy at all

Both leave the boundary of where `unsafe` is allowed to live soft, and soft boundaries erode
under time pressure exactly like protected-name barriers did in the Python reference (E00-S01
found protection could quietly stop being enforced). A `forbid` default with one clearly-named
exception crate keeps `unsafe` a decision that has to be made on purpose, at the crate level,
not something that appears wherever it was locally convenient.

### Cargo workspace at the repository root, or a top-level `crates/` with no `rust/` wrapper

Both were considered more "idiomatic" for a reader who expects a standalone Rust project.
Rejected in favor of `rust/`: this is not a standalone Rust project yet, it coexists with a
frozen Python reference and eventually a documented repository split, and the directory
boundary should say so.

### macOS-only Rust CI initially (mirroring the current Python CI matrix)

Cheaper to run and matches precedent. Rejected: the Python reference's CI matrix reflects
what Python v1 actually ships on (macOS/Homebrew only, C-12's platform-support truthfulness
already applied there); the Rust target's entire premise is being genuinely cross-platform, so
its CI should reflect that premise from the first crate rather than only once E07 forces the
question.

### Deny-list license policy

Less maintenance (only ban known-problematic licenses), more permissive default. Rejected:
weaker guarantee about the whole dependency tree, and this project already prefers an
explicit allow-list style everywhere else it governs external trust (provider manifests,
knowledge bundles - SI-021, SI-022).

## Consequences

### Positive

- Every later Rust epic (E03-E06) inherits a fixed toolchain floor and lint posture instead
  of re-deciding it, or worse, discovering divergence between crates written months apart.
- The `unsafe` and license policies give `cargo-deny`/CI concrete, mechanically enforceable
  rules from the first commit, consistent with this project's preference for governance that
  is enforced by a machine rather than remembered.
- Cross-platform CI from day one means C-12 is honored before any platform-specific code
  exists to violate it, rather than retrofitted.

### Negative / cost

- A stricter `unsafe`/license policy can force rejecting an otherwise-suitable crate later;
  the cost is accepted as a supply-chain and auditability tradeoff, not an oversight.
- Running the full CI matrix on three platforms from E02-S02, before any platform-specific
  code exists, spends CI minutes on redundant coverage for a while. Judged worth it given how
  C-12 was already violated once by Python v1 shipping macOS-only assumptions that were only
  discovered late (`docs/architecture/AS_IS.md`'s verified P0 defects).

### Neutral / follow-up

- MSRV 1.85 is a floor, not a ceiling; nothing here prevents bumping it later through the
  explicit process this ADR itself establishes.
- The `rust/` directory boundary is purely organizational; it does not by itself decide when
  (or whether) E17-S06's repository split happens.

## Safety and compatibility impact

- Change Risk implication: this ADR is process/tooling scope, not a mutation-authority
  change; the stories it governs (E02-S01, E02-S02) are CR1.
- Safety Invariants affected: none directly invoked (no mutation path exists yet in E02); the
  `unsafe` policy is a forward-looking control for the eventual safety kernel's (E03) trusted
  computing base.
- Migration/rollback: none of these are runtime-reversible decisions in the traditional sense
  - they are build-time/repository conventions. Reversing any of them (a different edition,
  relaxing the license allow-list, moving the workspace out of `rust/`) is itself a new
  reviewed decision, not a rollback of running state.

## Supersession

If replaced later, keep this ADR and mark it superseded by the ADR that replaces it.
