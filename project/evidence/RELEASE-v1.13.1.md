# Release Evidence - v1.13.1

## Source

- Tag: `v1.13.1`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: 2026-09-13

## Included work

This release closes no epic. It exists because an already tagged version could not
carry the fix below, and a published tag is immutable history:

v1.13.0's release workflow failed at verify-rust on all three platforms and published nothing; v1.12.0's failed while packaging the Windows artifact and published nothing either. Both tags stand as history and cannot be re-cut. This version carries the fixes that make the pipeline run to completion: the clippy denial that stopped verify-rust (E06-S05), the shell declaration for the step that killed the Windows leg (E22-S08), and the two halves of an MSRV break that had left the workspace unable to compile on its own promised minimum toolchain since 2026-09-09 (E09-S05, E16-S07).

## Gates

Re-run at the tag by `.github/workflows/release.yml`; run locally before tagging:

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py \
  scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py scripts/release.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

- G1 Functional: PASS
- G2 Safety: PASS
- G3 Compatibility: PASS
- G4 Operability: PASS

## Compatibility

- Platforms: macOS. Python 3.10 and 3.14 exercised in CI.
- Providers/capabilities: Codex CLI and Claude Code, layouts observed at release time.
  Unclassified entries are reported by `status --coverage` and never cleaned.
- State/schema migrations: none. The tool keeps no persistent state.

## Supply chain

- Checksums: the Homebrew formula records the SHA-256 of the tag archive, written by `scripts/release.py finalize`.
- SBOM: not produced at this stage. The shipped tool has no runtime dependencies; development tooling is pinned in `requirements-dev.txt`.
- Provenance/attestation: deferred to E17.
- Signature verification: deferred to E17.
- Release manifest: this file.

## Install smoke tests

- Homebrew: `brew audit --strict` and `brew style` run in CI on every change; `brew install`/`brew test` exercise the tagged archive.
- direct shell / PowerShell / Linux packages: not applicable at this stage.

## Performance

- Scan benchmarks: none formalised; deferred to E10.
- Self-budget: recorded scan errors are bounded, and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

### Added

- **The engineering system is now measured and falsifiable, not only documented** (E25, E26; CR0-CR2,
  no user-visible product behavior change). Twelve findings from
  [the methodology review](../../docs/audits/2026-09-12-METHODOLOGY_REVIEW.md) against DO-178C, ISO 26262,
  IEC 61508, NPR 7150.2 and the software-measurement literature; eleven are now repaired.
  - `scripts/process_metrics.py` measures the process from artifacts the repository already commits:
    review yield per round, first-pass rejection rate split by reviewer independence, a
    Lincoln-Petersen residual estimate from reviewer overlap, evidence-ledger integrity, and
    documentation readership. The first result was that **47% of round-1 independent review verdicts
    are `FAIL`, on work whose executor had run every gate green**.
  - `scripts/check_risk_classification.py` gives the Change Risk Level a floor derived from the paths
    a change touches, enforced at `commit-msg` where the staged diff and the story are both visible.
    Four kernel surfaces are stated in code, where configuration may raise them and may never lower
    one. No safety standard lets the implementing party assign its own criticality level.
  - `scripts/gate_sensitivity.py` plants a violation of each named claim and records which gate
    caught it - Mills' error seeding, generalised from the two comparators already using it. Eleven
    mutants, all killed; the one that initially survived had edited the risk floor itself. A control
    pass runs every gate on an unmutated copy first, because a gate that fails on a clean export
    appears to kill everything it is pointed at, and a kill by such a gate is not counted.
  - `scripts/check_evidence.py` requires a row per acceptance criterion, a real residual-risk section
    at CR3 and above, and a Safety Verdict for a CR4 story at `done`.
  - `scripts/safety_oracle.py` checks protected-name enforcement, root capability and the retention
    rule against predicates written from the invariants rather than recorded from the implementation.
  - `scripts/check_ears.py` classifies acceptance criteria and requires a CR2+ story to say what
    happens when something is wrong. **13 of 366 criteria describe unwanted behaviour.**
  - `docs/security/HAZARD_ANALYSIS.md` adds an STPA pass over the mutation control loop: twenty
    unsafe control actions, five loss scenarios from process-model inconsistency, and three gaps the
    invariant set does not constrain.
- **The agent toolchain is governed as a dependency** (E26). `project/agent_toolchain.json` records
  every skill, hook, plugin, MCP server and language server with its source, pinned version, trust
  tier, capability surface, always-on token cost and a dated decision - plus what was rejected and
  why. Nothing unmanaged; a component that runs code must be first-party or a named vendor;
  decisions expire; and the always-on context cost is budgeted, by the same argument C-11 makes
  about storage. `scripts/check_agent_toolchain.py` also compares pins against upstream and records
  which components are actually used. **No agent installs, updates or removes a component.**

