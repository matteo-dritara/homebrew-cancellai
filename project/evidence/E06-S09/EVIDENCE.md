# Evidence Packet - E06-S09

- Commit/PR: the E06-S09 commit on `main`
- Executor: Claude
- Independent verifier: pending - reviewed with the rest of E06's cutover stories
- Change Risk: CR3
- Spec version/commit: `project/epics/E06.json` at this commit

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - a harness kills a running clean at each enumerated point on macOS, Linux and Windows | `tests/kill_harness.rs` enumerates `roots-established`, `before-delete#0..3`, `after-delete#0..3` and `before-report` (10 points over a 4-artifact plan) and kills with `Child::kill` (SIGKILL / TerminateProcess). `rust.yml`'s `kill-harness` job runs it on all three OSes. Local: macOS, 3 passed. | PASS (Linux/Windows: first CI run) |
| AC2 - after a kill, nothing outside the plan changed; each planned artifact intact or removed; no partial state read as valid | `violations()` compares a byte-level snapshot of the whole `$HOME` before and after: any appeared file, any changed file, any unplanned removal, or a removed count different from the point's implication is a violation. Every case reports none. `clean` writes no state file, so there is none to read back; the planted-defect test shows an appearing half file is caught. | PASS |
| AC3 - a rerun completes the remaining plan without error and without acting twice | Each case reruns `clean --yes --json`: exit 0, `succeeded` equals exactly the remainder, `failed` is 0, and the final tree has lost exactly the planned set. | PASS |
| AC4 - an unreached point fails the run | `kill_at` panics if the process exits or the timeout passes before the marker appears; `an_unreached_point_fails_instead_of_counting_as_survived` pins it. | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-019 | A kill mid-plan leaves a partially mutated artifact or touches something unplanned | Deletion is a single handle-relative unlink per artifact (`MutationOperation::DeleteFile`), and every point was killed with no partial artifact and no unplanned change. | PASS |
| Release surface | The pause hook reachable in a shipped binary | `kill_points::reached` is an empty `#[inline(always)]` function without the `kill-points` feature, and the environment variables are read only under it; `release.yml` builds without the feature. | PASS |

## Verification Commands

```text
cargo test -p cancellai-cli --features kill-points --test kill_harness                -> 3 passed
cargo clippy --workspace --all-targets --all-features -- -D warnings                  -> clean
cargo clippy -p cancellai-cli --all-targets --all-features --target x86_64-pc-windows-gnu -- -D warnings -> clean
cargo clippy ... --target x86_64-unknown-linux-gnu                                    -> NOT RUN locally: no cross C compiler for rusqlite's bundled SQLite; CI lints Linux natively
python3 scripts/check_workflows.py check                                              -> OK
python3 scripts/check_mutation_boundary.py check                                      -> OK
```

## Method defects

- **What happened**: the harness's first run failed parsing the rerun's output: `clean --json` prints a plain sentence when nothing is planned, where the reference prints a document. No earlier gate saw it, because every JSON test of `clean` ran against a tree with work to do. **Prevented by**: none exists - the differential gate compares planning semantics, not the output shape of a run that plans nothing, and no CLI test parses `clean --json` on an empty tree. **Disposition**: accepted 2026-09-23 - carried by E06-S11, which now blocks E06-S04; the harness asserts the current sentence on that path and must be updated when E06-S11 lands.

## Residual risks

- **The plan is small** (four artifacts). The points are per-action, so a larger plan adds cases
  but no new kind of point; the mutation primitive has no multi-step operation to interrupt.
- **Companion payload directories are not deleted by the Rust engine at all**, so no kill can split
  a session from its payload. That is a pre-existing divergence, not something this harness proves safe.
- **Windows and Linux are unproven until the first CI run of `kill-harness`.**

## Verifier verdict

pending
