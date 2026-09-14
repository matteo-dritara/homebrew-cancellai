# Evidence Packet - E27-S06

- Commit/PR: on `rework/codebase-health`, [PR #19](https://github.com/matteo-dritara/homebrew-cancellai/pull/19)
- Executor: Claude
- Independent verifier: **required and not performed.** CR4 in `cancellai-sealedfs`.
- Change Risk: CR4
- Spec version/commit: `project/epics/E27.json` at this commit
- Owner authorisation: the nightly toolchain was installed on the owner's explicit instruction. `AGENTS.md` and `docs/development/AGENT_TOOLCHAIN.md` forbid an agent installing a component on its own; this was not that.

## Outcome

IMPLEMENTED - awaiting independent verification

## How this story started: the verifier was right and the executor nearly undid it

Codex's E27-S01 review repaired two `SAFETY` comments, reporting that `FILE_STANDARD_INFO` has
`bool` fields and therefore a validity invariant the executor's comment denied.

The executor checked that claim against `windows-sys 0.59`, where `DeletePending` and `Directory`
are `BOOLEAN`, a `u8` alias with no validity invariant - and was one edit away from reverting a
correct repair. **The workspace compiles `windows-sys 0.61.2`, where both fields are Rust `bool`.**
The verifier was right; the executor read the wrong version of the dependency.

That near-miss is the finding. The soundness of `mem::zeroed()` there was never a property of the
code: it was a property of a dependency's field types, which changed between two minor versions and
would change again under a routine `cargo update` with nothing to notice. Clippy checks that a
`SAFETY` comment exists, never that it is true.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - delete rather than document | `windows-sys 0.61.2` derives `Default` on `FILE_STANDARD_INFO` and `BY_HANDLE_FILE_INFORMATION`. Three `unsafe { mem::zeroed() }` blocks become `::default()`. **41 unsafe blocks to 38**, counted excluding comment lines. | PASS |
| AC2 - a version-dependent argument says so | The replacement comment names the 0.59-to-0.61 change explicitly, so the next reader knows the hazard class rather than inheriting a conclusion. `PROCESSENTRY32W` and `IO_STATUS_BLOCK` derive no `Default` and keep `mem::zeroed()`. | PASS |
| AC3 - Miri run and recorded | Run, with results below. It is no longer a residual risk in four packets; it is a measurement with a stated reach. | PASS |
| AC4 - each refusal names its cause | Four crates refuse, each on a specific foreign function or intrinsic - listed below, not summarised as "Miri does not work here". | PASS |
| AC5 - cadence chosen from cost | `cancellai-tui` alone takes **333 seconds** under the interpreter. Weekly in `rust-benchmark.yml`, beside the benchmark that made the same trade for the same reason. Not per-PR. | PASS |

## What Miri found

**197 tests executed with no undefined behaviour:**

| Crate | Tests under Miri |
| --- | --- |
| `cancellai-model` | 15 |
| `cancellai-inventory` | 41 |
| `cancellai-provider-api` | 80 - including the exhaustive glob differential test from E27-S01 |
| `cancellai-tui` | 63 |

**Four crates Miri cannot execute**, each for a named reason:

| Crate | Refusal |
| --- | --- |
| `cancellai-sealedfs` | `can't call foreign function statfs on OS macos` - and by extension the whole `openat`/`fstatat`/`mkdirat`/`unlinkat`/`renameat` family |
| `cancellai-platform` | `statfs` |
| `cancellai-policy` | `fsetattrlist` |
| `cancellai-safety` | `sha2 0.11.0`'s aarch64 SHA-512 path: under Stacked Borrows a `vld1q_u64` aliasing violation in `sha2/src/sha512/aarch64_sha3.rs:39`, under Tree Borrows `can't call LLVM intrinsic llvm.aarch64.crypto.sha512h` |

**The honest reading: Miri helps least exactly where this repository's risk is highest.** Every
remaining `unsafe` block is in `cancellai-sealedfs`, and that is the one crate Miri refuses. What it
did buy is real but different from what was hoped: 197 tests of authority, classification, parsing
and rendering logic now have a dynamic UB check behind them, including the code that reads
third-party provider manifests.

The `sha2` report is a third-party observation, not a defect here: the aliasing violation is in a
dependency's hand-written SIMD, reached through `ed25519-dalek`. It is recorded rather than acted
on, because nothing in this workspace can fix it and a pin would trade a real advisory surface for
an unproven one.

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | Replacing `mem::zeroed()` changing what the syscall receives | `Default::default()` on a `#[derive(Default)]` C struct produces the same all-zero value; the field is then overwritten by the same call as before. Clippy `-D warnings` clean on native, `x86_64-pc-windows-gnu` and `x86_64-unknown-linux-gnu`; no test changed. | PASS |
| SI-019 | The remaining `mem::zeroed()` sites inheriting the false argument | Rewritten to name the dependency-version hazard rather than repeat "no validity invariant". | Corrected |

## Verification Commands

```text
rustup toolchain install nightly --component miri        (owner-authorised)
MIRIFLAGS=-Zmiri-disable-isolation cargo +nightly miri test -p <crate>
cargo clippy --workspace --all-targets --all-features -- -D warnings          -> 0
  ... --target x86_64-pc-windows-gnu                                          -> 0
  ... --target x86_64-unknown-linux-gnu                                       -> 0
grep -v '^\s*//' crates/cancellai-sealedfs/src/*.rs | grep -c 'unsafe {'      -> 38 (was 41)
```

## Residual risks

- **CR4 with no independent verdict**, again, and this time the executor has already been shown
  wrong once about exactly this file.
- **Miri's reach is the opposite of this project's risk profile.** It cannot execute a single one
  of the 38 remaining `unsafe` blocks. `-Zmiri-native-lib` exists and is experimental and Unix-only;
  it was not attempted.
- **The weekly cadence means a UB regression can live for six days.** That is the same trade the
  benchmark job already makes, and it is a trade rather than a solution.
- **`sha2`'s SIMD path is unverifiable here** and sits under signature verification. It is a
  dependency's code, reported upstream by no one in this repository.
- **Two `mem::zeroed()` sites remain** (`PROCESSENTRY32W`, `IO_STATUS_BLOCK`), still resting on an
  argument about a dependency's field types, still with no gate that would notice a change.

## Safety Verdict

**Not issued.** CR4 requires an independent verifier.

## Verifier verdict

pending