### Fixed

- The Rust quality gate is green against current stable clippy again (E06-S05, CR1). A descending
  sort in `cancellai-policy`'s atlas summary was written as a hand-rolled comparator, which the
  toolchain this workspace was developed against accepted and current stable denies - the v1.13.0
  release workflow failed at `verify-rust` on all three platforms because of it, and `publish` was
  correctly skipped. The ordering is unchanged. Recorded with the first use of the risk-floor
  override mechanism, since `cancellai-policy/src` carries a CR3 floor and this is presentation
  ordering downstream of every eligibility decision.

- The sensitivity harness plants each violation where the mutant aims it (E25-S12, CR2). Anchors are
  unique by construction: `except OSError` matched 26 sites in `cancellai.py` and only one is
  rewritten, so the SI-008 mutant had been editing a marker validator rather than the scan's
  completeness channel - and it therefore died on Python 3.13 and survived on 3.14, which is where
  CI found it. The report is now byte-identical on both, SI-008 is seeded in `Scan.record`, and a
  stale report prints the rows that differ instead of only saying that something moved.

- The MSRV promise holds again across a dependency update (E09-S05, CR1). Adding `ratatui` pulled in
  `instability` and `darling` releases that require rustc 1.88 while this workspace promises 1.85.0
  (ADR-0015), and the MSRV leg of `rust.yml` has failed on all three platforms on every push since -
  red on `main` across two release attempts, while every stable leg stayed green. The workspace now
  resolves with Cargo's MSRV-aware resolver (`resolver = "3"`), which will not select a version above
  the declared `rust-version`; the affected crates are locked to versions that support it. The Rust
  quality set passes unchanged.
- A release can carry a fix that closes no epic (E22-S07, CR3). The release contract could express
  only "this version closes an epic", so when the v1.12.0 and v1.13.0 workflows failed - a Windows
  packaging error, then a clippy denial - there was no way to cut the version carrying the fix, a
  published tag being immutable history. `release.py prepare` now takes `--fix <reason>` instead of
  `--epic`, and such a release must take the next patch number. Found while fixing it: a release was
  credited with covering an epic if the id appeared **anywhere** in the packet text, and a packet
  embeds the changelog - so PD-021's gate was satisfiable by a sentence. It now reads the declared
  `- Epic:` line. Four epics had been credited by prose; none was closed, so nothing had shipped
  wrongly.

- The risk-floor gate refuses a checkout it cannot reason over (E25-S14, CR2). It attributes
  committed changes to the story that declared them, and CI ran it against a depth-1 checkout: a
  shallow tip has no parent object, so `git log --name-only` reports the whole tree as its diff.
  All 539 tracked files were attributed to the last commit's story, and the gate refused a kernel
  crate's CR4 floor for a story that touched no Rust. It now refuses a shallow clone and names the
  checkout option that fixes it, a parentless commit attributes nothing, and the three CI jobs that
  run it fetch full history. The same defect pointing the other way is a silent pass, which is the
  version nobody would have found - and this is the second instance of the class, after the one
  that broke the v1.10.0 tag.
- The documents state the review rule that is actually in force (E25-S13, CR0). ADR-0025 replaced
  the two-round ceiling with a yield-based stopping rule, and three surfaces still said "at most
  twice": `AGENT_PROTOCOL.md` in two places, the `story-executor` skill, and PD-022 in the decision
  register, which is now superseded by PD-024. The skill is the one worth noting - the pack's rule
  is that a skill points at the contract and never restates it, and the single rule it restated is
  the one that drifted.

- The release workflow declares the shell for the step that broke v1.12.0 (E22-S08, CR1). That
  release packaged macOS and Linux, then its CycloneDX SBOM step - the one step in the job without
  `shell: bash` - ran under PowerShell on the Windows leg, where the backslashes continuing its
  command line are not continuations, and the release never published. A new workflow gate requires
  every multi-line `run:` in a job that can land on Windows to say which shell reads it. The gate's
  own first version reported clean: the step splitter was splitting on the matrix `include:` list
  rather than on `steps:`, so all four steps collapsed into one block and the missing declaration
  was masked by a sibling that had one.

- The knowledge-bundle verifier compiles on the promised toolchain again (E16-S07, CR4). Three
  `let`-chains written in E16-S02 are stable only from Rust 1.88, while the workspace promises
  1.85.0 (ADR-0015), so `cancellai-safety` has not compiled on the MSRV leg since 2026-09-09 -
  through two releases that failed to publish for other reasons that masked this one. Fixing the
  dependency side of the break removed the error that stopped cargo before it compiled anything,
  and this appeared underneath it. The conditions are rewritten with `is_some_and`, which the
  promised version supports, and the expiry boundary is now pinned in both places it appears.
  **This story stays at `ready_for_review`:** it is a CR4 change to the safety kernel, and the
  executor's own review cannot close one.

