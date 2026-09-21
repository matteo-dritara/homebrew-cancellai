# Evidence Packet - E14-S04 (round 5, readdir errno repair)

- Commit/PR: repairs the finding in `project/evidence/E14-S04-VERIFIER-REVIEW-ROUND6.md` (`FAIL`)
- Executor: Claude
- Independent verifier: Codex (pending)
- Change Risk: CR4
- Spec version/commit: `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-
  observation-type.md` ("Round 5, third self-correction" section)

## Outcome

PARTIAL (repair complete, awaiting independent review)

## Context

Round 6 review confirmed the TOCTOU/symlink repair was sound (unsafe ownership/lifetime reasoning
audited and passed) but found `list_child_names` treated every `readdir` null result as
end-of-directory, so a real mid-enumeration read failure returned a truncated-but-`Ok` listing.
Because the sole consumer decides "recognized" by exact signature equality, a truncated listing
could coincidentally match a known-good signature. The `DT_UNKNOWN` `fstatat` fallback had the
same fail-open shape.

## Repair

`rust/crates/cancellai-sealedfs/src/lib.rs`: added a platform-conditional `errno_location()`
helper (`__errno_location` on Linux/Android, `__error` on the BSD family including macOS).
`list_child_names` now clears `errno` before each `readdir` call and checks it after a null
result, returning `SealError::Io` for a nonzero value rather than treating it as end-of-stream,
per POSIX's documented `readdir` contract. The `DT_UNKNOWN` fallback's `fstatat` failure now
returns an error instead of defaulting to "not a directory".

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| Read failure surfaces as an error, not a truncated success | `list_child_names_fails_rather_than_silently_succeeding_on_a_broken_descriptor`: closes the bound root's own fd directly (via the test module's access to the private `dir` field) and confirms `list_child_names` returns `Err` | PASS |
| No regression to existing success-path behavior | Full `cancellai-sealedfs` suite (26 tests) and full workspace suite re-run: 0 failures | PASS |

## Verification Commands

```text
cd rust && cargo fmt --check                                                    -> PASS
cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings -> PASS
cd rust && cargo test --workspace                                               -> PASS (0 failed)
cd rust && cargo deny check                                                     -> PASS
python3 scripts/check_rust_workspace.py check                                   -> PASS
python3 scripts/check_mutation_boundary.py check                                -> PASS
python3 scripts/project_os.py check                                             -> PASS
```

## Documentation updated

- `docs/adrs/0036-provider-execution-authority-requires-a-non-forgeable-observation-type.md`
  ("Round 5, third self-correction" section added)
- Module doc on `list_child_names` in `rust/crates/cancellai-sealedfs/src/lib.rs`

## Method defects

- none beyond what the prior two evidence files in this directory already record for this
  same repair thread.

## Residual risks

- Unchanged from prior evidence files: Windows/non-Unix `Unsupported`, `known_signatures` has no
  trusted source, no mutation-boundary consumer yet.

## Verifier verdict

PENDING - awaiting independent review by Codex.
