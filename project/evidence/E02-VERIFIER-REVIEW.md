# E02 Independent Verifier Review - Round 1

- Review target: `b57279c..81d57ba`
- Verifier: Codex
- Date: 2026-08-28

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E02-S01 | PASS | The 12 documented crates exist; the workspace graph is acyclic and core model/safety have no provider-specific dependencies. Local `cargo fmt`, clippy, check, and test pass. |
| E02-S02 | FAIL | The `quality` job includes `windows-latest` but invokes a Docker action for cargo-deny; GitHub cannot run Docker container actions on Windows runners, so the claimed all-platform quality job fails before cargo-deny. |
| E02-S03 | PASS | Six typed categories and stable code/exit mappings are covered by six golden JSON diagnostics; human and JSON renderings derive the same category code. |
| E02-S04 | FAIL | `SystemFsObserver` maps failures to obtain/represent `modified` time to `Timestamp::EPOCH`, turning an unknown/partial filesystem fact into a credible timestamp instead of `Unreadable`/unknown. |

## Failures and required repair

### E02-S02 — Docker cargo-deny action scheduled on unsupported runners

`.github/workflows/rust.yml` schedules the `quality` job on
`macos-latest`, `ubuntu-latest`, and `windows-latest`, then invokes
`EmbarkStudios/cargo-deny-action@3c634983...`. The pinned action metadata sets
`runs.using: docker`; GitHub documents that Docker container actions execute only on Linux
runners. Consequently the macOS and Windows quality matrix entries fail before `cargo deny`
runs, contradicting ADR-0015 and AGENTS.md's required all-platform full quality set.

Required repair: run `cargo deny check` using a non-container installation/invocation that
works on macOS, Linux, and Windows, or put the Docker action in a Linux-only job and make the
all-platform quality matrix use a portable equivalent. Preserve the full all-platform gate
promised by ADR-0015, and add a workflow-policy regression test that rejects a Docker action
in a macOS/Windows matrix.

This violates E02-S02's quality-baseline verification contract and the accepted all-platform
quality decision in ADR-0015; it also makes the documentation claim in
`docs/security/SUPPLY_CHAIN.md` false as written.

Sources checked: [pinned action metadata](https://raw.githubusercontent.com/EmbarkStudios/cargo-deny-action/3c6349835b2b7b196a839186cb8b78e02f7b5f25/action.yml) and [GitHub Actions documentation](https://docs.github.com/en/actions/concepts/workflows-and-actions/custom-actions).

### E02-S04 — partial metadata silently becomes a valid epoch timestamp

`rust/crates/cancellai-platform/src/fs_observer.rs` treats only the initial
`symlink_metadata` call as fallible. Its `meta.modified().ok()` and subsequent
`duration_since(UNIX_EPOCH).ok()` discard every error and use
`unwrap_or(Timestamp::EPOCH)`. A filesystem that cannot return modification time (or an
otherwise unrepresentable pre-epoch value) is therefore reported as ordinary metadata with a
1970 timestamp. A retention/planning caller can interpret that as extremely old rather than
unknown, bypassing the intended unknown-is-protected semantics.

Required repair: preserve failure to observe a safety-relevant timestamp as typed unknown or
`Observation::Unreadable` (and add an injectable/adversarial test for that outcome); do not
substitute a retention-significant timestamp. Document how clock/filesystem range errors
degrade non-destructively.

This violates E02-S04 AC2 (the seam must not abstract away security-critical OS semantics)
and conflicts with C-02/C-03 and the platform seam's own stated SI-008/SI-009/SI-010 contract.
No mutation code exists yet, so this is a CR2 implementation defect, not an independent CR4
change-risk reclassification.

## Gates actually run

- PASS: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo deny check` (local macOS/stable toolchain).
- PASS: `python3 -m pytest tests -q` — 179 passed, 22 subtests.
- PASS: pinned Ruff, formatting, and mypy gates via `uv run`.
- PASS: generated-docs, governance, docs, workflow, fixtures, schemas, characterization, differential harness, Rust-workspace, process, release, and diff checks.
- Not locally executable: the MSRV 1.85.0 and Linux/Windows CI matrix (only stable macOS toolchain is installed). Independently, the Docker-action configuration proves the macOS/Windows quality failure above.

## Overall verdict

FAIL
