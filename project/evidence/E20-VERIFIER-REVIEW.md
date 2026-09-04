# E20 Independent Verifier Review - Round 1

- Epic: E20 - Windows and WSL Native Support
- Review target: `54b8f3567b958db767376fefde1eb1f8d9c75963..30ce089e2946d557740772777927f5d499b41622`
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-04

All three stories in this review batch were `ready_for_review` before review began. E20-S04
was already `done` and was not re-reviewed. Expected behavior was reconstructed from the story
contracts, linked architecture/security documents, SI-017/SI-018, TM-02/TM-04, and the final
diff. Executor reasoning was not treated as evidence.

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E20-S01 | FAIL | The target is not present on GitHub: `origin/main` is exactly the range base (`54b8f356...`), and `gh run list --commit 30ce089e...` returned no runs. Consequently none of the new Win32 code or tests has run on real Windows/Linux CI, while ADR-0020, SI-017, `PLATFORM_MODEL.md`, `CLI_RUST.md`, and `PLATFORMS.md` state that real Windows CI verified it. The required fixture set is also incomplete: the only native Windows adversarial link fixture creates an `IO_REPARSE_TAG_SYMLINK`; there is no NTFS junction fixture and no Windows-token volume-boundary fixture. The generic boundary test constructs Unix tokens only. |
| E20-S02 | FAIL | The implementation explicitly maps a WSL1 kernel string to `RuntimeEnvironment::Wsl2`, although WSL1 is not a WSL2 Linux VM and does not justify the same platform assumptions. An independent temporary test also reproduced `/proc/mounts` escape mishandling: mountpoint `/mnt/My\\040Drive` (`drvfs`) plus path `/mnt/My Drive/file` returned root `overlay`, not `drvfs`. Finally, both new observer types have no production caller outside their own tests, so no performance/safety caveat is surfaced by a product path. |
| E20-S03 | FAIL | The generated check does not establish its central claim. An in-memory WSL2 record set to Tier 1 with Ubuntu labels, both capabilities asserted `verified`, and **zero evidence tests** produced `validate(...) == []`; replacing Windows evidence with the production function name `observe_identity` also produced `[]`. The checker validates workflow text and arbitrary function names, not an exact-SHA successful run, `#[test]`, platform association, or destructive behavior. Worse, WSL2 is listed Tier 2/unverified while the real mutation implementation is enabled for every `cfg(unix)` target, and neither WSL observer is consulted on the clean path; the generated statement that all non-Tier-1 platforms are inspect-only/refused is therefore false. |

## FAIL reproductions and required repairs

### E20-S01 - the verified Windows authority claim has neither CI nor the contracted fixtures

GitHub evidence for the immutable target:

```text
git ls-remote origin refs/heads/main
54b8f3567b958db767376fefde1eb1f8d9c75963  refs/heads/main

gh run list --commit 30ce089e2946d557740772777927f5d499b41622 ...
[]
```

The latest real Rust run is `33862492833`, successful at the **base** commit
`54b8f3567b958db767376fefde1eb1f8d9c75963`; it cannot verify code introduced after that
commit. Cross-target Windows `cargo check` and clippy pass locally, but do not execute
`GetFileInformationByHandle`, filesystem semantics, or Windows-only tests.

Source inspection found these Windows-native tests only:

- ordinary file and directory observation;
- directory symlink reparse/no-follow observation;
- same-file hardlink identity;
- missing-path error.

There is no true junction (`IO_REPARSE_TAG_MOUNT_POINT`) fixture. There is also no test that
constructs `IdentityToken::Windows` for `ApprovedRoot::bind` and proves different volume
serial numbers refuse the bind; `bind_rejects_a_candidate_on_a_different_device_via_synthetic_identity`
uses Unix tokens. No Windows mutation/quarantine primitive exists, so the story outcome's
process, allocated-size, atomic-move, and mutation capability remains expressly unimplemented.

Required repair:

1. expose the exact repair commit to the normal PR/push workflow and record successful
   `check` and `quality` jobs on `windows-latest` and `ubuntu-latest` for that SHA;
