# Design consultation - E06-S04 and E33-S01 (2026-09-24)

Not a review round and not a verdict. The owner asked Claude (executor), Codex and OpenCode for
alternative designs after nine failed review rounds. Codex ran read-only (`codex exec -s read-only`)
on HEAD `57b1a1b`; OpenCode ran under the E34-S06 sandbox and produced no output (three attempts,
all `503 Service temporarily overloaded` from the free model). The prompt is reproduced first,
then Codex's answer verbatim. The decision it fed is ADR-0040.

## Prompt

### Design consultation - cancellAI E06-S04 and E33-S01 (not a review round)

You are one of three models (Claude = executor, Codex, OpenCode/Nemotron) asked by the project owner
to stop a review loop by finding **alternative designs**, not further patches. Do not write a verdict.
Do not modify any file. Read the repository at the current directory (HEAD) and answer in your final
message only.

#### Why this is being asked

Two CR4 stories have failed nine independent review rounds between them, each round finding a new
hole in a mechanism that was patched after the previous one:

- **E06-S04 - Canonical engine switch** (`project/epics/E06.json`; evidence
  `project/evidence/E06-S04/EVIDENCE.md`, `SAFETY_VERDICT.md`; review records
  `project/evidence/E06-VERIFIER-REVIEW-ROUND6.md` .. `ROUND9.md`; code `scripts/release.py`
  - `render_formula`, `live_formula_problems`, `published_digest_problems`, `engine_sha256s`,
  `finalize`, `verify_installed`; `.github/workflows/release.yml`, `tests.yml`, `rust.yml`).
  Rounds found in turn: formula-shape parsing fooled by Ruby nesting, adoption guards bypassable,
  version checks, digests trusted from `.sha256` sidecars instead of archive bytes, the release
  manifest not consulted, native per-platform evidence missing. AC1 also needs the owner to accept
  a migration Safety Verdict, which can only follow an independent PASS.
- **E33-S01 - Signed containment notices fetched from a published feed** (`project/epics/E33.json`;
  evidence `project/evidence/E33-S01/`; code `rust/crates/cancellai-store/src/containment_state.rs`,
  `rust/crates/cancellai-cli/src/containment.rs`, tests `rust/crates/cancellai-cli/tests/containment.rs`).
  Rounds found in turn: replay/current-sequence handling, concurrent refreshes writing duplicate
  lines that broke replay, then (after a chain-digest + nonce redesign) losing appends still writing
  bytes. The latest repair (commit b0cef3d) serializes appends with an exclusive SQLite transaction
  on a lock database and compares the chain head before writing.

Constraints you must respect: `AGENTS.md`, `docs/CONSTITUTION.md`, `docs/security/SAFETY_INVARIANTS.md`
(notably SI-019 one mutation boundary, SI-022, SI-029), ADR-0019 dependency rings, ADR-0039 cutover
perimeter, MSRV 1.88 (so `std::fs::File::lock` is unavailable), `unsafe` only in `cancellai-sealedfs`.

#### What to answer, for each story separately

1. The **root cause** of the recurring failures, in one paragraph: why does patching keep leaving
   a hole? Is the mechanism, the verification method, or the story's scope the problem?
2. **One or two alternative designs** that remove the failure class by construction rather than by
   more checks. For E06-S04 consider, among others: generating the Homebrew formula inside the
   release workflow from the bytes it just built (no post-hoc download), verifying GitHub
   artifact attestations / the signed release manifest instead of re-deriving digests, making the
   formula a pure function of the manifest, or splitting the story (e.g. cutover mechanics vs.
   release-integrity verification vs. owner acceptance). For E33-S01 consider, among others: the
   ledger as a SQLite table in one transaction (read-decide-append atomically), vs. the current
   JSONL + lock database, and what crash atomicity each gives.
3. For each design: which past findings (by round/ID) it eliminates, what residual risk remains,
   what it costs, and the **smallest adversarial test** that would falsify it.
4. Your recommendation, and whether the story should be split or its acceptance criteria rewritten
   (quote the AC you would change).

Be concrete: cite files and functions. Keep it under ~1200 words.

## Codex

### E06-S04 — Canonical engine switch

**Root cause.** The story combines three decisions with different evidence: which engine the formula installs, whether published bytes are the tested bytes, and whether the owner accepts the migration. Earlier `scripts/release.py` checks tried to recover those facts from a live Ruby formula and published sidecars. That made each repair another interpretation of independently editable material: rounds 6–8 found formula shapes the checks misread; round 9 found that matching sidecars said nothing about archive bytes. HEAD now generates the whole formula with `render_formula` and hashes archives in `engine_sha256s`, a substantial change in mechanism. The remaining design problem is that `finalize` still coordinates publication, formula replacement, and a status based owner gate across separate steps.

