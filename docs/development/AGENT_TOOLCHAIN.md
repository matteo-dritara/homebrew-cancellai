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

The instrument is [SkillSpector](https://github.com/NVIDIA/SkillSpector), pinned at 2.11.2 and
installed as a development dependency - **not** as a component in the manifest it is checking:

```sh
uv tool install 'git+https://github.com/NVIDIA/SkillSpector@v2.11.2'
```

It runs with `--no-llm`, so no file content leaves the machine and the semantic analysers do not
run. That is a real reduction in reach and the gate prints it on every run rather than letting a
static-only pass read as a full one.

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
