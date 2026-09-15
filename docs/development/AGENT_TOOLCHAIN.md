# Agent Toolchain

The agent toolchain is a dependency. It is governed like one.

Skills, hooks, subagents, slash commands, plugins, MCP servers, language servers, output styles
and status lines are not configuration. They are third-party code and third-party *prompt content*
entering the agent that writes this repository's code. They arrive by a single command, with no
review, no pin, no expiry and no record of why. Every other dependency here is governed -
`cargo deny` for crates, `project/provider_trust.json` for provider manifests, `dependabot` for
updates - and until E26 this class was governed by nobody.

## The manifest

[`project/agent_toolchain.json`](../../project/agent_toolchain.json) is the source of truth.
`scripts/check_agent_toolchain.py` enforces it, and the `toolchain` skill turns it into a
session-start review.

Each component records: `id`, `kind`, `scope`, `source`, pinned `version`, `trust`,
`capabilities`, `always_on_tokens`, a `purpose`, and a `decision` with a decider, a date and a
rationale. `rejected` records what was considered and refused, with the reason - a rejection
nobody can revisit is folklore, and without the record the same candidate is re-evaluated every
few months.

## The four rules

**Nothing unmanaged.** A component present in the repository and absent from the manifest fails
the gate. That is the whole supply-chain control: an addition becomes visible, and a visible
addition gets a decision.

**Capability sets the trust bar.** A component whose capabilities include `executes-code`,
`network`, `credentials` or `writes-files` must be `FirstParty` or `Vendor`. This is not a
judgement about community work; it is that "it was convenient" is not a reason to give an
unreviewed third party a shell on the machine that holds this repository. A component that is only
prompt text may be `Community` - it can be wrong, but it cannot execute.

**Decisions expire.** A decision older than `review_cadence_days` is reported until it is renewed
or the component is retired. A toolchain that is never re-reviewed becomes a list of things nobody
chose and nobody can now justify removing.

**Context is budgeted.** Every always-on component costs tokens in every session, forever, before
any work begins. [C-11](../CONSTITUTION.md) says a storage-governance tool may not become an
unbounded storage producer. By the same argument an agent toolchain may not become an unbounded
context producer, so the manifest carries `context_budget_tokens` and exceeding it fails.

## Scope, and the limit of what can be verified

`project` scope is versioned in this repository and enforced strictly: the checker sees it, and it
is present in CI.

`user` scope - a plugin installed into `~/.claude` - is declared intent. It is not present in CI
and the checker says so rather than pretending to verify it. Claiming to check what cannot be seen
would be worse than stating the boundary.

## What an agent may not do

**Install, update or remove a component.** Installing executes third-party code and changes what
every future session is told; that is an owner decision. The `toolchain` skill produces a proposal
and stops. If the owner approves, the manifest entry lands in the **same change** as the
installation - a component installed without one fails `check` on the next commit, which is the
intended outcome rather than an inconvenience.

## Adding something

Only against a **named gap** - a task this project repeatedly does by hand that a component would
do reliably, traceable to a story. Browsing for tools produces tools; it does not produce
capability.

Judge a candidate on capability surface, trust tier, always-on cost, and whether it introduces a
second source of truth for something this repository already defines. That last one refuses
otherwise excellent packages: two definitions of Done in one repository is the defect class the
E00 review took three rounds to close. Popularity is not one of the four.

## What a component contains

The manifest answers a question about provenance - was this declared, and is its trust tier high
enough for what it claims? That is the right question and only half of it. A skill is prompt
content injected into the agent that writes a tool which deletes files, and prompt content can
carry an instruction to ignore a rule, a path that exfiltrates a key, or a tool permission far
wider than its purpose. None of those change a trust tier or a pinned version. `cargo deny` does
not ask who published a crate; it reads the crate. `scripts/check_skill_content.py` does the same
for the pack (E28-S01).

```sh
python3 scripts/check_skill_content.py check    # fails at HIGH or above
python3 scripts/check_skill_content.py report   # every finding, with the fingerprint a waiver names
```

