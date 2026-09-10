# E16 Independent Verifier Review - Round 1

- Review target: `4d19ac62b864a12f6599b9875053bd6ccf7c6796..8696d7aeae9e82a76fefc1941702a5bfba06dc04` on `main`
- Verifier: Codex
- Date: 2026-09-11

## Verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E16-S01 | PASS | An independent Rust harness supplied `trust`, `authority`, `verified_by`, and `capability` keys at top/root/marker/artifact nesting; each was rejected. It also rejected `subdir: ../escape`, while the Gemini empty-home form remained bounded by its safe `.gemini` subdir. |
| E16-S02 | PASS | Local-policy-only tier provenance, injective length-prefixed signing bytes, expiry-before-mutation rollback, parser denial of untrusted fields, and verification-only `ed25519-dalek` features were independently inspected and exercised. See [Safety Verdict](E16-S02/SAFETY_VERDICT.md). |
| E16-S03 | PASS | The independent harness built a Gemini fixture tree and called every named `ProviderCapabilities` method; only detect/fingerprint/inventory were supported. The cited upstream `chatRecordingService.ts` constructs `chats` paths and `.jsonl` recordings. |
| E16-S04 | PASS | The same full capability harness confirmed the manifest-only ceiling. GitHub’s published Copilot CLI configuration-directory reference confirms `~/.copilot`, `COPILOT_HOME`, `config.json`, `settings.json`, and session state as the source layout. |
| E16-S05 | PASS | The harness covered all nine capability calls on a populated OpenCode tree. The cited upstream `storage.ts` places storage under `Global.Path.data/storage` and identifies the session/message/part/project/session-diff record layout. |
| E16-S06 | PASS_WITH_RESIDUALS | `check_provider_trust.py check` is read-only and runs in the release `verify` job under global `contents: read`. `CODEOWNERS` contains an explicit `/project/provider_trust.json` owner entry. A synthetic `builtin_verified` registry entry with a plausible verifier and nonexistent fixture path passes validation: the checker verifies presence, not fixture existence or execution. |

## Cross-epic dependency review

PD-023’s former cycle is real: the original E16-S02 → E17-S01 edge required E17 closure;
E17-S07 requires E16-S05 to be done; E16 could not close while E16-S02 waited on E17. The
removed edge is not load-bearing: E17-S01’s `knowledge_compatibility` schema was committed
before the E16 range and is unchanged by it. E17-S07 → E16-S05 remains but no longer returns to
E16-S02, so no cross-epic review-closure cycle remains.

## Gate status

- PASS: `python3 -m pytest tests -v` (268 tests), Ruff, formatting, MyPy, generated-document,
  governance, documentation, workflow, fixture, schema, characterization, differential,
  mutation-boundary, provider compatibility/trust, platform, process, release-manifest, and
  repository-topology checks listed in `AGENTS.md`.
- PASS: `cargo fmt --check`; Clippy with warnings denied; workspace check; workspace test;
  `cargo deny check` (warnings only: unmatched permitted licenses and the documented `syn`
  duplicate; advisories/bans/licenses/sources passed).
- PASS: independent E16 manifest/adapters harness, `cargo test -p cancellai-safety --doc`,
  `cargo tree -p cancellai-safety -i ed25519-dalek --edges features`, direct upstream source
  checks, and the fabricated trust-registry residual reproduction.

## Overall verdict

`PASS_WITH_RESIDUALS`

All six stories reached `done`. The epic cannot close yet: `project_os.py check` correctly
refuses an E16 `done` epic while declared epic dependency E15 remains unfinished. E16 is recorded
as `blocked`; no release evidence is due until that dependency permits epic closure. This is a
control-plane dependency, not an implementation or safety-review rejection.
