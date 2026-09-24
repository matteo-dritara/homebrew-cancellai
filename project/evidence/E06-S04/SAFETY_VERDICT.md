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

## Round 7

Verifier: Codex
Brief-Checksum: 76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642

Second and final owner-authorized independent pass, over repair commit `23bb14e` and round-count metadata `bfce0cd`. The full reproduction and eleven-axis audit are in `project/evidence/E06-VERIFIER-REVIEW-ROUND7.md`.

| Invariant / obligation | Adversarial evidence | Result |
| --- | --- | --- |
| SI-019, C-16, release identity | `release.check()` accepted a synthetic v2.0.0 formula with three fabricated all-zero engine digests; it also accepted arm64/Intel archive URLs exchanged between their platform blocks. | FAIL |
| C-17 and M8, controlled cutover | An isolated v2.0.0 `finalize` succeeded with `adopt_cutover=False` and no Rust engine; an explicit adoption silently replaced a partly edited live formula. No owner-accepted verdict is consulted, and the runbook still documents implicit adoption. | FAIL |
| G1-G4 packaged engine/version proof | The v1.21.0 cutover job accepts `cancellai-cli 0.1.0` and skips `brew test`. `verify-installed` accepted a synthetic legacy command reporting `12.0.0` for expected `2.0.0`. | FAIL |
| Release-note acceptance criterion | Windows `configure` and engine naming are now disclosed, but Rust-only `status --allow-running` and `version --source` are absent from the claimed full CLI inventory and Unreleased notes. The canonical CLI document still says no package manager distributes the engine. | FAIL |
| Rollback availability | The template installs tagged `cancellai.py` as `cancellai-legacy` with Python dependency; live formula still installs Python as `cancellai`. | PASS as a proposed mechanism |

### Adversarial cases

