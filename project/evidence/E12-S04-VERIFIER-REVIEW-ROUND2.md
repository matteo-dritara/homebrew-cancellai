# E12-S04 Verifier Review — Round 2

Review-Scope: story
Round: 2
Review target: repair commit `7a70559325558bae972805f5754d892f825c564a`; full story history `f215d33..9d47f4df95e28640f7afdcf8fc931b2273101a35` on `main`
Verifier: Codex
Brief-Checksum: 13ff307a3683218ad373346e3d17d1530ec685aa514195ee7bd8251f394f2a12
Date: 2026-09-20

## Verdict

`FAIL`

The repair closes the literal round-1 reproduction, but does not establish AC1's required
property. Its `validate_content_safe` accepts any one-to-four-word ASCII, hyphen-separated
string. That is still a caller-controlled, persistent content channel: the meaningful prompt
`do-not-purge-this` is accepted and round-trips through every one of the seven retained
fields. This is a structural counterexample to the stated claim, not a spelling variation of
the original underscore/path sentinel.

## Round-1 reproduction re-attempt

I independently compiled and ran an uncommitted public-API verifier test. The exact round-1
inputs (`PROMPT_SENTINEL_do_not_store_source_contents` in `provider_id`, `reason_code`,
`policy_id`, `plan_id`, and the evidence ID; `/private/provider/source.rs` in `category`) now
return `PurgeTombstoneError::UnsafeField` before append. `EventLedger::read_all()` is empty.
Thus the literal prior reproduction is refused with nothing written.

## Independent additional adversarial attempts

The temporary public consumer used only `cancellai_store::tombstone::record_purge_tombstone`,
`Tombstone`, and public model IDs. It was removed after the run and is not part of this review
commit.

| Case | Outcome | Evidence |
| --- | --- | --- |
| Exact round-1 prompt/path sentinels | Refused; no row | `UnsafeField`; `read_all().is_empty()` |
| Four-word prompt `do-not-purge-this` in `artifact_id`, `provider_id`, `category`, `reason_code`, `policy_id`, `plan_id`, and `EvidenceId` (seven individual calls) | **Accepted and persisted in every field** | Each `record_purge_tombstone` returned `Ok`; the read-back `PURGED` event contained the prompt. `ArtifactId::new` and `EvidenceId::new` publicly accept arbitrary `String`s, so neither bypasses this validator. |
| Non-ASCII `caf\u{00e9}`, newline, NUL, leading/trailing hyphen, doubled hyphen, and an over-long hyphen chain | Refused; no row | `UnsafeField`; `read_all().is_empty()` for each case. |
| Empty `plan_id`; empty `evidence_ids` | Refused; no row | The validator deliberately accepts the empty string, then `EventLedger::append` returns `Ledger`; no row is written in either case. The E13-S02 ledger contract remains intact. |
| `Delete + VendorConditional` | Refused; no row | `NotAnIrreversiblePurge`; `read_all().is_empty()`. AC2 remains intact. |

The documented bounds (`MAX_SEGMENTS = 4`, 32 characters per segment, 64 total) do reject
many malformed encodings, but they do not distinguish a short semantic instruction from an ID.
`do-not-purge-this` has four ordinary words, exactly fits the asserted real-ID shape, and is
meaningful retained prompt content. The same capacity can encode short source fragments or
secrets. This is therefore pattern restriction, not a principled contentlessness boundary.

## Safety obligation and required repair

AC1 remains failed, along with C-09 and SI-020's requirement that the explicit retained record
for an irreversible action not disguise retained content as cleanup metadata. SI-020's action
classification aspect is passing; its contentless tombstone record is not.

Required repair: remove the public `Tombstone` API's ability to accept arbitrary caller-selected
`String`/`ArtifactId`/`EvidenceId` bytes for persisted fields. Use non-forgeable, opaque
references allocated by trusted local code for artifact/plan/evidence linkage, and finite closed
vocabularies (or no retained value) for provider, category, reason, and policy semantics. A
lexical length/character/segment predicate is insufficient because valid-looking short natural
language is content. Make field construction private or otherwise authority-bound so public
callers cannot instantiate a reference from `"do-not-purge-this"`; reject before append and
prove zero rows on refusal. Add an external/public API adversarial regression that places a
short, validator-shaped meaningful phrase in every field and confirms the API cannot persist it.

## Gate status

| Command | Result |
| --- | --- |
| `cd rust && cargo test -p cancellai-store --test e12_s04_round2_verifier --offline` | PASS as a verifier experiment: it proved the round-1 sentinel is refused, AC2/ledger failures write nothing, and the short prompt is accepted and persisted through all seven fields. The temporary test was removed after observation. |
| `cd rust && cargo fmt --check` | PASS |
| `cd rust && cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cd rust && cargo check --workspace --all-targets` | PASS |
| `cd rust && cargo test --workspace` | PASS (121 `cancellai-store` unit tests; no workspace failures) |
| `cd rust && cargo deny check` | PASS (pre-existing unmatched-license and duplicate warnings only) |
| `python3 scripts/project_os.py generate`; `python3 scripts/project_os.py check` | PASS after the verdict/status transition |
| `python3 scripts/process_metrics.py generate`; `python3 scripts/process_metrics.py check` | PASS after adding this review record |
| `python3 scripts/check_process.py check` | PASS after the verdict/status transition (documented pre-existing review-round warnings) |
| `python3 scripts/check_evidence.py check` | PASS after the verdict/status transition (documented baseline warnings) |
| `python3 scripts/verifier_handoff.py check` | PASS after the verdict/status transition |

## Documents opened

`AGENTS.md`; `docs/INDEX.md`; `docs/CONSTITUTION.md`; `docs/BACKLOG.md`;
`project/epics/E12.json`; `project/evidence/E12-S04-VERIFIER-REVIEW.md`;
`project/evidence/E12-S04/SAFETY_VERDICT.md`; `project/evidence/E12-S04/EVIDENCE.md`;
`project/evidence/E12-S04/VERIFIER_BRIEF.md`; `docs/architecture/PERSISTENCE_MODEL.md`;
`docs/security/SAFETY_INVARIANTS.md`; `docs/development/ENGINEERING_SYSTEM.md`;
`docs/development/AGENT_PROTOCOL.md`; `docs/development/WORK_ITEM_MODEL.md`;
`project/templates/SAFETY_VERDICT.md`; `rust/crates/cancellai-store/src/tombstone.rs`;
`rust/crates/cancellai-store/src/ledger.rs`; `rust/crates/cancellai-store/src/lib.rs`; and
`rust/crates/cancellai-model/src/agent_artifact.rs`.

## Overall verdict

`FAIL` — E12-S04 returns to `in_progress`. The existing round-1 Safety Verdict remains an
owner-visible `REJECT`; this round supplies the final, precise required repair rather than
opening a third review round.
