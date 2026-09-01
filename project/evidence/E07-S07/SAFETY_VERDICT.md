# Safety Verdict - E07-S07

- Change: Provider-root link authority boundary
- Risk: CR4
- Commit/PR: `11e7b24..0bb7f6c72afd3dce6b98b83b8dea878954c8e9bb`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-01

## Verdict

`FAIL`

## Safety surface changed

The change controls whether a default-named provider root can carry authority for `clean` and
the vendor configuration write performed by `configure`.  It adds literal-root link detection
at classification and immediately before these operations.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | A mutation/configuration write must have a positively bounded provider root. | `cmd_configure` checks `roots::is_symlink` at `main.rs:859`, but immediately passes the unbound raw path to `configure_claude_retention` at `main.rs:866`. A root directory can be atomically replaced with a link after the check. | FAIL |
| SI-003 | No provider mutation may escape its approved root through link indirection. | After that swap, `configure_claude_retention` uses `create_dir_all`, `read_to_string`, `OpenOptions::open`, and `rename` on `claude_home.join(...)` (`main.rs:918-962`); these path operations resolve the replacement symlink and write the link target. | FAIL |
| SI-013 | Link/reparse drift is rejected immediately before mutation. | The root check and the first write-side operation are separate syscalls with no retained directory handle, identity token, or no-follow/handle-relative operation. The following interleaving reaches an outside write: (1) start with real `$HOME/.claude`, so classification is Default; (2) let `is_symlink` at line 859 return false; (3) atomically rename that directory aside and atomically rename `$HOME/.claude -> <outside>` into place; (4) resume line 866. The configuration helper follows the link and creates/renames `<outside>/settings.json`. | FAIL |
| SI-019 | Provider mutation is evidence-gated through an authority boundary. | The configuration path is deliberately outside `ApprovedRoot`/`IdentityObserver`; its added standalone check is not an atomic authority boundary and has the escape above. | FAIL |

## Adversarial cases

- Native Unix root-link regression tests execute and pass: `clean_refuses_to_mutate_when_home_dot_claude_is_itself_a_symlink` and `configure_refuses_when_home_dot_claude_is_itself_a_symlink`.
- The implementation still fails the required drift case for `configure`: a regular default root swapped to a symlink after its final `is_symlink` check is followed by raw path-based write operations. No test synchronizes this interval, so the passing static-link fixtures provide false confidence for this TOCTOU path.
- Windows-only directory-symlink tests are not executable on this macOS verifier. A real NTFS junction is also not fixture-proven. These are open compatibility gaps, but the Unix configuration race already requires rejection.

## Differential / compatibility evidence

- `cargo fmt --check`: PASS.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: PASS.
- `cargo check --workspace --all-targets`: PASS.
- `cargo test --workspace`: PASS (native Unix suite; 18 CLI behavior tests, including static root-link refusal).
- `cargo deny check`: PASS; existing unmatched license-allowance warnings for BSD-2-Clause, BSD-3-Clause, and ISC.

## Known residual risks

- The genuine NTFS-junction/reparse fixture and actual Windows execution remain unproven.
- More importantly, the configuration root is not held by a no-follow, identity-bound capability across the write. This is a present authority escape, not an acceptable residual.

## Required repair

Route `configure` through a root-bound configuration-write capability that holds and verifies the
provider root immediately through mutation. On Unix, open the root with no-follow semantics,
verify its identity, and perform all settings/temp/rename operations relative to that retained
directory handle; on Windows, implement or explicitly fail closed until equivalent reparse-safe
handle semantics are verified. Add a deterministic adversarial test that swaps a real default
root for a symlink after the final validation and proves the outside sentinel is unchanged.

## Rollback / recovery

No rollback is required: reject this implementation and retain E07-S07 in `in_progress` until
the configuration authority path is repaired and independently re-reviewed.

## Owner decision

`REJECT`

Owner note: Do not accept the Windows-junction residual while the independently reproducible
Unix root-swap escape remains open.
