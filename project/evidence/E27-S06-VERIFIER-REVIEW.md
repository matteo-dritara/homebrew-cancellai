# Independent Verifier Review - E27-S06

- Review target: ce2e872f8f46da95328bec36cba28671dfe371c8, repaired by af97e09, 51c7af2, and b1f0d08
- Verifier: Codex (independent verifier)
- Date: 2026-09-14
- Risk: CR4; SI-019 mutation boundary / sole unsafe crate
- Verdict: PASS_WITH_RESIDUALS

## Acceptance criteria

| AC | Concrete independent evidence and reproduction | Result |
| --- | --- | --- |
| Binding-provided zero value removes unsafe initialization | Inspected the locked windows-sys 0.61.2 definitions and Windows API signatures. FILE_STANDARD_INFO is repr(C), i64/i64/u32/bool/bool: field bytes occupy offsets 0..22, alignment is 8, and tail padding occupies bytes 22..24. GetFileInformationByHandleEx(FileStandardInfo) documents lpFileInformation as [out], with its byte size supplied. Default may leave tail padding unspecified, but an out-parameter does not read it; passing the pointer to a valid Rust object is not itself UB. MaybeUninit::zeroed().assume_init() and mem::zeroed() would introduce unsafe without an API need. BY_HANDLE_FILE_INFORMATION is thirteen u32 words (including three two-u32 FILETIME values), size 52/align 4/no padding; GetFileInformationByHandle likewise documents its struct pointer [out]. | PASS |
| Remaining field-dependent comments say so | ce2e872 did not change the three IO_STATUS_BLOCK or one PROCESSENTRY32W comment as claimed. I repaired all four. windows-sys 0.61.2 defines PROCESSENTRY32W as u32/usize/i32/[u16; 260]; IO_STATUS_BLOCK is an NTSTATUS-or-nullable-raw-pointer union plus usize. Both zero patterns are valid in this version. PROCESSENTRY32W.dwSize is assigned immediately before Process32FirstW; no error path reads the entry without a successful return. | PASS after af97e09 |
| Miri is a measured result | Nightly with Miri is installed. Exact successful execution set: model 19, inventory 42 (one native-only 10-second performance assertion filtered), provider-api 80, provider-claude 26, guardian 0, store 0, TUI 66; total 233. The initial packet's 197 count omitted integration and doc tests. | PASS after b1f0d08 |
| Refusals are specific | sealedfs: statfs; platform: statfs through sealedfs; policy: fsetattrlist; CLI and provider-codex: posix_spawnattr_init; safety: sha2 aarch64 SHA-512 aliasing report under Stacked Borrows. The original packet omitted the two process-spawning crates. | PASS after b1f0d08 |
| Cost and cadence | TUI took 353.73 seconds total under Miri. Weekly scheduling is proportionate; a new inventory check fails if every workspace crate is not explicitly executable or excluded, a rename changes that inventory, and a listed executable crate starts refusing. | PASS after b1f0d08 |

## Safety comment surgery

The ce2e872 sealedfs diff was read in full. The script removed only the stale six SAFETY-comment
lines next to the three replaced zero initializations and left the following call arguments and
logic unchanged. The replacement comments originally overgeneralized the FILE_STANDARD_INFO
version hazard to BY_HANDLE_FILE_INFORMATION and incorrectly described Default as a fully
zeroed object. af97e09 corrects both explanations without restoring unsafe code.

## Miri scope

Miri used -Zmiri-disable-isolation because the tests use synthetic temporary files. That permits
real filesystem I/O and weakens isolation/determinism, but does not turn off invalid-value,
aliasing/provenance, or other UB checking. TUI's deliberate Box::leak fixtures cause leak reports
after all tests pass, so -Zmiri-ignore-leaks is also explicit in the workflow. It disables leak
reporting only.

Miri does reach the first remaining unsafe call in sealedfs: it aborts when it cannot model the
statfs FFI call. Thus the executor's statement that it cannot execute any of 38 remaining unsafe
blocks is false. It supplies no evidence for blocks not reached before that abort.

## Unsafe-inventory counterexamples

The original line-based filter counted unsafe text in block comments, raw strings, and trailing
line comments. 51c7af2 adds a narrow lexer and regression test:

- block comment containing unsafe { is ignored;
- raw string containing unsafe { and SAFETY: is ignored;
- an actual unsafe block with a trailing comment remains counted;
- SAFETY: is counted only in lexical comments, not strings.

The repaired inventory count is 38 actual unsafe blocks, all in cancellai-sealedfs.

## Counterexamples tried with no finding

- A Default-created FILE_STANDARD_INFO with uninitialized tail padding passed to an out-only API.
- A claim that FFI pointer passage is automatically UB because Rust padding is unspecified.
- BY_HANDLE_FILE_INFORMATION padding: computed layout has none.
- Invalid PROCESSENTRY32W.dwSize ordering and an error-path entry read.
- A changed IO_STATUS_BLOCK field validity invariant in the locked binding.
- Nested/block-comment, raw-string, and trailing-code attempts to evade the unsafe inventory.
- A changed sealedfs behavior in the comment-removal diff.

## Gates

| Command | Target/result |
| --- | --- |
| cargo fmt --check | workspace: PASS |
| cargo clippy workspace all targets all features, warnings denied | native macOS: PASS |
| same Clippy command with x86_64-pc-windows-gnu | Windows-GNU cross target: PASS |
| same Clippy command with x86_64-unknown-linux-gnu | Linux-GNU cross target: PASS |
| cargo test --workspace | macOS host: PASS |
| cargo deny check | PASS; existing non-fatal unmatched-allowance and duplicate warnings |
| python3 -m pytest tests -v | macOS host: PASS, 522 passed |
| pre-commit run --all-files | PASS |
| python3 scripts/check_coverage.py check | FAIL twice: cancellai-platform 93.24% versus 95.83% ratchet; no platform source changed since baseline, tracked as E27-S07 |
| Miri executable set above | macOS nightly: PASS with documented flags |

## Documents opened

AGENTS.md; docs/INDEX.md; docs/CONSTITUTION.md; docs/BACKLOG.md; project/epics/E27.json;
docs/development/ENGINEERING_SYSTEM.md; docs/development/AGENT_PROTOCOL.md;
docs/security/SAFETY_INVARIANTS.md; docs/adrs/0017-sealed-root-handle-for-configuration-writes.md;
docs/adrs/0028-lint-policy-states-the-safety-thesis-and-differs-by-ring.md;
project/evidence/E27-S06/EVIDENCE.md; project/templates/SAFETY_VERDICT.md; and
.github/workflows/rust-benchmark.yml.

## Unverified / residual

- No native Windows execution locally. Cross-target compilation passed and PR #19 had green native
  Windows CI before the verifier repair commits; the scheduled Miri job has not run yet.
- Miri shim availability differs by OS; the named local macOS refusals are not a prediction of the
  first Ubuntu scheduled run.
- E27-S07 owns the coverage-ratchet discrepancy. Its baseline has not been changed here.
