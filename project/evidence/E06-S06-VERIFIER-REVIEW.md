Review-Scope: story
Round: 1
Verifier: Codex
Brief-Checksum: 817de404f11c12559e5d15fc1f1c11e17aa4731db38847bbb757505ef9c96b65

# E06-S06 Independent Verifier Review - Round 1

- Review target: current working tree, with executor's uncommitted control-plane changes left intact
- Date: 2026-09-23
- Story contract: `project/evidence/E06-S06/VERIFIER_BRIEF.md`; E06-S06 [CR4]
- Reviewed implementation: current E21-S03 scan-completeness adapters and E21-S07 handle-relative unlink path

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S06 | FAIL | Directory-based CR-TE-01 cases reproduce as repaired for Claude and Codex in default and custom root scenarios. A mode-000 Codex rollout file is the missed read-error case: Rust reports complete with zero errors and `plan` proposes a delete; Python exits 4 and withholds. Claude also accumulates companion traversal reasons in an unbounded intermediate vector before applying the bounded scope log. |

## Findings and required repairs

### F-01 - Codex lineage read error bypasses scan completeness (SI-008/SI-009/SI-010)

**Reproduction:** Synthetic stale Codex rollout with readable metadata and mode 000 content, under both `$HOME/.codex` and custom `CODEX_HOME`. Python `clean --yes --days 1 --keep-latest 0 --tool codex` exits 4 and preserves the file. Rust `inspect --json` reports `codex-cli complete=true,error_count=0`; Rust `plan --json` emits a delete action. A real clean may fail later while opening the target for identity verification, but the plan is still destructive and the scope is falsely complete.

**Required repair:** Propagate lineage file open/read errors into the Codex scope's `ReasonLog` with path and cause. Keep successfully read unknown/no-parent content distinct from I/O failure. Add an integration regression that asserts incomplete scan, no destructive action, and reference parity in both root-origin scenarios, including a deterministic permission change after metadata observation.

### F-02 - Claude companion error retention is unbounded before the bounded log (SI-010; C-11 availability)

**Reproduction:** `walk_companion_payload` pushes every failing path into `Vec<CompletenessReason>` and only transfers the vector to the 64-entry `ReasonLog` after traversal finishes. A synthetic companion containing more than 64 unreadable nested directories enters this path. Final retained output is capped, but memory consumed during traversal scales with every failure.

**Required repair:** Record failures directly into a bounded/counting accumulator; do not return or build an unbounded reason vector. Test more than 64 distinct failures at the adapter boundary and assert the cap, exact total, partial scope and action withholding.

## Additional counterexamples checked

- The native CR-TE-01 cases for Claude unreadable project, companion and nested directories pass in both root origins. Codex unreadable first-level, nested payload-shaped and nested directories pass in both root origins; Rust and Python clean agree and preserve the eligible rollout.
- Root absence stays distinct from unreadable roots in the reviewed adapters; resolver completeness is carried into planning for directory errors.
- Symlink traversal uses no-follow checks; sealedfs holds a directory descriptor and uses handle-relative lookup/unlink. No alternate production deletion path was found (`check_mutation_boundary.py` passes).
- The leaf-entry `fstatat`/`unlinkat` race remains as explicitly documented: an entry replaced after identity lookup can be unlinked before post-check detection. This is not newly introduced by this review, but owner disposition is still needed for cutover acceptance.
- Permission revocation exactly between entry metadata and lineage reading was not scheduled deterministically; the mode-000 file reproduces the same open failure. Native Linux and Windows scenarios were not run on this macOS host.

## Gates run

| Gate | Result |
| --- | --- |
| `python3 scripts/diff_harness.py check` | PASS |
| `python3 scripts/rust_python_parity.py check` | PASS: 13 NORMATIVE fixtures, both origins |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS; existing warnings for unmatched license allowances and duplicate dependencies |
| `python3 scripts/check_mutation_boundary.py check` | PASS |
| Synthetic directory incompleteness matrix | PASS: Rust and Python agree, both providers/root origins where layout applies |
| Synthetic unreadable Codex rollout bytes | FAIL: Rust plan contains delete action; reference withholds |
| `python3 scripts/verifier_handoff.py check` | PASS |
| `python3 scripts/project_os.py check` | PASS |
| `python3 scripts/check_docs.py check` | PASS: 518 Markdown files |
| Native Linux/Windows checks | NOT RUN: unavailable on this macOS host |

## Verdict

FAIL