2. add a real NTFS junction/reparse adversarial fixture and a falsifiable Windows volume-token
   boundary fixture (plus native multi-volume coverage where the CI environment permits it);
3. keep Windows identity authority `Unsupported`, or keep all claims explicitly unverified,
   until those tests pass on Windows; then correct every false “verified on real Windows CI”
   statement from one source of truth;
4. implement the remaining capabilities named by the story outcome, or amend the story/ADR
   through the owner-controlled contract process rather than narrowing it in evidence prose.

This violates E20-S01's Windows-CI verification contract, SI-017 (an unverified Windows
mapping gained real identity authority), the adversarial reparse requirement behind AC1, and
the concrete-evidence obligation for AC2/SI-018. See `E20-S01/SAFETY_VERDICT.md`.

### E20-S02 - WSL variants and escaped mountpoints are misclassified

The checked-in test `wsl1_kernel_osrelease_is_also_classified_as_wsl2` passes while asserting
the defect: any case-insensitive `microsoft` substring becomes `Wsl2`. WSL1 and WSL2 do not
share the same kernel/filesystem execution model, so a type documented as “a Windows host
running a WSL2 Linux guest” cannot truthfully include WSL1.

For mount parsing, I temporarily added this test to the actual module, ran it, observed the
failure, and removed it before recording the review:

```rust
let mounts = "none / overlay rw 0 0\nC:\\\\ /mnt/My\\040Drive drvfs rw 0 0\n";
assert_eq!(
    longest_matching_mount_fstype(mounts, Path::new("/mnt/My Drive/file")),
    Some("drvfs")
);
```

Actual result:

```text
left: Some("overlay")
right: Some("drvfs")
```

`/proc/mounts` escapes whitespace and backslashes in fields. Comparing the encoded mountpoint
directly with an ordinary `Path` silently selects a less-specific filesystem and can erase the
Windows-mounted caveat. `rg` also found `RuntimeEnvironment`, `FilesystemContextObserver`, and
`WindowsMounted` only in `wsl.rs`, its exports/docs, and tests; no shipped command consumes the
facts.

Required repair: represent WSL1 separately or as an explicit unsupported/unknown environment,
and grant `Wsl2` only on WSL2-specific positive evidence; decode the kernel mount-table field
escaping (or parse a more appropriate kernel interface) before prefix matching; add escaped,
overmounted, malformed, WSL1, and WSL2 fixtures; and wire the classification to an observable
status/inventory explanation so the performance/permission/atomicity caveat is actually
surfaced. Run the Linux branch in Ubuntu CI at the exact target and retain the disclosed lack
of WSL2 integration evidence until a real WSL2 smoke run exists.

This violates AC1 (“WSL detection is explicit”) and AC2 (`/mnt/*` crossings are separately
classified and their caveats surfaced), and conflicts with C-12's requirement to treat WSL as
a distinct, authority-tested environment.

### E20-S03 - the platform checker accepts fabricated support and runtime contradicts the matrix

The following independent in-memory mutation of the loaded JSON returned no validation errors:

```text
wsl2.tier = 1
wsl2.ci_check_job = true
wsl2.ci_quality_job = true
wsl2.ci_os_labels = ["ubuntu-latest"]
wsl2.capabilities = {identity: "verified", mutation: "verified"}
wsl2.evidence_tests = []
validate(data) -> []
```

Thus the checker does not require a destructive fixture at all. A second reproduction set
Windows `evidence_tests = ["observe_identity"]`; validation again returned `[]`, even though
that is a production function, not a test. The implementation's own docstring has the false
direction backwards: accepting a non-test name as test evidence is a false positive.

The checked-in Tier-1 macOS/Linux “mutation evidence” name,
`establish_rejects_a_root_swapped_to_a_symlink_after_final_validation_but_before_the_bind`,
tests sealed-root establishment and creates no deletion. Real deletion tests exist elsewhere,
but the matrix does not cite or structurally associate them with a platform/capability. CI
flags mean only that a label occurs in workflow YAML, and remain `yes` even when the review SHA
has never run.

