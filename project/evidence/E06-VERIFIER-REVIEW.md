# E06 Independent Verifier Review - Round 1

- Review target: `4024ab8..16a44b0`
- Verifier: Codex (`/root`), independent verifier
- Date: 2026-09-01
- Epic: E06 - Rust CLI Parity and Cutover

All four E06 stories were `ready_for_review` before this review began.  This review used the
story contracts, architecture/security requirements, final diff, and independently constructed
counterexamples; executor rationale was not treated as evidence.

## Per-story verdicts

| Story | Verdict | Concrete evidence |
| --- | --- | --- |
| E06-S01 | FAIL | A stale session beneath a root supplied through `CLAUDE_CONFIG_DIR` is reported as `origin=default`, `confidence=default`, then permanently deleted by `clean --yes`.  The Python reference refuses every custom root, including a low-confidence root containing only `projects/`, with exit 4.  `configure` follows its predictable `settings.json.cancellai-tmp` symlink and overwrites a file outside the configured root.  A malformed `settings.json` is silently replaced.  A mixed partial Claude scan plus a clean Codex candidate makes `clean --dry-run` exit 0 while a provider is withheld; the partial-scope inventory emits `knowledge_confidence: observed`, exceeding JSON_CONTRACTS' `LOW/UNKNOWN` ceiling. |
| E06-S02 | FAIL | The supposedly ADR/RFC-cited allow-list accepts arbitrary text: setting `INTENTIONAL_DIVERGENCES = {"fx": "uncited free text"}` makes `_compare_results("fx", ..., ({"a"}, False), ({"b"}, True))` return `[]`.  The actual comparator observes only delete-UUID sets and one withheld bit.  It cannot detect the normative corpus' root-confidence/origin, protected/unknown coverage, or other classified artifact semantics; `codex-layout-drift`'s normative coverage assertion is one concrete blind spot. |
| E06-S03 | PASS_WITH_RESIDUALS | `cargo test -p cancellai-cli` independently passes the five side-by-side tests: `version` identifies `cancellai-cli` and its concrete version, read-only commands leave synthetic home unchanged, real clean changes only its selected artifact, and invocation is CWD-independent.  Neither engine has cancellAI-owned persisted state today.  Residual: this is a source-built beta smoke, not a packaged installer/upgrade/uninstall test; E17 owns the release factory.  It is blocked from closure this round because E06-S02 failed. |
| E06-S04 | FAIL | The submitted gate assessment is correct that cutover is not ready, but that means AC1 (an accepted owner-visible migration Safety Verdict) is not met.  G1--G4 remain explicitly not ready; the S01/S02 failures additionally invalidate the claimed CLI parity and SI-019 posture.  Python remains available in tag `v1.6.0`, and no canonical-engine switch was made.  See `project/evidence/E06-S04/SAFETY_VERDICT.md`. |

## Required repairs

### E06-S01

Reproduction, on a new synthetic tree, was:

```text
CLAUDE_CONFIG_DIR=<custom-root> cancellai-cli clean --tool claude --keep-latest 0 \
  --allow-running --yes --json
```

where `<custom-root>/projects/project-a/<uuid>.jsonl` was stale and was its only provider
marker.  The command emitted `succeeded` and removed the file.  The retry also pre-created
`<claude-root>/settings.json.cancellai-tmp` as a symlink to `<outside-file>`; `configure
--claude-retention 30` replaced the outside file and left `settings.json` as that symlink.

Required repair:

1. Determine default-versus-custom roots from their resolved paths, retain the provider
fingerprint's actual origin/confidence in classification, and independently refuse mutation of
custom/unverified roots at execution.  Do not hard-code `is_default_root = true` or a verified
`RootFingerprint` when sealing a plan.  Add low/high/unknown custom-root and absent-home
regressions.
2. Treat `configure` as a safety-relevant provider mutation: reject custom roots, malformed or
non-object settings, and links; use a unique no-follow temporary file and an atomic durable
replace that cannot write outside the approved root.  Extend boundary governance or add a
dedicated configuration-write boundary test so this cannot be omitted from SI-019 checks.
3. Propagate partial/unknown scan completeness to every emitted artifact's confidence, and make
both dry-run and execution return exit 4 whenever requested work is withheld for incomplete
state.  Strictly reject invalid arguments for every command.
4. Revalidate the recorded `process_not_running` precondition immediately before deletion; the
current sealed plan revalidates only filesystem identity.

This violates E06-S01 AC2/AC3, SI-007, SI-008/SI-009/SI-014, SI-019, C-02, C-03, C-06, and C-07.
The irreversible production deletion/configuration surface also needs a reviewed CR-level
assessment rather than relying on its current CR3 label.

### E06-S02

Required repair: make every allow-list entry validate a real accepted ADR/RFC identifier and
fail if it does not.  Compare a canonical semantic projection for each NORMATIVE fixture,
including root authority, scan completeness, protected/unknown coverage, discovered identity
records, and proposed actions, not only deletion UUIDs and one Boolean.  Add injected-divergence
tests for each projected field and a real custom-root fixture outside the characterization
helper's default-root patch.  This violates E06-S02 AC1 and AC2, and M6's requirement that an
unexplained semantic divergence blocks cutover.

### E06-S04

Required repair: keep Rust non-canonical; first close E06-S01/S02 and rerun their independent
verification.  Then supply real G1--G4 evidence, a passing independent CR4 Safety Verdict, and
owner acceptance before any switch.  The control plane must also resolve the prerequisite cycle:
the current checklist requires tier-1/install evidence assigned to E07/E17, while E07/E17 depend
on E06.  This violates E06-S04 AC1 and SI-019's CR4 evidence-gating requirement.

## Gate status

| Command | Result |
| --- | --- |
| `cargo test -p cancellai-cli` | PASS (re-run after retry) |
| `cargo fmt --check` | PASS |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `cargo test --workspace` | PASS |
| `cargo deny check` | PASS (three existing unmatched-license-allowance warnings; required advisory-cache access granted) |
| `.venv/bin/python -m pytest tests -v` | PASS: 179 tests, 22 subtests |
| `.venv/bin/python -m ruff check .` / `ruff format --check .` | PASS |
| `.venv/bin/python -m mypy` over every required target including `rust_python_parity.py` | PASS |
| Generated docs, project OS, docs, workflows, fixtures, schemas, characterization, differential harness, Rust workspace, mutation boundary, provider compatibility, process, and release checks | PASS |
| `python3 scripts/rust_python_parity.py self-test` / `check` | PASS: comparator self-test; 10 current NORMATIVE fixtures |
| Adversarial custom-root deletion, temporary-symlink configuration escape, malformed settings, partial-scan exit/schema, and uncited allow-list probes | FAIL as described above |

The system Python 3.14 lacks pytest, Ruff, and mypy; the pinned repository `.venv` supplied the
successful Python-tool gates.  A static mutation-boundary pass is not evidence that the new
configuration writer observes SI-019: it does not scan `std::fs::write`, `rename`, or
`create_dir_all`.

## Overall verdict

**FAIL — round 1 of at most 2.** E06-S01 returns to `in_progress`; E06-S02, E06-S03, and
E06-S04 are blocked by the failed dependency chain.  The epic remains `in_progress` with one
review round remaining.  No cutover, release, or owner Safety Verdict acceptance is recommended.
