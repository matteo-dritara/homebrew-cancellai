Review-Scope: story
Round: 1

# E06 Self-Review - E06-S08

- Reviewer: a forked context of the executor (Claude) - a **self-review** under AGENTS.md: it may
  find and report defects, it is not an independent verdict. E06-S08 is CR1 and closes on this
  review plus `project/evidence/E06-S08/CEILING_DECISION.md` after reaching the owner's
  two-independent-review limit (rounds 3 and 4).
- Date: 2026-09-23
- Reviewed: `scripts/cutover_benchmark.py`, `tests/test_cutover_benchmark.py`, `rust.yml`
  `cutover-budget` job, at `7f6c554`.

| Story | Verdict | Evidence |
| --- | --- | --- |
| E06-S08 | PASS_WITH_RESIDUALS | Both round-3/4 counterexamples are refused: a Rust stand-in exiting 7 (`exited 7`), printing nothing (`printed nothing`), and exiting 0 with `x` (`does not describe the corpus`) - each pinned by a test in `tests/test_cutover_benchmark.py`, all 14 passing. A reference that fails or prints nothing is refused by the same `failed_run`, and a wrong count from either engine by `result_matches_corpus`. A nightly (non-stable) build is refused by the corpus liveness check before timing. **Residual, reproduced:** the validation is a plausibility check on counts, not proof of work. A stand-in that runs the real binary for `plan` (the liveness check) and prints canned, corpus-shaped text for the rest - `claude-code: 5 artifact(s)` / `codex-cli: 5 artifact(s)` for `status`, a 10-element `artifacts` array for `inspect`, `6 delete candidate(s)` for `clean --dry-run` - passes `cutover_benchmark.py check --sessions 5 --runs 1` with exit 0 and timings of 5 ms. |

## Counterexample (reproduced)

```sh
REAL=rust/target/stable-channel/release/cancellai-cli   # stable-channel release build
cat > /tmp/fake <<EOF
#!/bin/sh
case "\$1" in
  plan) exec "$REAL" "\$@" ;;
  status) printf 'claude-code: 5 artifact(s), 0 bytes, scan_complete=true\ncodex-cli: 5 artifact(s), 0 bytes, scan_complete=true\n' ;;
  inspect) printf '{"artifacts":[1,2,3,4,5,6,7,8,9,10]}\n' ;;
  clean) printf '6 delete candidate(s)\n' ;;
esac
EOF
chmod +x /tmp/fake
python3 scripts/cutover_benchmark.py check --sessions 5 --runs 1 --rust-bin /tmp/fake
# -> status 0.005s / inspect 0.005s / clean --dry-run 0.005s ... "cutover performance budget OK", exit 0
```

## Why this is a residual, not a FAIL

The gate's threat model is an accidental regression in the engine CI builds from source
(`rust.yml` always passes the freshly built binary), not an adversarial `--rust-bin`. A regression
that stops doing work does not reproduce the corpus's exact per-provider counts and delete count
while skipping the scan; the canned stand-in needs the corpus parameters baked in. Closing it
fully would mean making the corpus unpredictable per run - random session UUIDs, random session
count - and requiring the JSON outputs (`inspect`, `plan`) to name those ids; the human `status`
and dry `clean` outputs carry no ids and would stay count-checked.

## Suggested backlog item

Randomize the benchmark corpus per run (session count and UUIDs) and require `inspect`/`plan` to
name the generated ids, so a canned output cannot pass. Not required by E06-S08's criteria or
round 4's repair.