Finally, `RuntimeEnvironment`/`FilesystemContextObserver` have no production caller, while
`confirmed_delete_file_inner` and the sealed-root walk compile for all `cfg(unix)` targets.
A WSL2 guest therefore takes the Linux deletion path. The matrix simultaneously labels WSL2
Tier 2, identity/mutation unverified, and claims a non-Tier-1 `clean` cannot delete. That is not
an enforced platform contract.

Required repair: make the generated model carry capability-specific, platform-specific test
metadata and exact CI run/SHA provenance; require at least one genuine `#[test]` for every
verified destructive capability; reject borrowing an Ubuntu run as WSL2 evidence; add
negative tests for empty evidence, non-test functions, wrong-platform tests, stale/missing CI,
and non-mutating “mutation” fixtures. Until exact-head CI succeeds, render Windows as
unverified. Either add a CR4 safety-boundary refusal for WSL2/Windows-mounted contexts until
native authority-level verification exists, or obtain that verification and truthfully update
the tier; a CR1 documentation tool cannot merely assert inspect-only behavior that runtime
does not enforce.

This violates E20-S03 AC1 (support can be fabricated without required CI/destructive fixtures)
and AC2 (the listed unsupported/unverified WSL2 environment is not inspect-only or explicitly
refused).

## Additional counterexamples checked

- Windows cross-target `cargo check` and clippy passed, including the new `windows-sys` FFI
  signature and cfg arms. This is useful static evidence but not native behavior evidence.
- The Windows safety executor has two independent fail-closed backstops: Windows identity maps
  to no deletion operation, and the non-Unix platform mutation implementation refuses. No new
  Windows deletion path was found in this range.
- The Unix root/candidate device comparison remains present and local synthetic cross-device,
  symlink, TOCTOU, partial-read, and mutation tests pass.
- The generated matrix remains byte-for-byte reproducible from `project/platforms.json`; the
  failure is that its inputs and “evidence” are assertions the checker cannot authenticate.
- No provider/version parsing or persistence format changed in this epic.

## Gate status

| Command / evidence | Result |
| --- | --- |
| Initial `python3 scripts/project_os.py check`, `status`, `next`, `review`, and all three verifier briefs | PASS; exactly E20-S01/S02/S03 were queued and all were `ready_for_review` |
| `python3 -m pytest tests -v` | PASS: 192 tests, 28 subtests |
| `python3 -m ruff check .`; `python3 -m ruff format --check .`; AGENTS.md mypy target list | System Python unavailable (`No module named ruff/mypy`); rerun with `.venv/bin/python`, all PASS (221 formatted files, 14 mypy source files) |
| `gen_docs`, project OS, docs, workflows, fixtures, schemas, characterize, diff harness, Rust workspace, mutation boundary, provider compatibility, platform, parity self-test/check, process, and release Python check commands from AGENTS.md | PASS individually; parity: 13 NORMATIVE fixtures in both root-origin scenarios; process emitted only the documented E00/E07 ceiling warnings |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS; all ordinary workspace tests/doc-tests passed, two scheduled performance tests ignored by design |
| `cargo deny check` | PASS: advisories, bans, licenses, sources; three unmatched-license-allowance warnings |
| Windows GNU cross-target `cargo check`; cross-target clippy `-D warnings` | PASS (compile/static only) |
| Linux GNU cross-target `cargo check` | PASS (compile/static only) |
| Focused WSL, identity, and root-capability tests | PASS, including the incorrect checked-in WSL1-as-WSL2 assertion |
| Temporary escaped-mount adversarial test | FAIL as reproduced above; removed after reproduction |
| Platform-checker negative probes | FAIL: false Tier-1/no-evidence and production-function-as-test claims both returned no errors |
| Exact-head GitHub Windows/Linux CI | FAIL: target SHA has zero runs and is not on the remote; latest Rust run is for the range base |
| Real WSL2 integration smoke | NOT RUN / unavailable; no repository WSL2 runner exists |

The local gate suite was rerun before verdict recording. Generated governance is rerun after
the status/evidence changes below.

## Overall verdict

**FAIL - review round 1 of at most 2.** E20-S01 returns to `in_progress`. E20-S02 and E20-S03
also have their own required repairs, but are `blocked` by failed dependencies. E20 remains
open; no release is cut. One epic review round remains.
