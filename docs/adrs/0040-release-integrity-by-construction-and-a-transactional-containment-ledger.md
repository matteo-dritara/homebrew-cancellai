# ADR-0040: Release integrity by construction, owner-authorized cutover, and a transactional containment ledger

- Status: Accepted
- Date: 2026-09-24
- Owners: project owner (decided 2026-09-24, after a design consultation with Codex, to replace
  the patched mechanisms rather than spend a tenth review round on them)
- Related: E06-S04, E06-S14, E06-S15, E33-S01, E33-S03, ADR-0039, ADR-0025, SI-019, SI-022, SI-029

## Context

Two CR4 stories failed nine independent review rounds between them, each round finding a new hole
in a mechanism patched after the previous one.

- **E06-S04 (canonical engine switch).** `scripts/release.py` tried to recover three different
  facts after the fact: what the formula installs (by parsing and re-rendering a Ruby file), whether
  the published bytes are the tested bytes (from `.sha256` sidecars, later from downloads), and
  whether the owner accepted the migration (from E06-S04's `done` status). Rounds 6-8 found formula
  shapes the checks misread; round 9 found that a sidecar agreeing with the formula said nothing
  about the archive's bytes. Each repair was another interpretation of material someone else could
  edit.
- **E33-S01 (containment feed).** The CLI decided from a replayed history and then appended to a
  separate JSONL file. A chain digest and a nonce could identify a losing write after it happened;
  a lock database could serialize appends; neither made the decision and the durable append one
  transaction, and a crash inside the one `write` still left a torn line.

Codex, asked for alternatives rather than a review, reached the same diagnosis independently
(recorded in `project/evidence/E06-S04/DESIGN_CONSULTATION.md`). OpenCode was asked too; its free
model was overloaded for every attempt.

## Decision

1. **The Homebrew formula is a pure function of the release manifest and the tag archive's
   digest, rendered where the bytes are built (E06-S14).** The release workflow's `publish` job,
   after `release_manifest.py verify-checksums` has tied every archive to the manifest, renders the
   formula from the manifest and publishes it as the release asset `cancellai.rb`. `finalize`
   adopts that asset only if it is byte-identical to what `render_formula` produces from the
   published manifest, and only after every engine archive the formula names has been downloaded,
   hashed to the manifest's digest, and its build provenance verified with
   `gh attestation verify`. Nothing is derived from a sidecar; a missing piece refuses.
2. **Cutover is adopted on an explicit owner authorization, not on a story status (E06-S15).**
   `finalize --adopt-cutover` requires `project/evidence/E06-S04/CUTOVER_AUTHORIZATION.md`, which
   names the version it authorizes and the SHA-256 of the `SAFETY_VERDICT.md` the owner accepted,
   and whose final round the file must still show as passing. Editing the Safety Verdict after the
   authorization voids it.
3. **The containment ledger is a SQLite table decided and appended in one transaction (E33-S03).**
   `containment install`, `refresh` and `lift` read the history, replay it through
   `cancellai-safety`, decide, and insert at most one row inside one `BEGIN IMMEDIATE` transaction.
   A losing or refused decision inserts nothing; an interrupted transaction contributes no row. The
   JSONL history, its chain digest, its nonce and the lock database are removed. No migration is
   needed: containment has never shipped. SQLite stays persistence only - every event is still raw
   signed text re-verified on every load, and the database never becomes a second authority engine
   (SI-022).

## Consequences

- E06-S04 keeps only what is the owner's: accepting the migration Safety Verdict, the Python
  transition window and the release notes. The mechanics it used to carry are E06-S14 and E06-S15.
- E33-S01's second acceptance criterion changes from "the ledger shall stay byte-identical" to "the
  ordered event rows and their raw signed payloads shall stay identical": a SQLite transaction may
  touch database metadata even when it inserts nothing.
- A release now needs `gh` with network access at `finalize`, as it already needed the network to
  download the tag archive. A release that cannot verify provenance cannot be finalized.
- The same-user limit ADR-0039 recorded is unchanged: a process running as the owner can still
  delete or edit the database, as it could the JSONL file.

## Alternatives considered

- **Another round of patches on each mechanism.** Rejected by the owner after nine rounds.
- **Verify only attestations, without hashing the downloaded bytes.** Rejected: an attestation
  proves where some bytes came from, not that the URL the formula names serves them.
- **Keep JSONL and hold an OS file lock.** `std::fs::File::lock` needs Rust 1.89 and the MSRV is
  1.88 (ADR-0026); raising it to keep a format nobody depends on yet is the wrong trade.
