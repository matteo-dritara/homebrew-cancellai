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
