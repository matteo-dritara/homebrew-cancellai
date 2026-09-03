# Evidence Packet - E22-S05

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E22 epic review round 1
- Change Risk: CR3
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md` (`CR-TE-10`)

## Outcome

PASS

## Scope

`cancellai.py` prefers the vendor's own `codex delete --force` for a Codex session's deletion
strategy when the installed CLI supports it, so Codex's own subagent/thread bookkeeping stays
consistent; `cancellai-cli` always deletes at the filesystem level regardless. Detection
already exists and is correct (`cancellai-provider-codex::codex_delete_supported`/
`NativeDeleteSupport`, four distinct outcomes, ported at E05-S04) - the gap is that nothing in
`cancellai-cli` calls it. This story's AC is explicitly either/or: wire it, or refuse to claim
parity and state the divergence.

## Decision: explicit, permanent disclosure - not wired

Investigated wiring first, then chose not to, for a structural reason rather than an effort
one. In the Python reference, `perform_delete` calls `delete_codex_via_cli` **instead of**
`safe_remove` when `action.strategy == "codex-cli"` (`cancellai.py:1493-1512`) - the vendor
command *replaces* filesystem deletion as the mutation mechanism, it does not run alongside
it as a bookkeeping step. Reproducing that in the Rust engine while preserving SI-019/C-07
("all filesystem/vendor mutations route through the one safety executor") requires the
*kernel's* mutation boundary (`cancellai-safety::mutation_executor`,
`cancellai-platform::mutation`) to grow a second production mutation primitive - authorizing
and then invoking an external, PATH-resolved binary under the same root/process/authority
checks the raw `unlink` path uses today - not a call `cancellai-cli` (an outer-ring crate,
ADR-0019) can make on the kernel's behalf without bypassing it.

`scripts/check_mutation_boundary.py` today proves exactly one thing deletes anything
(`cancellai-platform/src/mutation.rs`) and exactly two files reference that capability. Adding
a second mutation mechanism is the same class of change ADR-0017 (the `libc`/`unsafe` kernel
exception) and E21-S07 (removing two *unconfirmed* `MutationOperation` variants rather than
leaving them armed for a later epic to inherit) both treat as its own dedicated, reviewed
story - and TM-09 ("native vendor delete semantics change - a provider command starts
deleting broader data or changes cascade behavior") is precisely the review that change would
need. This story's own Change Risk is CR3; a kernel mutation-boundary addition is CR4-shaped
(irreversible mutation / authority boundary, `docs/development/WORK_ITEM_MODEL.md`), which
does not fit inside "the smallest coherent change satisfying this contract"
(`AGENT_PROTOCOL.md`). Disclosure is the AC's own explicitly sanctioned alternative, not a
downgrade of scope.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - either wire it, or refuse to claim parity and state the divergence in `docs/CLI_RUST.md`'s Known gaps | Chose disclosure (see Decision above). `docs/CLI_RUST.md`'s Known gaps entry rewritten: states the divergence is now permanent and disclosed rather than pending, explains why in the same terms as this evidence packet, and states the concrete, unchanged consequence (deletion still succeeds; Codex's own bookkeeping is simply unaware of it). | PASS |
| AC2 - if wired, the four `NativeDeleteSupport` outcomes stay distinct (TM-09) | Not wired, so not directly exercised by this story - but already true and unaffected: `CodexProvider::capability(NativeDeleteCapability)` (E05-S04) already maps all four outcomes to distinct `SupportState`/`KnowledgeConfidence` pairs (`Verified`/`Verified`, `Unsupported`/`Observed`, `Unsupported`/`LowUnknown`, `ErrorPartial`/`LowUnknown`), visible in `docs/PROVIDERS.md`'s generated compatibility matrix (`native_delete_capability` row). No code change was needed or made. | N/A (pre-existing, verified unaffected) |
| AC3 - if wired, a `--codex-backend` selector with the reference's semantics, default never silently weaker | Not wired, so no selector was added - adding one would have advertised a choice this build cannot yet honor safely, which `docs/CLI_RUST.md` now states explicitly as the reason none exists. | N/A (not applicable to the disclosure path) |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-004 (unknown provider layout/version reduces capability) | Not touched by this story - no capability-detection code changed. `codex_delete_supported`'s `BinaryNotFound`/`ProbeFailed` outcomes already fail closed to `Unsupported`/`ErrorPartial` with `LowUnknown` confidence, unaffected. | Pre-existing test suite (`rust/crates/cancellai-provider-codex/src/native_delete.rs::tests`, 6 tests covering all four outcomes including the probe-timeout case) still passes. | PASS (unaffected) |
| SI-019 (one mutation boundary) / C-07 | The one counterexample that matters here - "does this story add a second mutation path" - is refuted by construction: no code changed the kernel, `cancellai-cli`, or any mutation-adjacent path. `check_mutation_boundary.py` reports the identical result (only `mutation.rs` deletes, only it and `mutation_executor.rs` reference the capability) before and after this story. | `python3 scripts/check_mutation_boundary.py check` - unchanged output. | PASS |
| SI-021 (provider manifest trust bounds authority) | Not touched - no trust/manifest code changed. | N/A | PASS (unaffected) |

## Verification Contract

- "A synthetic codex binary exercises each of the four detection outcomes, including probe
  timeout" - already true of the pre-existing `native_delete.rs` test suite (`FakeCli`-backed:
  `ac2_a_fake_cli_advertising_force_is_reported_supported`,
  `ac2_a_fake_cli_not_advertising_force_is_reported_unsupported_not_absent`,
  `ac2_a_fake_cli_that_exits_nonzero_is_unsupported_even_if_it_mentions_force`,
  `no_codex_bin_and_nothing_on_path_is_binary_not_found`,
  `a_fake_cli_that_hangs_is_killed_and_reported_as_a_probe_failure_not_a_hang`,
  `a_fake_cli_with_large_output_does_not_deadlock`); re-run below, unmodified.
- "Deletion through the native path still routes its authority decision through the safety
  boundary; the provider command never becomes a second mutation path (C-07)" - vacuously true
  under the disclosure decision: there is no native deletion path to route anything through.

```text
$ cargo test -p cancellai-provider-codex native_delete::
test result: ok. 6 passed; 0 failed
$ python3 scripts/check_mutation_boundary.py check
mutation boundary OK: 53 Rust source files scanned; only rust/crates/cancellai-platform/src/mutation.rs
  deletes anything, only rust/crates/cancellai-platform/src/mutation.rs,
  rust/crates/cancellai-safety/src/mutation_executor.rs reference the capability that does
