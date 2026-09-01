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
