# Safety Verdict - E06-S04

- Change: Canonical Rust-engine cutover gate
- Risk: CR4
- Commit/PR: review target `4024ab8..16a44b0`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-01

## Verdict

`FAIL`

## Safety surface changed

E06 adds a production Rust CLI path to permanent filesystem deletion and a direct Claude
configuration writer.  E06-S04 proposes no engine switch, but it is the CR4 gate that would
authorize Rust as canonical; therefore it must assess the complete new authority surface rather
than only whether a switch statement changed.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- |
| SI-019 | All filesystem/vendor mutations route through one evidence-gated safety boundary. | `configure_claude_retention` calls `std::fs::create_dir_all`, `write`, and `rename` in `cancellai-cli/src/main.rs`, outside `cancellai-safety`.  A pre-created temp-file symlink caused it to overwrite an outside sentinel file.  `check_mutation_boundary.py` passes only because it scans deletion primitives/capability names, not this mutation class. | FAIL |
| SI-007 | Ambiguous or invalid configuration remains non-destructive. | A malformed `settings.json` was accepted and replaced with a new object; `version --definitely-invalid` exits 0. | FAIL |
| SI-008 / SI-009 / SI-014 | Partial/unknown provider state does not authorize or report successful cleanup. | With a locked Claude companion directory and a stale Codex rollout, `clean --dry-run` printed a withheld Claude action but returned 0.  The partial-scope inventory emitted `knowledge_confidence: observed`, not LOW/UNKNOWN. | FAIL |
| SI-002 / SI-004 | A custom or low-confidence provider root cannot gain destructive authority. | A root supplied by `CLAUDE_CONFIG_DIR`, with only a `projects/` marker, was labelled default/eligible and its stale session was deleted by `clean --yes`. | FAIL |

## Adversarial cases

- Custom `CLAUDE_CONFIG_DIR` with one stale synthetic UUID session: Rust deleted it; the Python
  reference withheld it with exit 4.
- Pre-existing `settings.json.cancellai-tmp` symlink to an outside synthetic file: `configure`
  wrote the outside target and installed the symlink as `settings.json`.
- Malformed settings: `configure` silently discarded the invalid content and exited 0.
- Partial Claude scan plus eligible Codex artifact: dry-run exited 0 despite an explicit safety
  withholding.

## Differential / compatibility evidence

- `python3 scripts/rust_python_parity.py self-test` and `check` pass for the ten current
  NORMATIVE fixtures, but the checker fails its own contract: arbitrary uncited text suppresses
  an allow-listed divergence, and it compares only candidate UUIDs plus one withheld bit.
- Rust formatting, Clippy, check, workspace test, and cargo-deny gates pass locally on macOS.
- The required real tier-1 macOS/Linux/Windows CI evidence is absent.  G1, G2, G3, and G4 are
  each recorded as not ready in `docs/development/RELEASE_GATES.md`.

## Known residual risks

- No Rust cutover is safe while the custom-root, configuration-write, partial-state, and parity
  failures above remain.
- No packaged installer, CLI performance budget, crash/recovery proof, or actual tier-1 CI
  matrix evidence exists; those cannot be treated as a stable release gate pass.
- The current plan assigns some prerequisites to E07/E17 while those epics depend on E06,
  creating a control-plane prerequisite cycle that must be resolved before a canonical switch.

## Rollback / recovery

Rust remains non-canonical.  Stop invoking the source-built `cancellai-cli` beta and continue
using the tagged Python `cancellai` reference; no cancellAI-owned local-state migration exists
to undo.  Repair the failed paths before re-opening the cutover gate.

## Owner decision

`REJECT`

Owner note: independent verifier recommendation.  Acceptance requires a later passing Safety
Verdict and explicit owner decision after the failed findings and G1--G4 gaps are closed.

## Round - 2026-09-24

Verifier: Codex
Brief-Checksum: 76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642

This is E06-S04's first independent pass over the actual canonical-switch implementation, separate from the 2026-09-01 pre-cutover verdict. Reviewed tree: `2531746`, with the verifier brief uncommitted on purpose. The full counterexamples and eleven-axis analysis are in `project/evidence/E06-VERIFIER-REVIEW-ROUND6.md`.

| Invariant / obligation | Adversarial evidence | Result |
| --- | --- | --- |
| SI-019 and C-16, release may authorize only the verified engine | `release.py check` returned no problem for cutover formula resources pointing to v1.20.0 with corrupted digests while the source/formula tag said v1.21.0. An extra malformed engine resource also passed `point_engine_resources`. | FAIL |
| C-17, safe transition and rollback | A temporary live-formula copy switched to Rust when `finalize('1.21.0')` was repeated for an existing Python release. A forced post-write consistency error left that copy changed. The current live formula is still Python and the template does provide `cancellai-legacy`. | FAIL |
| M8 G1-G4 and version identity | The published v1.21.0 arm64 engine archive ran as `cancellai-cli 0.1.0`; `tests.yml` invokes version without asserting it, while the template's own test expects 1.21.0. Earlier G1-G4 stories are done and owner-accepted, but this switch lacks a reliable artifact/version smoke assertion. | FAIL |
| Release-note acceptance criterion | Compared Python and Rust command surfaces and `docs/CLI_RUST.md`: Rust `configure` refuses Windows, while Unreleased says Windows supported without that exception; canonical help/version still identifies a beta `cancellai-cli`. | FAIL |

### Adversarial cases

- Temporarily substituted a wrong-version, wrong-digest formula into `release.check`; result `[]`.
- Injected an extra malformed engine resource; `point_engine_resources` accepted it.
- Ran `finalize('1.21.0')` against a temporary formula copy; it adopted the Rust template. Forced the post-write check to fail; changed bytes remained.
- Downloaded and executed the published v1.21.0 arm64 archive: `cancellai-cli 0.1.0`; its archive SHA-256 matched the published sidecar.
- Rendered the cutover formula and checked Ruby syntax, `brew style`, and `brew audit --strict` on the byte-identical throwaway tap. No Homebrew installation occurred on this host.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests, stable-channel CLI with kill-points | PASS locally |
| Python pytest | PASS: 705 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `check_process.py check` | PASS at baseline; release check has the demonstrated false negative |
| `verifier_handoff.py check` | FAIL at baseline because this old verdict lacked the new brief checksum; post-append check is recorded in the round review |
| Native Linux/Windows cutover install | NOT RUN: no native host in this verifier workspace |
| Latest main CI | PASS: governance, tests, rust, and CodeQL for `2531746`; the counterexamples remain untested by those workflows |

### Rollback / recovery

Do not adopt the template yet. The live formula still installs the Python `cancellai`, and the proposed template's `cancellai-legacy` installs the tagged Python reference. Repair F-01 through F-04 and rerun the release gate before asking the owner to accept migration.

### Owner decision

PENDING

FAIL
