# E14-S04 Independent Verifier Review — Round 5, Pass 2

Review-Scope: epic
Round: 5
Review-Target: `a5452dc9cac166100987b0a98fbb70fd56da9108`
Verifier: Codex
Date: 2026-09-21
Brief-Checksum: 1f1c8220ff4c6384fdc01b75c86d400421aea406743e5bad126aac9fb669b27c

This is the second and final independently authorized pass over the round-5 repair. It does not
replace `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5.md`; that prior `FAIL` remains the
record that prompted this repair.

## Per-story verdict

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E14-S04 | FAIL | The public `IdentityObserver` injection seam is closed, but `BoundLayoutObservation::observe` still reads `root` twice through separate path-based operations: `SystemIdentityObserver.observe(root)` (`symlink_metadata(root)`) and then `std::fs::read_dir(root)`. A same-user swap in that interval can pair drifted root D's real identity with recognized root R's real markers, returning an `Autopilot` permit tagged with D. Thus the claimed root/marker binding remains forgeable through TOCTOU, rather than through a caller-supplied observer. |

## Independent evidence

### Prior finding reproduction against current HEAD

An external temporary Cargo package, using public path dependencies only, attempted the old
construction exactly:

```rust
BoundLayoutObservation::observe(Path::new("/tmp"), &SyntheticIdentityObserver::new())
```

`cargo check --offline --bin wrong_args` failed with `E0061`: `observe` takes one argument and
the synthetic observer is the unexpected second argument. The added doctest also ran as part of
the workspace suite and passed.

The same external package attempted a struct literal built from a real observation's public
accessor values. `cargo check --offline --bin forged_observation` failed with `E0451`: both
`root_identity` and `markers` are private. Inspection of the complete public surface found no
second constructor, builder, `From`/`Into`, or mutable field/setter for
`BoundLayoutObservation`; its only public constructor is `observe`. `Clone` can only duplicate
the already-bound private fields and `PartialEq` only compares them, so neither can attach a new
identity to different markers after construction.

The round-4 separation also remains intact: `AuthorityInputs` has no layout field,
`effective_authority` returns `EffectiveAuthority`, and the only permit-minting path is
`resolve_provider_execution_authority` with a `&BoundLayoutObservation`. `ProviderExecutionPermit`
still has private fields, no public constructor, and no conversion from `EffectiveAuthority`.

### New bypass: the two allegedly co-observed facts are not co-observed atomically

On Unix, `SystemIdentityObserver.observe(root)` is `std::fs::symlink_metadata(root)` in
`identity.rs`; `BoundLayoutObservation::observe` subsequently invokes `std::fs::read_dir(root)`
in `provider_layout.rs`. These are separate path resolutions. The following valid interleaving
recreates the exact security property the repair claims to establish:

1. Provider root path `P` names real, drifted directory D. `symlink_metadata(P)` returns D's
   identity token.
2. A same-user concurrent actor atomically renames D away and a real, recognized directory R
   (with `sessions/`) into `P`.
3. `read_dir(P)` opens R and supplies its recognized marker set, while the observation retains
   D's identity from step 1.
4. With the caller-supplied `known_signatures` residual set to `sessions/` and otherwise
   permissive, legitimately promoted authority inputs, the public resolver emits an
   `Autopilot` permit labelled with D's identity.
5. The actor restores D at `P` before a future E14-S05 consumer compares the permit token with
   a fresh identity. That comparison accepts D even though the layout which earned the permit
   was R's.

The external stress probe performed two million concurrent directory-swap attempts; it did not
hit this scheduler-sensitive interval on this host. That does not invalidate the counterexample:
the source has no retained descriptor, lock, retry-and-compare, or other mechanism which makes
the documented interleaving impossible. The platform model explicitly records this exact rule:
a re-check followed by a separate path operation cannot close a swap window; a retained,
no-follow handle is required. The implementation's statements that identity and markers are
read "from the same call" are therefore false for the code currently committed.

There is a related direct topology gap: `symlink_metadata(root)` records a final symlink as
`FileKind::Symlink`, while `read_dir(root)` follows that symlink and enumerates its target. The
constructor neither rejects the non-directory identity nor uses a no-follow retained directory
handle, so even absent an active race the token and markers need not describe the same directory
object.

## Invariant table

| Claim | Result | Evidence |
| --- | --- | --- |
| Round-4 `EffectiveAuthority`-to-permit bypass is closed | PASS | No layout field remains on public `AuthorityInputs`; plain analysis produces `EffectiveAuthority`, not a permit; permit fields remain private and there is no conversion path. |
| `known_signatures` residual is accurately disclosed | PASS_WITH_RESIDUALS | It remains caller-supplied. A caller can assert a signature for unrelated real markers, but repository search finds no production resolver call with a non-empty signature source. |
| No mutation-boundary permit consumer exists | PASS_WITH_RESIDUALS | Repository search finds the resolver and permit in the authority module and test helpers only; neither `mutation_executor` nor the one permitted mutation module consumes a permit. |
| Hardcoded `SystemIdentityObserver` platform/failure behavior | PASS | Unix, Windows, and unsupported-platform paths return an identity observation; absent, unreadable, and unsupported cases become `LayoutObservationError`. `read_dir` and entry/file-type errors are also returned, not treated as clean markers. No new ordinary error/panic path was found. |
| Root/marker identity binding promised by ADR-0036 | FAIL | Separate `symlink_metadata` and `read_dir` calls admit the swap and symlink-following counterexamples above. |

