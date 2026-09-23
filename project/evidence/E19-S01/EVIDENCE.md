# Evidence Packet - E19-S01

- Commit/PR: the commit carrying this packet on `main`
- Executor: Claude
- Independent verifier: Codex (pending, E19 round 1)
- Change Risk: CR3
- Spec version/commit: `project/epics/E19.json` E19-S01 (acceptance criteria 3-5 added 2026-09-23
  to state the unwanted-behaviour cases the two original criteria implied)

## Outcome

PASS

## Acceptance Criteria Evidence

| AC | Evidence | Result |
| --- | --- | --- |
| AC1 "Desktop cannot bypass safety executor." | `cancellai-desktop-api/Cargo.toml` depends on no crate that reaches a provider root or the mutation executor; `protocol::tests::no_mutating_request_is_expressible`; `tests/unauthorized_clients.rs::a_mutating_request_is_not_part_of_the_protocol_even_when_authenticated`; `cancellai-cli/tests/desktop_api.rs::serving_a_plan_full_of_delete_candidates_mutates_nothing` (every document kind served against a tree whose plan proposes deletions; file snapshot identical before and after); `scripts/check_mutation_boundary.py check` | PASS |
| AC2 "API is local-authenticated and versioned." | `transport::Server::bind` binds `127.0.0.1:0` and draws a 256-bit token with `getrandom`; `auth::tests::*`; `version_one_request_frames_are_stable`, `version_one_response_frames_are_stable`; `the_descriptor_is_loopback_and_one_json_line` | PASS |
| AC3 "If a local client connects without the session token, with a wrong token, or before authenticating, then the API shall refuse the request, disclose no engine data, and close the connection." | `a_document_request_before_hello_is_refused_and_the_connection_closed`, `a_wrong_empty_or_truncated_token_is_refused`, `a_token_from_another_server_is_refused`, `a_browser_style_http_request_is_refused_without_engine_data`, `an_oversized_frame_is_refused_before_it_is_parsed` - each also asserts the engine was never asked for a document; `an_unauthorized_client_gets_nothing_and_the_server_keeps_serving` (real binary) | PASS |
| AC4 "If a client requests an API version the engine does not support, then the API shall refuse it and name the versions it supports." | `an_unsupported_version_is_refused_with_the_supported_list` (0, 2, `u32::MAX`), `the_client_reports_version_refusal_and_refuses_non_loopback_addresses` | PASS |
| AC5 "The API shall expose read-only documents only - the same inventory and plan documents the CLI's `status --json` and `plan --json` produce - and no request that performs or schedules a mutation." | `documents_match_the_cli_json_output_for_the_same_tree_and_query` (status, inspect and plan for three queries against the real binary, equal to the CLI output apart from the generation timestamp and the clock-derived retention cutoff); `a_custom_root_plan_reports_the_same_withholding_the_cli_reports`; the CLI builds both from the same `inventory_doc`/`plan_actions`/`plan_doc` functions | PASS |

## Safety Evidence

| Invariant | Counterexample tested | Evidence | Result |
| --- | --- | --- | --- |
| SI-007 | A request or flag that could resolve toward `clean` | no such request exists; `clean`-kind document and extra `yes`/`execute` fields refused | PASS |
| SI-019 | Desktop path reaching deletion | no dependency path; no-mutation snapshot test | PASS |
| SI-002 | Custom-root plan shown as mutation-eligible | withholding carried in the envelope, equal to CLI | PASS |

## Verification Commands

```text
cargo fmt --check                                                        -> ok
cargo clippy --workspace --all-targets --all-features -- -D warnings     -> ok
cargo clippy ... --target x86_64-pc-windows-gnu -- -D warnings           -> ok
cargo test --workspace                                                   -> all passed
cargo test -p cancellai-desktop-api                                      -> 20 passed
cargo test -p cancellai-cli --test desktop_api                           -> 5 passed (4 consecutive runs)
cargo deny check                                                         -> ok
python3 scripts/check_coverage.py check                                  -> OK; cancellai-desktop-api 91.11%
python3 scripts/check_rust_workspace.py check                            -> 14 crates match TARGET.md
pre-commit run --all-files; python3 -m pytest tests -q                   -> passed
```

## Compatibility

- New outer-ring crate `cancellai-desktop-api` (ADR-0019): `serde`, `serde_json`, `getrandom 0.4`
  (already in `Cargo.lock`; MIT OR Apache-2.0). No kernel-ring manifest changed.
- New CLI subcommand `desktop-api`; help goldens updated (`top_level_help.txt`, new
  `desktop_api_help.txt`). No existing command changed behaviour: `cmd_read_only` now calls the
  extracted `inventory_doc`/`plan_actions`/`plan_doc`, whose bodies are the code it inlined.
- Not packaged: `release.yml` builds only `cancellai-cli`, which now carries the subcommand.

## Performance / operability

- Single-threaded server, one connection at a time, 30 s idle timeout, 16 KiB request cap,
  256 MiB response cap on the client.

## Documentation updated

- `docs/architecture/TARGET.md` (crate list; "Desktop API boundary (E19-S01)"), `CHANGELOG.md`.

## Method defects

- **What happened**: the first parity test compared documents built at two different moments with the running-process guard on, and failed only when a `claude`/`codex` process started or stopped in between (as the concurrent Codex review did), then again when the two builds straddled a second boundary in the retention-cutoff explanation. **Prevented by**: none exists - no document says which inventory fields are wall-clock or live-process dependent, so every parity comparison has to rediscover them. **Disposition**: proposed 2026-09-23

## Residual risks

- Token possession is the authentication; a local process that can read the parent's pipe or
  memory can read the token too - the user-account trust boundary.
- One client at a time; a hostile local client can occupy the server for up to the idle timeout.
- The API serves documents only; no desktop client of it existed before E19-S02.

## Verifier verdict

(pending - independent reviewer)