The instrument is [SkillSpector](https://github.com/NVIDIA/SkillSpector), pinned at 2.11.2 in
`requirements-dev.txt` as a development dependency - **not** as a component in the manifest it is
checking. The documented setup installs it, so a fresh clone can run every gate after one command:

```sh
pip install -r requirements-dev.txt
```

The requirement carries a `python_version >= "3.12"` marker, which is not cosmetic: the scanner
declares `requires-python = ">=3.12,<3.15"` and this repository's test matrix still exercises 3.10,
where an unconditional requirement makes the whole development install fail. Below 3.12 `pip` skips
it and the gate refuses with the reason, rather than passing as though nothing were missing.

It was briefly a separate install step in two workflows and a line in this document, which is a
step somebody does not take: the gate then passes for everyone who already has the tool and refuses
for everyone who followed the instructions. `tests/test_dev_environment.py` asserts that the
requirement and the version the gate pins do not drift apart, because the failure mode of that
drift is a repository that only works on the machine it was built on.

It runs with `--no-llm`, so no file content leaves the machine and the semantic analysers do not
run. That is a real reduction in reach: static-only analysis remains explicitly `partial`, but the
gate refuses if SkillSpector omitted a carried skill or entirely skipped a file rather than letting
that absence read as a clean result.

**Three things the gate deliberately does not read**, each measured rather than assumed:

- **the exit code.** A skill planted with SSH-key exfiltration and `eval "$(curl ...)"` produced
  two HIGH findings and still exited 0.
- **`risk_recommendation`.** On that same planted skill it read `SAFE`, while this repository's
  own clean pack read `CAUTION`. The per-skill `issues` are correct; the rolled-up field is not
  something to gate on.
- **`finding_id`.** It is regenerated per run - two consecutive scans of an unchanged pack
  produced different ones. Waivers key on `match_fingerprint`, which was stable.

A waiver lives in `project/skill_content_waivers.json`, names one `match_fingerprint`, and carries
a date and an argument. "It is a false positive" is not an argument; why it is one is. A waiver
that stops matching is reported as **stale** rather than dropped, because a drifted fingerprint
means the finding is back and nobody was told.

The pack's one standing waiver is worth reading as a caution about scanners generally: the
`adversarial-cases` skill was flagged for session persistence because it contains the words
`~/.claude` and `~/.codex` - inside the sentence forbidding anyone to touch them. The rule matched
the prohibition as though it were the act.

## The manifest's own numbers

Two fields in the manifest used to be assertions with nothing behind them, which is the same
defect the content scan above removes from `trust` (E28-S04).

**`always_on_tokens` is now measured.** The budget is what decides whether this project can carry
a component - it is the field that refused Task Observer at 14,460 tokens against a 6,000 ceiling -
and it summed numbers a person typed. `check` now measures a project-scope component's real
contribution from the YAML frontmatter of its skills, which is what actually sits in every session,
and refuses a declaration more than 25% away from it. The tolerance is wide on purpose: the
characters-per-token conversion is an approximation and the gate is not a tokenizer, it is looking
for an entry wrong by an order of magnitude. A user-scope component cannot be measured from this
repository at all, and is reported as **unmeasured** rather than confirmed. A project-scope
component that carries prompt content but cannot be measured is different: its files should be
present here, so the gate refuses rather than recasting a broken measurement as uncertainty.

The first run answered the question the story asked. `cancellai-skill-pack` declared 900 and
measures 964 - 6.6% out, inside the tolerance. Hand-entry was reliable here; now nobody has to
assume it stays so.

**`license` now exists.** `rust/deny.toml` has kept a licence allow-list for crates since ADR-0015
and nothing governed the prompt content carried into the agent. The allow-list lives in the
manifest as `license_allowlist` and is deliberately **not** a copy of the crate list: those govern
linked code, these govern text the agent reads and patterns this repository may derive from. It
admits CC-BY-SA-4.0 because `trailofbits/skills` is carried under it, and AGENTS.md records
harvesting patterns from external packs - share-alike is a condition worth naming rather than
discovering.

A source with **no licence** is its own refusal, distinct from a licence not yet recorded: one is a
fact about the upstream, the other is work not done here. The 2026-09-15 census made that concrete -
`anthropics/skills` at 176k stars, `vercel-labs/agent-skills` and `hamelsmu/claude-review-loop` all
carry no licence file, so by default all rights are reserved.

## Three things the manifest now binds to their evidence

**Waivers are bound to the scanner that produced them** (E29-S02). A waiver names a
`match_fingerprint`, which was measured stable across two runs of one scanner version and is
untested across two versions. `project/skill_content_waivers.json` records the version it was
written against and the date somebody last revalidated it; the gate refuses when that differs from
the pin. Bumping the pin is routine and used to carry no obligation to revisit the waivers, which
is exactly when a drifted fingerprint would start suppressing a different finding in silence.

**A cost CI cannot observe can be measured where it is installed** (E29-S05). Ten of twelve
components are user-scope. A `measured` block - `tokens`, `component_version`, `taken_on` - records
a measurement taken on a machine where the component exists, and it is read as **stale** the moment
the component's version moves on. It is never required: CI passes with none recorded.

**A licence names the revision it was read from** (E29-S06). A licence is the upstream's
declaration at one moment, and nothing re-read it, so an upstream that relicenses leaves the
manifest asserting something that was true and is not. `license_source_revision` is what makes a
later comparison possible at all; E26-S02 already reaches these upstreams for versions and
abandonment and degrades truthfully without a network, which is where the comparison belongs.
