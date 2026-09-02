# Safety Verdict - E07-S07

- Change: Provider-root link authority boundary
- Risk: CR4
- Commit/PR: `7e3d938..f9db57e`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-02

## Verdict

`FAIL`

## Safety surface changed

The repair adds `cancellai-sealedfs::SealedRoot` for `configure`: the final root component is
opened with `O_NOFOLLOW` and child operations use its retained descriptor. This closes the
round-1 final-component swap, but does not reject a link/reparse point in an intermediate path
component.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | A mutation/configuration write must have a positively bounded provider root. | `SealedRoot::establish` binds only the final path component. `HOME=<link-to-outside>` and its real `.claude` leaf are classified Default and configured successfully. | FAIL |
| SI-003 | No provider mutation may escape its approved root through link indirection. | Native reproduction: `HOME=/private/tmp/.../home-link` where `home-link -> /private/tmp/.../outside`; `configure --claude-retention 30` exits 0 and changes `outside/.claude/settings.json` from 7 to 30. | FAIL |
| SI-013 | Link/reparse drift is rejected immediately before mutation. | The retained descriptor closes a swap of the final root component after `establish`, but `symlink_metadata(path)`, `create_dir_all(path)`, and `open(path, O_NOFOLLOW|O_DIRECTORY)` still resolve all intermediate components lexically. | FAIL |
| SI-019 | Provider mutation is evidence-gated through an authority boundary. | The new boundary is incomplete: an untrusted intermediate link selects the object retained by `SealedRoot`, so the later handle-relative operations faithfully mutate the wrong root. | FAIL |

## Adversarial cases

- The round-1 final-root swap is closed: `SealedRoot` holds an `O_NOFOLLOW` final-root descriptor and child writes are relative `openat`/`renameat` calls.
- The new suite has no intermediate-component-link fixture. Its passing root-leaf, temp-name, and final-root-swap tests therefore provide false confidence for the stated path-resolution boundary.
- Windows directory-symlink tests were not executed on this macOS verifier, and a real NTFS junction remains unproven. The fallback refuses configuration on non-Unix, but that does not repair the Unix intermediate-link escape.

## Differential / compatibility evidence

- `cargo fmt --check`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo check --workspace --all-targets`: PASS.
- `cargo test --workspace`: PASS (native Unix suite; 18 CLI behavior tests and 8 `cancellai-sealedfs` tests).
- `cargo deny check`: PASS; existing unmatched license-allowance warnings for BSD-2-Clause, BSD-3-Clause, and ISC.

## Known residual risks

- The genuine NTFS-junction/reparse fixture and actual Windows execution remain unproven.
- The final component is held safely after binding, but every intermediate component is still resolved through an untrusted lexical path. This is a present authority escape, not an acceptable residual.

## Required repair

Create E07-S09 and repair the root establishment primitive: walk every Unix provider-root path
component handle-relatively from a trusted anchor, opening every directory with `O_NOFOLLOW |
O_DIRECTORY` and creating absent components with `mkdirat` only beneath a retained parent
descriptor. Do not use lexical `create_dir_all`, `symlink_metadata`, or `open` to select a
component after the anchor. Add deterministic configure and clean tests in which an intermediate
component (including the `$HOME` prefix) is a symlink to an outside sentinel. On Windows, retain
explicit refusal until equivalent verified handle/reparse semantics and a true junction fixture
exist.

## Rollback / recovery

No rollback is required: reject this implementation, retain E07-S07 in `in_progress`, and track
the surviving round-2 gap as E07-S09; do not open a third E07-S07 review round.

## Owner decision

`REJECT`

Owner note: Do not accept the Windows-junction residual while the independently reproducible
Unix root-swap escape remains open.