- Substituted a v2.0.0 formula with correct URLs but all-zero 64-digit resource digests into an isolated `release.check` call: result `[]`.
- Swapped arm64 and Intel archive URLs between Homebrew platform blocks: `formula_engine_problems` returned `[]`.
- Reproduced v2.0.0 finalize without adoption, and template adoption over a malformed existing engine block, using temporary formula files; both succeeded.
- Compared Python and Rust help for `status`, `clean`, `configure` and `version`; found two undocumented Rust-only flags.
- Rendered the v1.21.0 cutover formula; Ruby syntax passed. No Homebrew installation, tag, push or publication occurred.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests, stable-channel CLI with `kill-points,test-curl` | PASS locally |
| Python pytest | PASS: 714 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check` | PASS at baseline; release check has reproduced false negatives |
| v1.21.0 render and Ruby syntax | PASS; Homebrew style/audit NOT RUN in round 7 (passed on a byte-identical render in round 6) |
| Native Linux/Windows cutover install | NOT RUN: no native host here |
| Latest main CI | All four workflows PASS for both `23bb14e` and `bfce0cd`; reproduced counterexamples are outside those gates |

### Owner decision

PENDING

FAIL

## Round 8

Verifier: Codex
Brief-Checksum: 76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642

Owner-authorized independent pass over `351295d`. Full findings and eleven-axis audit: `project/evidence/E06-VERIFIER-REVIEW-ROUND8.md`.

| Invariant / obligation | Adversarial evidence | Result |
| --- | --- | --- |
| SI-019, C-16, release identity | A 2.0.0 formula without engine resources passed `release.check`; an arm resource outside its CPU block also passed; source digest and archive bytes are not independently checked against the manifest. | FAIL |
| M8, controlled formula adoption | `finalize` adopted a temporary template whose engine installation line was removed and its final `check()` passed. Candidate validation remains after atomic replacement. Story `done` status, v1.21.0 guard, partial-marker refusal and a normal same-version retry were confirmed. | FAIL |
| G1-G4, installed version | Exact legacy version spoof was refused. CI now builds a stable-channel candidate, checks exact installed versions and invokes `brew test`; no published 2.0.0 artifact exists yet. | PASS within tested scope |
| Release-note contract | Python refuses `--days 0` on status and clean; Rust accepts it and proposed a Delete in a synthetic plan. The inventory calls the flag `same` and Unreleased omits the changed range. | FAIL |
| Rollback | The current live formula is Python-only; template carries `cancellai-legacy` through 2.1.0, but candidate adoption does not validate the mapping. | CONDITIONAL |

### Adversarial cases

- Isolated `formula_engine_problems` and `release.check` probes accepted a 2.0.0 Python-only formula, an engine resource after `on_arm`'s closing `end`, a changed source SHA, and a formula without an engine installation command.
- Isolated `finalize("2.0.0", adopt_cutover=True)` with a modified template succeeded; its post-write check returned no problems. Repeating finalize without adoption preserved identical bytes. The live formula was not edited.
- Python `status --days 0` and `clean --days 0 --dry-run` exited 2. A stable-channel Rust `status --days 0` exited 0; `plan --days 0 --keep-latest 0` proposed one Delete for an old synthetic session. No real user data was touched.
- v1.21.0 rendered formula passed `ruby -c`. No Homebrew installation, tag, push or publication occurred.

### Gates

| Gate | Result |
| --- | --- |
| Rust fmt, workspace Clippy, Windows-target Clippy, workspace tests, stable-channel CLI with `kill-points,test-curl` | PASS locally |
| Python pytest | PASS: 719 passed, 3 skipped |
| `release.py check`, `project_os.py check`, `verifier_handoff.py check`, `check_process.py check` | PASS at baseline; release check has reproduced false negatives |
| Render v1.21.0 and Ruby syntax | PASS; brew style/audit NOT RUN in round 8 |
| Native 2.0.0 Homebrew, Linux and Windows install | NOT RUN: no published candidate or native hosts here |
| Post-evidence governance gates | Recorded in the round 8 epic review |

### Owner decision

PENDING

FAIL

## Round 9 — 2026-09-24

Verifier: Codex
Brief-Checksum: 76f63bf3b664b057b00bc82b00b7377d819011f07f9aa96bda0d4b81cedcc642
Review target: `351295d..7e35a2a`

| Invariant / obligation | Independent evidence | Result |
| --- | --- | --- |
| SI-019, C-16, release identity | A generated 2.0.0 formula passed both shape and published-digest checks when its engine SHA matched synthetic sidecars but differed from the engine archive bytes; the verifier probe made zero archive requests. The release manifest was not consulted. | FAIL |
| Cutover control and rollback | The live formula remains Python-only; generated 2.0.0 formula retains `cancellai-legacy`. The owner migration Safety Verdict is pending, so AC1 is not met. | INCOMPLETE |
| G1/G2/G3/G4 local gates | Host Rust fmt/Clippy/check/tests, Windows-target Clippy, parity and governance gates passed. Native 2.0.0 artifact installation and per-platform partial-scan reproduction were not observed; CI status was unavailable. | INCOMPLETE |

### Adversarial case and exact repair

`published_digest_problems` returned no problem when the formula and `.sha256` sidecar both named `aa…aa` for all three engine assets while the simulated archive bytes hashed to `0d12c9f388630a692893dc73f48aa392bed36bef50b822c3c3c067198d337a8b`. Download and hash each engine asset, verify its version/target and manifest identity, and compare all digests before formula acceptance. The full reproduction and gate results are in `project/evidence/E06-VERIFIER-REVIEW-ROUND9.md`.

### Known residuals, rollback and owner decision

Existing Unix leaf-name and late-link residuals remain recorded from prior rounds. The Python source remains tag-archiveable and the proposed engine formula installs it as `cancellai-legacy`; no cutover release was made. Owner acceptance of the migration Safety Verdict: PENDING. This round rejects the story and does not accept a new residual.

FAIL

## Round 14 — 2026-09-24

Verifier: Codex
Brief-Checksum: b48e620110482c92c727a7f8bad34338f6b6b7fbe228714ce16b6df83afe97e9
Review target: `2ac50ebaeafae6a8e576f1ee43aaa99ada0c1c4c..1f7873fadf809ba6dae19c7058c27f006076e874`

### Safety surface and obligations

E06-S04 would authorize the Rust engine as canonical, including permanent deletion. SI-019 and C-16 require a verified single mutation boundary, independent CR4 evidence and an owner-visible Safety Verdict before that authority is released. `check_mutation_boundary.py check` passed locally, and the macOS parity comparator matched all 14 normative fixtures in both root-origin scenarios. These results do not cover the required native Windows E21-S02 partial-scan cases.

| Obligation | Independent evidence | Result |
| --- | --- | --- |
| SI-019 mutation boundary | Static gate inspected 121 Rust sources; the deletion primitive and capability remain confined to `cancellai-platform::mutation` and `cancellai-safety::mutation_executor`. Local stable-channel CLI and kill-harness tests passed. | PASS on tested host |
| Partial/unknown state must withhold destructive action | The 14-fixture differential gate passed locally; the workflow's parity matrix covers macOS/Linux only. Windows is tier 1 and advertised as supported, but its stable-channel CLI tests do not execute the E21-S02 partial-scan fixtures against the frozen reference. | NOT VERIFIED on Windows |
| Owner-visible cutover acceptance | `CUTOVER_AUTHORIZATION.md` is absent; `cutover_authorization_problems('2.0.0')` refuses. The owner has not accepted a passing migration verdict. | FAIL |

### Adversarial cases and compatibility

The comparator's self-test caught injected divergences; the actual local run matched 14 normative fixtures under default and custom roots. The counterexample to the *verification claim* is the Windows CI path: it runs a different set of CLI tests, with the relevant permission-failure tests Unix-gated, so a Windows partial-scan regression in those fixture branches would not be caught by the named gate. This does not establish an unsafe Windows deletion. The live v1.21.0 formula still installs Python; the proposed cutover formula retains `cancellai-legacy` through 2.1.0. The Unreleased notes disclose the intended CLI differences.

### Required repair, residual risk and recovery

Run native Windows E21-S02 partial-scan cases in both root-origin scenarios with withheld actions and exit status asserted against the frozen reference's expected behavior, or obtain an owner-approved ADR that excludes Windows from the initial cutover and corrects the support claims. Then seek a new independent migration PASS and the owner's version-and-hash-bound authorization. The local docs/pytest gate failure comes from an untracked harness prompt; the Linux cross-target Clippy command lacks a cross C compiler; these environmental gaps are recorded in the round review and are not treated as passes. Until the criteria are met, keep the Python formula live and do not adopt the Rust cutover.

### Owner decision

PENDING

FAIL

## Round 15 — 2026-09-25

Verifier: Codex
Brief-Checksum: 41346037461d07abb0cb2d459581a90b1a2015696b662e239eacd4529e6f0dbb
Review target: `89e319b..e54a63994e70d2ad235b3cf7dad829f21859f996`

### Safety surface and invariant

The proposed 2.0.0 Homebrew switch would install the Rust engine as `cancellai`, which can permanently delete provider files. SI-019 requires that mutation remains inside the independently verified safety boundary. `check_mutation_boundary.py check` passed on 121 Rust source files. The finite cutover checklist is part of the evidence gate for releasing that authority; this round found its C3 claim unproved. Full reproduction and gate results are in `project/evidence/E06-VERIFIER-REVIEW-ROUND15.md`.

| Obligation | Independent evidence | Result |
| --- | --- | --- |
| SI-019, one mutation boundary | Static boundary gate passed; no new mutation caller appeared in this review diff. | PASS within examined source |
| C1, C2, C4-C8 | Dependencies closed; real rehearsal release and asset bytes checked, with authenticated provenance rerun unavailable locally; native partial-scan CI jobs passed; transition, notes and rollback documented; unauthorized finalize refuses. | PASS WITH THE RECORDED S16 RESIDUAL |
| C3, 2.0.0 candidate installation | The passing Homebrew job derives `version` from `cancellai.py`, which is 1.21.1 at the reviewed commit. Its `render-local-cutover` and `brew test` exercise a 1.21.1 formula, not the specified 2.0.0 formula. | FAIL |

### Adversarial reproduction and exact repair

`python3 cancellai.py version` printed `1.21.1`; `render_formula` with the CI job's version and local-archive inputs emitted `version "1.21.1"`. The successful GitHub `tests` job on `e54a639` therefore cannot prove the 2.0.0 installation claim. Build and install a version-aligned 2.0.0 source and engine candidate in that job and rerun its version checks and `brew test` on the reviewed commit; if the intended claim is only the engine-formula shape at 1.21.1, the owner must change C3 explicitly. Until repaired, keep the Python formula live. This finding does not allege an unsafe deletion.

### Compatibility, residual risk, rollback, owner decision

The v1.21.1 tag still archives `cancellai.py`; the proposed engine formula keeps it as `cancellai-legacy` through 2.1.0. `docs/RELEASING.md` documents signed containment, legacy invocation and formula revert. The reviewer's local `gh` lacks authentication, so the real release's provenance step could not be rerun here; public release CI and independently checked archive bytes are recorded in the round record. There is no `CUTOVER_AUTHORIZATION.md`, and `cutover_authorization_problems('2.0.0')` refuses. Owner acceptance of a migration Safety Verdict: PENDING.

FAIL
