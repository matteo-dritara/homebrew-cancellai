# Evidence Packet - E06-S07

- Commit/PR: the E06-S07 commit on `main`
- Executor: Claude
- Design review: Codex (2026-09-23, before implementation) - three findings, all addressed below
- Independent verifier: pending - reviewed with the rest of E06's cutover stories
- Change Risk: CR4
- Spec version/commit: `project/epics/E06.json` at this commit; ADR-0039

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 - every plan/clean action's authority computed with effective_authority_under_containment from the compiled channel and the persisted ledger; sealed at that authority | `containment::apply_authority` runs inside `plan_actions`, which `plan`, `clean` and the desktop API all use; each `Delete` gets `effective_authority_under_containment(inputs, BuildChannel::from_compiled_env(), ledger, target)` with inputs from the artifact's own facts (Codex's Q1 answer). `execute_clean` seals at `action.authority`, no longer a constant. | PASS |
| AC2 - below the minimum: plan and clean both downgrade to Observe, naming constraint and incident | `a_contained_provider_is_withheld_in_plan_and_clean_and_an_uncontained_one_still_deletes` (the plan names `INC-1`; clean exits 4, keeps the contained session, deletes the uncontained one); `a_nightly_build_withholds_every_deletion_and_says_why` (names `release_channel_authority`). | PASS |
| AC3 - install of a notice from the project key or an owner-trusted publisher is ingested and persisted before success | `cmd_install`: replay current history, `ContainmentLedger::install`, append one line, then re-read and re-verify the persisted history before reporting success. The kernel test `the_project_key_verifies_a_project_notice_and_nothing_else_does` covers the compiled-key path; the CLI tests cover an owner publisher. | PASS |
| AC4 - oversized, unsigned/forged, untrusted, malformed, replayed or expired notice refused, history byte-identical, non-zero exit | `every_refused_notice_leaves_the_history_byte_identical` - seven cases, each exit 4 and the history compared byte for byte; `install_refuses_an_oversized_notice_before_parsing` (kernel); the CLI reads at most `MAX_NOTICE_BYTES + 1` bytes before parsing. | PASS |
| AC5 - an existing history or trust file that cannot be read, parsed or validated caps everything at Recommend and says why | `an_unreadable_or_unverifiable_history_caps_everything_at_recommend` (truncated, garbage, unsigned install) and `a_malformed_or_reserved_owner_trust_file_is_unknown_not_ignored` (malformed, bad key, reserved id): no deletion planned, `containment_ledger_unknown` named, clean exits 4 and deletes nothing. | PASS |
| AC6 - a missing history is an empty ledger | `no_state_directory_is_an_empty_ledger_and_plan_creates_none`: both deletions planned, and `plan` does not create the state directory (`LocalStateRoot::existing_platform_default`). | PASS |
| AC7 - an expired persisted containment still binds | `an_expired_containment_still_binds_after_install` (installed 2 s before expiry, checked after); kernel `install_checks_expiry_but_replay_does_not_lift_an_expired_containment`. | PASS |
| AC8 - no trust, ceiling or lift from notice content | Trust comes only from `containment_trust_policy(compiled key, owner file)`, every publisher at `TrustedTier::untrusted()`; the owner file may not reuse the project id or repeat an id (`the_containment_trust_policy_refuses_a_reserved_duplicate_or_invalid_publisher`). The notice vocabulary is unchanged from E17-S07 (no lift/restore/tier field). Lifts come only from `containment lift --confirm` (`only_a_confirmed_local_lift_removes_a_containment`). | PASS |

## Design review findings (Codex, before implementation)

| Finding | Disposition |
| --- | --- |
| A same-user edit of the persisted history (unsigned lift, valid-prefix rollback) lifts containment | Owner decision 2026-09-23: the guarantee is narrowed to inputs from outside the owner's account and documented in SI-029, INCIDENT_RESPONSE.md, ADR-0039 and the store module. |
| Concurrent commands can lose an install; clean may miss a notice installed after planning | The history is append-only (`O_APPEND`, one write per event, no rename, no lock file to remove), so no write is lost; an out-of-order or torn append fails replay and reads as unknown. `clean` re-checks the ledger immediately before every deletion (`containment::recheck`). |
| An action with no classified target must fail closed | `apply_authority` withholds a `Delete` whose target this run did not classify. |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-022 | Evidence created from a file | The store keeps raw bundle text; `ContainmentLedger::replay` re-verifies signature, digest, publisher and sequence for every event; `replay_re_verifies_every_install_and_fails_whole_on_any_bad_event` | PASS |
| SI-029 | Expiry, replay, forged or untrusted input lifting containment | Kernel and CLI tests above; replay skips only expiry, through a crate-private path | PASS (owner-state limit accepted) |
| SI-030 | A nightly build deleting | `a_nightly_build_withholds_every_deletion_and_says_why` | PASS |
| SI-019 | A second deletion path | No mutation code added; the store only appends; `check_mutation_boundary.py` passes | PASS |

## Verification Commands

```text
cargo test -p cancellai-safety                                                        -> 211 passed
cargo test -p cancellai-store                                                         -> pass
cargo test -p cancellai-cli (nightly build)                                           -> pass; deletion tests skip loudly
CANCELLAI_CHANNEL=stable cargo test -p cancellai-cli --features kill-points (own target dir) -> pass, incl. containment (7) and kill harness (3)
cargo clippy --workspace --all-targets --all-features (host + x86_64-pc-windows-gnu)   -> clean
python3 scripts/rust_python_parity.py check                                           -> 14 NORMATIVE, both origins (stable build)
python3 scripts/cutover_benchmark.py check                                            -> budget OK (stable build)
```

## Residual risks

- **Owner-state limit** (accepted): a same-user process can lift containment by editing the history.
- **Concurrent installs** from one publisher can land out of order; the history then fails replay and
  every deletion is capped at Recommend until the owner removes the bad line - fail closed, noisy.
- **Local builds cannot delete.** Any build without `CANCELLAI_CHANNEL=stable` withholds every
  deletion. That is SI-030 working, and it means a source build (e.g. a future Homebrew formula
  building Rust) must set the variable. No such formula exists yet.
- **A notice installed during a deletion already in flight** binds from the next deletion on.

## Verifier verdict

pending
