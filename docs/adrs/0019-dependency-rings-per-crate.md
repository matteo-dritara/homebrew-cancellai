# ADR-0019: Dependency rings - the kernel stays bare, the outer ring uses reviewed libraries

- Status: Accepted
- Date: 2026-09-03
- Owners: project owner
- Related: ADR-0015, ADR-0017, E02-S02, E22-S03, E09, E13, E15, C-15, C-17, SI-007

## Context

`AGENTS.md` states one rule for the whole Rust workspace: *do not add a dependency merely to
reduce implementation effort*. It was written for a safety kernel, and for a safety kernel it
is exactly right. `rust/Cargo.lock` today resolves to four external crates - `serde`,
`serde_json`, `unicode-normalization`, `libc` - each of which entered through a reviewed,
named need, and `libc` only under ADR-0017's unsafe-isolated boundary. That is an unusually
disciplined dependency posture and nothing here relaxes it where it belongs.

Applied uniformly, the same rule has started producing costs that were never the point of it.
`cancellai-cli` parses its own arguments by hand in `main.rs`. The 2026-09-03 review
(`docs/audits/2026-09-03-CODE_REVIEW.md`, CR-TE-07) found the predictable result: the binary
has no `--help`, no `-h` and no `--version` at all - each exits 2 with `unrecognized flag` -
while the frozen Python reference has a full `argparse` surface, `docs/CLI.md` is generated
from that surface, and the Homebrew formula's own smoke test asserts `cancellai --version`.
The hand-rolled parser also accepts flags a command cannot act on, and it will grow with every
command the roadmap adds.

Looking forward, three planned epics are library-shaped, not kernel-shaped: E09 delivers a TUI,
E13 a SQLite current-state store and event ledger, E15 an OS-native user-service runtime. A
uniform rule points each of those at a hand-written implementation of a solved problem, inside
a project whose stated value is a trustworthy safety boundary - not a bespoke terminal renderer.

The distinction that matters is not effort. It is blast radius. A defect in the authority
lattice or the mutation seam deletes a user's data. A defect in a table-rendering crate draws a
wrong border.

## Decision

We will define two dependency rings, and apply a different rule to each.

**Kernel ring** - `cancellai-model`, `cancellai-safety`, `cancellai-platform`,
`cancellai-sealedfs`, and any future crate that participates in authority, identity, or
mutation. The current rule stands unchanged: no dependency except by a dedicated, reviewed ADR
naming the specific capability `std` cannot express. ADR-0017 is the template and remains the
only instance.

**Outer ring** - `cancellai-cli`, `cancellai-tui`, `cancellai-store`, `cancellai-guardian`, and
the provider adapters. A dependency is admissible when it is a mature, widely-audited crate;
its licence is already inside `rust/deny.toml`'s allow-list; it does not reach into authority,
identity, or mutation decisions; and the story adopting it says what it replaces. Reduced
implementation effort is a legitimate reason here, because the thing being implemented is not
a safety boundary.

Named for the epics already planned, subject to the criteria above at adoption time:
`clap` for command-line parsing, `ratatui` with `crossterm` for the TUI, `rusqlite` with a
bundled SQLite for the store, `tracing` for structured diagnostics. All are MIT or Apache-2.0
and therefore already inside the ADR-0015 allow-list; none requires widening it.

Two constraints cross both rings and are not negotiable by ring membership:

- `unsafe_code = "forbid"` stays the workspace default. `cancellai-sealedfs` remains the sole
  exception, under ADR-0017.
- An outer-ring dependency may not become a second path to a safety decision. A CLI parser
  decides what the user asked for; it never decides what is permitted. SI-007 in particular -
  no flag and no missing subcommand may resolve toward `clean` - is a property of the command
  dispatch this workspace owns, and must keep its own tests regardless of who parses the tokens.

## Alternatives considered

### Keep one uniform zero-dependency rule

Simplest to state and impossible to erode by argument. Rejected because the erosion it prevents
is cheaper than the cost it imposes: the CLI already ships without a help surface, and E09,
E13 and E15 would each become a from-scratch implementation of a solved, non-safety problem.

### Adopt `clap` only, and defer the rest

Considered seriously. It closes CR-TE-07 with the smallest possible commitment and leaves the
TUI/store/runtime decisions to the epics that own them. Rejected as the primary form because it
answers the instance and not the question: E09 would reopen exactly this discussion, with the
same arguments, and the intervening period would leave `AGENTS.md` stating a rule the
repository had already decided to make an exception to. The ring boundary is the durable
answer; `clap` is its first application.

### A per-dependency ADR for every addition, with no ring

The current de facto process. Rejected as ceremony without discrimination: it applies the same
review weight to a terminal-rendering crate as to an FFI boundary inside the mutation path,
which both over-taxes the harmless case and flattens the signal on the dangerous one.

## Consequences

### Positive

- The kernel's dependency posture is now a deliberate, defended position rather than an
  accident of a rule written once for a different scope.
- E09, E13 and E15 can be planned against real libraries, which changes their size materially.
- CR-TE-07 gets a fix that also removes the hand-rolled parser rather than growing it.

### Negative / cost

- The outer ring acquires a supply-chain surface it does not have today. `cargo deny check`
  already gates licences, advisories, wildcards and sources on every platform, and E22-S02 adds
  cargo-ecosystem dependency updates and Rust static analysis - both are prerequisites in
  practice, and E22-S03 depends on them for that reason.
- Ring membership is a judgement that must be re-made for any new crate. The criterion -
  does this crate participate in authority, identity, or mutation - is stated here so the
  judgement is against a written test rather than taste.

### Neutral / follow-up

- `AGENTS.md` must state both rules; today it states one, and an agent reading it would apply
  the kernel rule to a TUI.
- E22-S03 implements the ring boundary and the CLI surface together.

## Safety and compatibility impact

- Change Risk implication: CR2. The command surface change touches CLI dispatch, which is
  SI-007-relevant, so its existing ambiguity tests must pass unchanged against the new parser.
- Safety Invariants affected: SI-007 (ambiguous CLI/configuration is non-destructive). No
  authority, identity, or mutation code is in scope.
- Migration/rollback: additive. Existing invocations keep working; the new surface is help and
  version output that today errors.

## Supersession

If replaced later, keep this ADR and mark it superseded by ADR-XXXX.