$ python3 scripts/check_provider_compatibility.py check
provider compatibility matrix OK: 36 rows across 2 provider(s)
```

Full local gate set (`pytest`, `ruff`, `check_docs`, `check_workflows`, `check_process`,
`release.py check`) re-run and green; no Rust source changed, so the Rust toolchain gates were
re-confirmed via the commands above rather than a full `cargo test --workspace` re-run (nothing
in the workspace's compiled surface changed since E22-S03's commit).

## Compatibility

- No behaviour change to any shipped code path. `clean` still deletes Codex sessions
  correctly, at the filesystem level, exactly as it did before this story - the change is
  purely documentation stating a fact that was previously true but unstated as a *permanent*
  decision (it was previously phrased as pending, "E22-S05 resolves it").

## Performance / operability

- Not applicable.

## Documentation updated

- `docs/CLI_RUST.md` - Known gaps entry for `CR-TE-10` rewritten to state the divergence is
  permanent and disclosed, with the structural reasoning above.
- `docs/PROVIDERS.md` - new note under "Capability vocabulary" distinguishing capability
  *detection* (accurate, already true) from the mutation path actually *using* it (not yet,
  by decision).
- `CHANGELOG.md` - new "Documentation" entry under `[Unreleased]`.

## Residual risks

- This is the same divergence the epic inherited from `docs/audits/2026-09-03-CODE_REVIEW.md`
  (`CR-TE-10`) - now explicitly permanent rather than pending, which is a stronger, not
  weaker, disclosure: a future contributor reading `docs/CLI_RUST.md` will not expect this to
  resolve itself as a side effect of an unrelated story.
- Wiring native Codex delete remains real, wanted future work. It belongs in a dedicated story
  scoped as a kernel mutation-boundary change (CR4-shaped), with its own threat-model review
  against TM-09 and its own independent verification - not filed here as a new backlog item,
  since the epic/story-creation process (`AGENTS.md`: "decide whether it is a defect inside an
  existing story, or add/update the project control plane before implementation") makes that
  the owner's call, not an executor's to schedule unilaterally.

## Verifier verdict

pending