## Adversarial cases tried

- Old public synthetic-observer argument: rejected at compile time (`E0061`).
- External struct-literal observation forge: rejected at compile time (`E0451`).
- Search for a secondary constructor/builder/conversion or public mutation route: none found.
- Clone/`PartialEq` derive audit: no means to replace either private field.
- Absent, unreadable, and unsupported identity paths: fail closed to `LayoutObservationError`.
- Unix/Windows code-path audit: hardcoded `SystemIdentityObserver` has no ordinary panic path.
- Two-million-iteration concurrent directory-swap stress probe: no hit on this host; source-level
  adversarial interleaving remains possible and unguarded.
- Final-symlink topology: identity observes the link while `read_dir` follows it; no refusal is
  present.

## Required repair and owner decision

This is not a third self-authorized patch cycle. A further attempt requires a fresh owner
decision, as the authorized two-pass repair budget is now exhausted.

The next design must bind both facts to one retained, no-follow directory object: on Unix, open
the provider root through a sealed handle/no-follow component walk, obtain identity from that
handle, and enumerate its direct entries relative to that same descriptor; on Windows, provide
an equivalently handle-bound implementation or fail closed as unsupported. It must refuse a
symlink/reparse root rather than observe its link identity and enumerate its target. A
post-hoc re-read/identity comparison narrows but cannot eliminate the gap, so it is not an
adequate repair. The owner must decide the new scope/ADR and authorize any subsequent review.

## Gate status

| Command | Result |
| --- | --- |
| External public-API probe: `cargo check --offline --bin wrong_args` | PASS — expected `E0061`, no observer override remains. |
| External public-API probe: `cargo check --offline --bin forged_observation` | PASS — expected `E0451`, private observation fields cannot be constructed externally. |
| External concurrent directory-swap probe | INCONCLUSIVE — 2,000,000 attempts did not hit the scheduler-sensitive window; source inspection proves no primitive prevents the valid interleaving. Temporary package removed. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo test --workspace` | PASS — includes `BoundLayoutObservation::observe` `compile_fail` doctest. |
| `cd rust && cargo deny check` | PASS — existing unmatched-license and duplicate-crate warnings only; advisories, bans, licenses, and sources passed. |
| `python3 scripts/check_rust_workspace.py check` | PASS |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `gh run list --branch main --limit 5` | FAILING BASELINE — scheduled `rust-benchmark` run 35600616995 failed because Miri attempted `rusqlite::sqlite3_threadsafe`, an unsupported foreign call. The latest push workflows shown were successful; CI is not treated as green. |

## Documents opened

- `AGENTS.md`
- `docs/INDEX.md`
- `docs/CONSTITUTION.md`
- `docs/BACKLOG.md`
- `project/epics/E14.json`
- `docs/development/ENGINEERING_SYSTEM.md`
- `docs/development/AGENT_PROTOCOL.md`
- `docs/adrs/0019-dependency-rings-per-crate.md`
- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
- `docs/architecture/DOMAIN_MODEL.md`
- `docs/architecture/GUARDIAN_MODEL.md`
- `docs/architecture/PLATFORM_MODEL.md`
- `docs/security/SAFETY_INVARIANTS.md`
- `docs/security/THREAT_MODEL.md`
- `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND5.md`
- `project/evidence/E14-S04/EVIDENCE-ROUND5-IDENTITY-BINDING-REPAIR.md`
- `rust/crates/cancellai-platform/src/provider_layout.rs`
- `rust/crates/cancellai-platform/src/identity.rs`
- `rust/crates/cancellai-safety/src/authority.rs`

## CR4 Safety Verdict — E14-S04

## Verdict

`FAIL`

The targeted public observer-injection defect is closed, but the repaired primitive still
claims a root-bound fact from two unbound path lookups. A concurrent swap, or a final symlink
that `read_dir` follows, can make the observation carry one object's identity and another
object's markers. This violates SI-004's requirement that provider-layout drift cannot preserve
destructive capability, and conflicts with C-02, C-05, and TM-05. The two disclosed ADR-0036
residuals remain accurate but do not excuse this primitive-level binding failure.

## Owner decision

E14-S04 cannot close on this verdict. The second authorized pass found a new, distinct binding
failure; no further patch/review cycle is self-authorized. An owner decision is required before
any repair, followed by fresh independent review.
