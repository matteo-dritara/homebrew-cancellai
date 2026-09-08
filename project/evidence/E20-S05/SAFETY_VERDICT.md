# Safety Verdict - E20-S05

- Change: Windows process observation, allocated-size observation, handle-relative sealed-root operations, atomic rename, and identity-confirmed deletion.
- Risk: CR4
- Review target: `2dffe54c7752be80076292ea09096b5bb8b1ef3e..aaad5b3eca42eeee1aa816dc098e7b1fa4b43ff9`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-08

## Verdict

`PASS`

## Safety surface changed

Windows can now reach the existing safety executor's real deletion capability. The deletion path binds the parent through a retained handle-relative no-follow walk, opens the direct child without following a reparse point, compares its Windows volume/file-index identity, and marks that handle for deletion. Configuration writes use the same sealed root and a handle-relative atomic rename.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-013 | The target and root are revalidated at execution and a name swap cannot redirect deletion. | `windows_confirmed_delete_rejects_a_target_already_swapped_before_open`, `windows_confirmed_delete_detects_a_target_swapped_between_open_and_unlink`, and `unlink_child_matching_windows_identity_deletes_only_on_a_real_identity_match` passed in native Windows CI. Static review confirmed the final removal is `NtCreateFile` relative to the held parent, followed by identity comparison. | PASS |
| SI-017 | Windows identity and reparse semantics are native and fail closed. | The code uses `GetFileInformationByHandle` volume serial/file index and checks `FILE_ATTRIBUTE_REPARSE_POINT`; Windows CI passed the real directory-symlink and NTFS-junction refusal fixtures. | PASS |
| SI-018 | Mutation does not silently cross volumes, junctions, or reparse points. | The sealed walk opens one component at a time relative to the retained parent and rejects reparse points. The existing Windows volume-token boundary tests and the real junction fixtures passed on Windows CI. | PASS |
| SI-019 | Only the safety boundary reaches real deletion. | `python3 scripts/check_mutation_boundary.py check` passed: the sole raw deletion seam is `cancellai-platform::mutation`, and only it plus `cancellai-safety::mutation_executor` reference the capability. | PASS |

## Adversarial cases

- Native Windows CI runs 33904582262 and 33905293698 both passed the Windows quality job, including the symlink/reparse, junction, pre-planted temporary-name reparse, stale-identity, and between-open-and-unlink swap fixtures.
- The verifier traced `NtCreateFile` calls to a single validated component with `OBJECT_ATTRIBUTES.RootDirectory` set to the retained parent handle; a path replacement after the walk cannot redirect the subsequent child operation.
- A mismatched file index, a final reparse point, a missing target, a non-UTF-8 child name, and an unsupported identity token all refuse rather than delete.

## Differential / compatibility evidence

- Local Rust gates passed: `cargo fmt --check`, clippy with `-D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo deny check`.
- Local Python/reference gates passed, including 192 pytest tests, Ruff, mypy, documentation, fixture, schema, characterization, differential-parity, workspace, mutation-boundary, platform, process, and release checks.
- GitHub Actions run 33904582262 is a successful `rust.yml` run at the exact code commit `995fa94583b5ffa663d196d521408b8ad5e0c4c5`; run 33905293698 successfully repeated the Windows quality job at the final review head, whose intervening commit changes only documentation and evidence.

## Known residual risks

- WSL2 remains Tier 2 with mutation refused because it has no dedicated native CI verification; this is recorded as unsupported in `project/platforms.json` and is outside E20-S05's native-Windows authority claim.
- UNC/device-path root establishment is refused, rather than being treated as an unverified variant of a local drive path.

## Rollback / recovery

Revert E20-S05 as one change set and restore Windows mutation to its prior fail-closed unsupported posture. No migration or stored-state recovery is required.

## Owner decision

`ACCEPT`

Owner note: independent CR4 review passed; the task authorizes moving E20-S05 and E20 to `done` and cutting the required release.