| Design | Findings eliminated | Residual risk and cost | Smallest falsifying test |
| --- | --- | --- | --- |
| **Generate the formula in `.github/workflows/release.yml` from the downloaded build artifacts and tag source archive**, after `release_manifest.py verify-checksums`; install and test that exact candidate before publication. Publish the formula and archives only after all native jobs pass. | Removes round 6 F-01/F-02 and R7-01/R7-02/R8-01/R8-02’s post hoc formula parsing and adoption errors. Deriving digests from artifact bytes removes round 9’s sidecar substitution during generation. | A released asset could differ from the workflow artifact or change after publication. Publication ordering and a post-publication identity check still matter. Costs a release workflow change and a staged formula artifact. | Replace one public archive after the staged formula is generated; the release must fail its public asset check before the formula becomes canonical. |
| **Make `render_formula` a pure function of a verified release manifest**, with artifact names, targets, digests, source digest, version, and channel fixed in one input. Verify the manifest’s provenance and each referenced public archive against its digest before adoption; retain the existing native install and parity gates. | Eliminates the formula-shape findings through round 8 and round 9’s sidecar-only claim. It also gives `finalize` one candidate to validate before `write_atomically`. | A manifest is only a claim until bound to attested build artifacts and the bytes users can download. GitHub attestations alone do not prove a later public URL serves the same bytes. Costs a defined manifest-to-formula contract and public asset verification. | Serve a correctly shaped manifest and sidecar while changing one archive byte; adoption must refuse. |

**Recommendation.** Use the build-time formula, with the manifest as its sole structured input, then check public asset identity before switching the tap. Keep `.github/workflows/tests.yml`’s exact installed-version and `brew test` checks, and `.github/workflows/rust.yml`’s native partial-scan evidence; local cross-compilation does not satisfy the story’s platform verification. Split release-integrity mechanics from the owner’s cutover decision. In particular, move **“Owner-visible migration Safety Verdict is accepted.”** to a final cutover authorization item that follows an independent PASS. `finalize` currently reads E06-S04’s `done` status as that authorization; a story status should not stand in for the owner’s explicit decision. Preserve the Python transition and release-note criteria in that final item.

### E33-S01 — Published containment notices

**Root cause.** `cmd_refresh` and `install_text` decide from a replayed history, while `containment_state::append_event` commits to a separate JSONL file. The chain digest and nonce can identify a losing write after it happens, but cannot make the decision and physical append one transaction. Round 8 exposed duplicate accepted sequences; round 9 exposed losing lines that still consumed bytes. HEAD’s exclusive SQLite transaction now rechecks the head before `write_line`, so it addresses that race, but SQLite locks one file while the committed event is written to another. A crash during that write can still leave a torn, unusable history.

**Alternative design: make the ledger itself a SQLite table.** Store raw signed notice text and local lift events as ordered rows. In one `BEGIN IMMEDIATE` transaction, read and replay the rows through the existing `cancellai-safety` verification path, decide whether the fetched notice is new/current/rollback, insert at most one row, and commit. Both `containment install` and `refresh` use this transaction path; SQLite remains persistence, never a second authority engine (SI-019/SI-022). A committed row is visible in full and an interrupted transaction contributes no row, subject to SQLite’s configured durability and filesystem guarantees. This removes R8-06 and round 9’s physical losing append by construction, and removes the separate lock-file/JSONL crash gap. Round 6 F-05 and R7-05 still require verified replay and publisher-sequence rules inside the transaction; F-06’s fixed curl path remains a separate transport control.

The cost is a CR4 migration of existing JSONL histories, an ADR-0039 update, and an owner decision about AC2’s literal byte requirement. A SQLite transaction can leave database or journal metadata changed even when its **event rows** do not change. Rewrite **“then the ledger shall stay byte-identical and authority shall keep being computed from local state”** to require *the ordered event rows and their signed raw payloads to remain identical, with authority computed from the last committed, verified local history*. A corrupt old JSONL file must migrate as unknown, never as an empty ledger. The accepted same-user tamper limit remains.

The smallest falsification test is two processes refreshing the same signed notice, with one killed immediately before commit: afterward there must be exactly one row or none, every successful response must correspond to a committed row, and replay must remain usable. Also compare rows before and after a refused rollback. I recommend splitting the ledger migration into a prerequisite story and keeping feed transport/ingestion in E33-S01; each then has a single CR4 failure surface and independent evidence.
