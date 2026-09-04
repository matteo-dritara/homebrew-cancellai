# E22-S02 - Round 2 repair (independent verifier review round 1 findings)

- Story: E22-S02
- Round: repair after `project/evidence/E22-VERIFIER-REVIEW.md` (round 1, FAIL)
- Date: 2026-09-04

## Verdict this repairs

Round 1 verdict: FAIL, not because the configuration was wrong, but because it had never been
exercised for real: `origin/main` was stuck at `d0df840` while the target commits were local
only, so GitHub had never parsed or run the added `analyze-rust` CodeQL job or the `cargo`
Dependabot entry. The verifier's own evidence: latest real CodeQL run `33749220786` at
`d0df840`, job list `[Python reference security analysis]` only; `open Cargo Dependabot alerts
returned: []`.

## What changed

No code or configuration changed for this story specifically (the round-1 `codeql.yml`/
`dependabot.yml` content was already correct - `docs/PROVIDERS.md`/AC text confirms this).
What changed is that the target commits are now on GitHub, so both required real-service
verification bullets could actually be exercised:

1. `a407917` (this round's S01/S03/S04/S05 repair commit) was pushed to `origin/main`.
2. A synthetic outdated pin was created and pushed (`25b49f8`, since reverted - see below):
   `cargo update -p libc --precise 0.2.170` against `cancellai-sealedfs`'s `libc = "0.2"`
   requirement (still satisfied, build unaffected).
3. Dependabot's "Check for updates" was triggered manually for the `rust/Cargo.toml`
   ecosystem via the repository's Insights -> Dependency graph -> Dependabot UI (the cargo
   ecosystem's first automatic evaluation, triggered by `a407917` reaching `main`, had found
   nothing outdated and so produced no PR - a manual check was required after the synthetic
   downgrade to get a fresh evaluation without waiting for the monthly schedule).
4. The resulting Dependabot proposal (PR #18) was reviewed and merged, both closing the
   synthetic exercise and legitimately restoring `libc` to its real latest patch version.

## Verification (real GitHub evidence, not configuration inspection)

**CodeQL Rust analysis, real and non-empty:**

```
$ gh api repos/matteo-dritara/homebrew-cancellai/code-scanning/analyses \
    --jq '.[] | select(.commit_sha=="a407917a763c2a322c2b0c9e7b8ba2e752def46b")'
{"category":".github/workflows/codeql.yml:analyze-rust","commit_sha":"a407917...",
 "ref":"refs/heads/main","results_count":0,"rules_count":25,"created_at":"2026-09-04T09:52:58Z"}
{"category":".github/workflows/codeql.yml:analyze-python-reference","commit_sha":"a407917...",
 "ref":"refs/heads/main","results_count":0,"rules_count":43,"created_at":"2026-09-04T09:51:09Z"}
```

Run `33860311566` (job "Rust target-engine security analysis") completed successfully: checkout,
Rust toolchain, `codeql-action/init` (languages: rust), `cargo build --workspace`,
`codeql-action/analyze` all succeeded. `rules_count: 25` for the Rust category confirms a real
query suite ran against real compiled Rust code (0 findings is a clean result, not a skipped
language - contrast the Python category's `rules_count: 43` on the same commit, both non-zero
and independently reported). This closes AC2/AC3 and the "CodeQL Rust job completes on a real
run and reports a non-empty analysis, not a skipped language" verification bullet.

**Cargo Dependabot produces a real proposal for a real outdated pin:**

```
$ cd rust && cargo update -p libc --precise 0.2.170   # 0.2.189 -> 0.2.170, still satisfies "0.2"
$ cargo build --workspace                              # unaffected
$ git commit -m "test(deps): synthetic outdated libc pin..." && git push origin main
```

Dependabot's first automatic evaluation of the newly-live `dependabot.yml` (triggered when
`a407917` first reached `main`) ran before the synthetic downgrade and correctly found nothing
outdated (`cargo in /rust - Update #1557296200`, real log excerpt: "No update needed for
unicode-normalization 0.1.25", "No update possible for cancellai-sealedfs 0.1.0" - each
workspace member and dependency was individually resolved and already current). After the
synthetic downgrade, a manual "Check for updates" (GitHub Dependabot UI,
`network/updates/40559446/jobs`) produced:

```
$ gh api repos/matteo-dritara/homebrew-cancellai/pulls \
    --jq '.[] | select(.user.login=="dependabot[bot]")'
{"number":18,"title":"chore(deps): bump libc from 0.2.170 to 0.2.189 in /rust",
 "created_at":"2026-09-04T10:01:48Z"}
```

PR #18's own CI (rust/codeql/tests/governance) passed and it was merged, restoring the real pin.
This closes AC1 and the "a synthetic outdated pin produces a dependabot proposal" verification
bullet - Dependabot genuinely resolves this Cargo workspace and can propose a real update.

## Residual risk

Dependabot's automatic (non-manual) trigger cadence for `cargo` is the configured monthly
schedule (`dependabot.yml`), same as the other two ecosystems; a manual "Check for updates" or
a real outdated dependency landing in the interim are the only ways to get an earlier proposal.
This is existing, intended Dependabot behavior, not a gap this story introduced.