### Security

- **Open, low severity, not cleared:** `lru 0.12.5` is subject to
  [GHSA-rhfx-m35p-ff5j](https://github.com/advisories/GHSA-rhfx-m35p-ff5j) (`IterMut` violates
  Stacked Borrows), fixed in `lru 0.16.3`. It reaches this workspace through `ratatui 0.29`, which
  pins `lru ^0.12`; the version that takes the fix is `ratatui 0.30`, whose own minimum Rust is
  1.88.0 against this workspace's promised 1.85.0. It is an unsoundness report rather than a
  demonstrated exploit, and nothing here calls `lru::IterMut` - it is ratatui's internal render
  cache. `cargo deny check` does not see it: that gate reads the RustSec database and this advisory
  is in GitHub's, which is a narrower coverage than "advisories are checked" suggests.
  [ADR-0026](../../docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md) puts the choice - raise the
  minimum and clear it, or accept it explicitly - in front of the owner with the evidence. It is
  **proposed, not decided**; nothing in this release changes the minimum.

### Changed

- Review stops on **measured yield** rather than a fixed round count, and an epic that changes
  nothing in the shipped artifact closes as `done_no_release` instead of cutting an empty version
  ([ADR-0025](../../docs/adrs/0025-review-stops-on-yield-and-a-shippable-nothing-does-not-cut-a-release.md),
  amending ADR-0014).
- `docs/development/AGENT_PROTOCOL.md` states what "independent" can mean here in the standards' own
  vocabulary - one owner and two model instances is IEC 61508's lowest rung - and defines what a
  self-review may and may not certify.
- `docs/development/VERIFICATION_STRATEGY.md` separates **regression detectors** from **correctness
  oracles**: a fixture recorded from an implementation detects change, never wrongness.

No user-visible product behavior changes in E24, E25 or E26.

### Added

- The engineering contract is now loadable by the agent harness that is supposed to execute it
  (E24, CR0). `.claude/skills/` carries seven Agent Skills in the open
  [Agent Skills](https://agentskills.io) format - `orient`, `story-executor`, `epic-verifier`,
  `adversarial-cases`, `risk-gate`, `rust-kernel-guard`, `evidence-packet` - so the same pack
  loads for the executor and for the independent reviewer, which `AGENTS.md` assigns to a
  different agent. A skill is a runner over the contract and never a second copy of it;
  `scripts/check_agent_skills.py` enforces that by requiring every repository path and every
  `scripts/*.py` command a skill names to resolve.
- A `PreToolUse` hook refuses a hand-edit to a generated document at the moment it is attempted,
  returning the regeneration command, instead of letting CI discover it after the work is done.
  It matches on the real, project-relative, case-folded path - `docs/BACKLOG.md`,
  `docs/backlog.md` and `docs/adrs/../BACKLOG.md` are one file on APFS - and fails open on any
  input it cannot interpret. It covers the structured file-writing tools only; the CI drift
  check remains the authority.

### Changed

- `AGENTS.md` states **diff discipline** alongside story discipline: no unrelated reformatting,
  no removal of pre-existing dead code (a safety rule here, since code that looks unreachable may
  be a barrier whose reachability is what is in dispute), removal limited to what a change
  orphaned, and flagging rather than fixing what is found outside the story.

No user-visible product behavior changes from E24.

## Known residual risks

No epic closes here, so there is no closure packet to carry them from. The story-level records are
in `project/evidence/`, and these three are the ones a reader of this tag should know:

- **E16-S07 ships implemented and independently unverified.** It is a CR4 change to the safety
  kernel - three conditions in the knowledge-bundle verifier rewritten so they compile on the
  promised minimum toolchain - and the executor may not write its own CR4 Safety Verdict. The
  story stays at `ready_for_review`. Behaviour is pinned by 101 tests in `cancellai-safety`,
  including both directions of the expiry boundary at both sites, and the packet says plainly that
  no verdict exists.
- **GHSA-rhfx-m35p-ff5j is open** in `lru 0.12.5`, transitively through `ratatui 0.29`. Low
  severity, an unsoundness report rather than a demonstrated exploit, in a render cache this
  workspace does not call. It cannot be cleared without `ratatui 0.30`, which needs a higher MSRV
  than this project promises; [ADR-0026](../../docs/adrs/0026-raise-the-workspace-msrv-to-1-88.md)
  puts that choice in front of the owner and is **proposed, not decided**.
- **The Windows leg of `build-artifacts` is unproven past the SBOM step.** v1.12.0 died there, so
  nothing downstream of it has ever run on Windows. This tag is the first to find out.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
