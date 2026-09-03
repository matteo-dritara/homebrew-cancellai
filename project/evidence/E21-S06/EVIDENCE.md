# Evidence Packet - E21-S06

- Commit/PR: pending (this work item)
- Executor: Claude
- Independent verifier: pending - E21 epic review round 1
- Change Risk: CR1
- Spec version/commit: `docs/audits/2026-09-03-CODE_REVIEW.md`, finding `CR-TE-04`

## Outcome

PASS

## Scope

`read_codex_parent_session_id` documented reading "without scanning the whole file - bounded to
the first 10 lines / 512KiB", and then called `fs::read(path)` followed by
`String::from_utf8_lossy` over the entire content, applying the bound afterwards. The cost scaled
with the largest transcript on disk, and agentic session transcripts grow without limit.

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - at most the first 10 records or 512 KiB are read, streamed rather than buffered whole | Parsing moved to `read_parent_from(impl BufRead)`, driven by `BufReader` over the file. Each record is read with `read_until` on a `Read::take` bounded by the remaining budget, so neither a large file nor a single enormous line can exceed it. | PASS |
| AC2 - peak memory during discovery is bounded independently of the largest transcript | Measured end-to-end on the audit's own 287 MB rollout: peak RSS **2.9 MB**, against 303 MB before - and an order of magnitude below the Python reference's 27.7 MB on the same file. | PASS |
| AC3 - the parsed result is identical on every fixture, including malformed and non-UTF-8 input | Byte accounting is deliberately identical to the previous implementation (`line.len() + 1` on the newline-stripped, lossily-decoded line), and `record_selection_is_unchanged_by_streaming` pins CRLF handling, the 10-record cutoff and invalid UTF-8 tolerance. The differential gate stays green across all 12 fixtures. | PASS |

## Safety Evidence

Not safety-bearing (CR1). Stated explicitly because the function feeds subagent-tree grouping,
which `keep_latest` depends on: any change in *which record wins* would be a semantic
divergence, not a performance improvement. That is what AC3's tests and the differential gate
exist to exclude here.

## Verification Commands

The memory claim is proven directly rather than by proxy, on bytes actually consumed:

```text
$ cargo test -p cancellai-provider-codex
test session::tests::a_reader_is_never_consumed_beyond_the_budget ... ok
test session::tests::a_single_enormous_line_cannot_pull_the_file_in_through_the_back_door ... ok
test session::tests::record_selection_is_unchanged_by_streaming ... ok
```

`a_reader_is_never_consumed_beyond_the_budget` drives `read_parent_from` with a `CountingReader`
over a 64 MiB input and asserts on bytes pulled out of it. That is the quantity the defect was
about, so it is the quantity asserted - no memory profiler, no dependency, no proxy.
`a_single_enormous_line_…` covers the failure mode a naive `read_line` loop would still have.

End-to-end, `status --tool codex` against a single 287 MB rollout:

| | peak RSS | wall clock |
| --- | --- | --- |
| Before (`CR-TE-04` measurement) | 303.0 MB | 0.35 s |
| Python reference | 27.7 MB | 0.20 s |
| After this story | **2.9 MB** | 0.42 s |

```text
python3 scripts/rust_python_parity.py check   12 NORMATIVE fixtures, both scenarios, OK
cargo test --workspace                        318 passed, 0 failed
```

## Compatibility

- No API change: `read_codex_parent_session_id(&Path)` keeps its signature. `read_parent_from` is
  `pub(crate)`, added so the bound is testable rather than asserted.

## Performance / operability

- Wall clock is not improved and is reported unchanged rather than dressed up: at 0.42 s against
  the reference's 0.20 s this path is still slower end-to-end, and most of that is process
  startup and the liveness probe, not the read. This story's claim is the memory bound, and only
  that.

## Documentation updated

- The function's own module docs now record what the old note claimed and what it actually did.
- `docs/development/RELEASE_GATES.md` G4 carries the before/after measurement.

## Residual risks

- The 10-record / 512 KiB budget itself is unchanged and remains a heuristic inherited from the
  reference: a rollout whose `session_meta` sits past it still reads as "no parent", exactly as
  before. That is the reference's contract, not a regression, and changing it would be a
  divergence requiring its own story.


## Round-1 independent review: FAIL, and its repair

The verifier failed the story on AC1 with an exact reading: `read_parent_from` computed
`take(remaining + 1)` and the test explicitly permitted `MAX_PARENT_SCAN_BYTES + 1`, so the
implementation could read 524,289 bytes against a documented 512 KiB maximum. A bound that
admits an off-by-one is not the bound the function documents.

Repair: two counters, separated on purpose. `read_total` is the bytes actually pulled from the
reader and is what `MAX_PARENT_SCAN_BYTES` bounds; `consumed` remains the reference-compatible
accounting (`line.len() + 1` on the newline-stripped line) that decides where parsing stops,
kept distinct because a CRLF line costs one more real byte than the reference counts for it. A
record cut short by the budget is not parsed at all, which keeps selection identical to the
whole-file version. The counting-reader regression now asserts `<= MAX_PARENT_SCAN_BYTES`.

## Verifier verdict

`FAIL` (round 1) - repaired above; owner-accepted closure without a round 2, see project/evidence/E21-CLOSURE.md
