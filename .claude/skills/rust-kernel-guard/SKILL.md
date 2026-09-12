---
name: rust-kernel-guard
description: Enforce cancellAI's Rust architecture rules on a change under rust/ - dependency rings per crate (ADR-0019), forbid(unsafe_code) and its single ADR-0017 exception, one mutation boundary, and no second path to a safety decision. Use before adding any crate dependency, when touching cancellai-safety / sealedfs / platform / model, when asked "posso usare questa crate", or when reviewing Rust in this workspace.
allowed-tools: Bash, Read, Glob, Grep
---

# Rust kernel guard

The workspace splits into two rings. The ring, not the convenience, decides what a crate may depend
on. `docs/adrs/0019-dependency-rings-per-crate.md` is the contract.

## Workspace state

!`/bin/ls rust/crates 2>&1`

!`grep -rn "^unsafe_code" rust/Cargo.toml rust/crates/*/Cargo.toml 2>&1 | head -20`

## The rings

**Kernel ring** - `cancellai-model`, `cancellai-safety`, `cancellai-platform`,
`cancellai-sealedfs`, and any future crate participating in authority, identity, or mutation.

> A dependency here requires a dedicated, reviewed ADR naming the specific capability `std`
> cannot express. ADR-0017 is the template. **"It would be less work" is not a reason in this
> ring** - and that is the single most common way this rule gets broken, because the argument is
> always locally true.

**Outer ring** - `cancellai-cli`, `cancellai-tui`, `cancellai-store`, `cancellai-guardian`, the
provider adapters. A mature, widely-audited crate is admissible when it does not reach into
authority/identity/mutation decisions **and** the adopting story says what it replaces. Reduced
implementation effort *is* legitimate here - the thing being implemented is not a safety boundary.

## Checks to run on any `rust/` change

```sh
python3 scripts/check_rust_workspace.py check
python3 scripts/check_mutation_boundary.py check
```

From `rust/`: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo check --workspace --all-targets`, `cargo test --workspace`, `cargo deny check`.

`cargo deny check` covers RustSec advisories, the license allow-list, wildcard/duplicate bans and
unknown-source denial in one command. A separate `cargo audit` is redundant and is not used.

## Review questions, in order

1. **Which ring is this crate in?** Answer before anything else; it changes every other answer.
2. **Does this dependency reach an authority, identity or mutation decision?** If yes, it is
   kernel-ring regardless of which crate it was added to. An outer-ring dependency may never
   become a second path to a safety decision - it decides what the user *asked for*, never what is
   *permitted*. SI-007 in particular stays a property of this workspace's own command dispatch,
   whichever crate parses the tokens.
3. **Is `unsafe_code = "forbid"` still the workspace default?** `cancellai-sealedfs` is the sole
   ADR-0017 exception. A second exception needs its own ADR, never a local `allow`.
4. **Is the license in the `cargo-deny` allow-list?** MIT, Apache-2.0, BSD-2/3-Clause, ISC,
   Unicode-3.0, Zlib. Anything else needs a reviewed change to `rust/deny.toml` first, in either ring.
5. **Does UI or Guardian code contain independent mutation logic?** It must not. Constitutionally,
   mutation goes through one safety boundary.
6. **Does cached or local DB state become destructive truth anywhere?** It must not.
7. **Does network or provider knowledge directly authorize a local deletion?** It must not.
8. **MSRV.** CI pins 1.85.0 alongside stable on macOS, Linux and Windows. A dependency or language
   feature that lifts MSRV is a visible decision, not a side effect.

## Verdict

```
Ring:        kernel | outer   (<crate>)
Dependency:  <name> <version> - admissible | REQUIRES ADR | refused (<reason>)
Unsafe:      forbid intact | exception (<ADR>)
Second path: none | FOUND (<where a non-kernel crate decides what is permitted>)
Checks:      <command> -> result
```

A `REQUIRES ADR` verdict stops the change. Draft the ADR from `project/templates/ADR.md` naming the
specific capability `std` cannot express - do not implement first and document after.
