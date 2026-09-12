---
name: toolchain
description: Review the agent toolchain - skills, hooks, subagents, plugins, MCP servers, language servers - against its manifest, check for updates and newly relevant tools, and put install / update / remove proposals to the owner. Use at the start of a session, when a component looks stale or unused, before adding any plugin, and when asked what tooling this project has or should have.
allowed-tools: Bash(python3 scripts/check_agent_toolchain.py:*), Bash(claude plugin:*), Bash(gh api:*), Read, Glob, Grep
---

# Toolchain

The agent toolchain is a dependency. It is third-party code and third-party *prompt content*
entering the agent that writes this repository, it arrives by one command with no review, and
before `project/agent_toolchain.json` existed nothing recorded what was carried or why.

This skill reviews it. **It never installs, updates or removes anything** - that executes third
party code on the owner's machine and changes what every future session is told. The output is a
proposal.

## Current state

!`python3 scripts/check_agent_toolchain.py report 2>&1`

!`python3 scripts/check_agent_toolchain.py check 2>&1 | tail -5`

Installed at user scope, which the checker can see only on a developer machine:

!`claude plugin list 2>/dev/null | grep -E '^  ❯' | sed 's/^  ❯ /    /' || echo "    (claude CLI unavailable here)"`

## What to do with it, in order

### 1. Reconcile

Anything installed and unmanaged is the finding that matters most: it entered without a decision.
Anything in the manifest and missing is either a broken environment or a retirement nobody
recorded. Resolve both before looking for new tools - a toolchain that does not match its manifest
cannot be reasoned about.

### 2. Renew what expired

A decision past the review cadence is not wrong, it is **unexamined**. For each one ask the two
questions that actually decide it:

- **Has it been used since the last review?** A component nobody invoked is paying always-on
  context in every session for nothing. Retiring it is the default; keeping it needs a reason.
- **Has its risk changed?** A new release that adds a hook or an MCP server changes the component
  from prompt content into software with a shell and a network. That is a new decision, not an
  update.

### 3. Check for drift and updates

For a `github:` source, compare the pinned version against the current release and read what
changed:

```sh
gh api repos/OWNER/NAME --jq '"\(.stargazers_count)★ pushed \(.pushed_at)"'
gh api repos/OWNER/NAME/releases/latest --jq '.tag_name'
```

An upstream that has not been pushed to in a long time is an abandonment signal and belongs in
the proposal as a *removal* candidate, not only as a stale pin. Never propose an update whose
changelog you have not read.

### 4. Look for what is missing, narrowly

Only against a **named gap**, never as a browse. A gap is a task this project repeatedly does by
hand that a component would do reliably - the open stories in `project/epics/*.json` are the
list. If nothing in the backlog names a gap, the answer is that nothing is missing, and that is a
complete answer.

When you do look, judge a candidate on four things before its popularity:

| | Why |
|---|---|
| **Capability surface** | Does it ship hooks, MCP servers or subagents, or only prompt text? A hook runs code; an MCP server reaches the network and may hold credentials. Popularity is not a substitute for either. |
| **Trust tier** | First-party, a named vendor, community, or unknown. Anything that runs code and is not first-party or vendor is refused by `check`, and rightly. |
| **Always-on cost** | Every component's description sits in every session forever. `claude plugin details <name>` reports it, and the manifest carries a budget. |
| **Second source of truth** | Does it redefine something this repository already defines - Done, risk, the plan? If so it is refused however good it is, because two definitions of Done is the defect class the E00 review took three rounds to close. |

Check `rejected` in the manifest before proposing anything. A rejection is a decision; re-opening
it needs new information, not a new session.

### 5. Propose

```
TOOLCHAIN REVIEW - <date>

Reconcile:  <unmanaged or missing components, or "clean">
Renew:      <component> - used? <yes/no> - risk changed? <yes/no> - recommend keep | retire
Update:     <component> <pinned> -> <latest> - what changed - recommend yes | no
Add:        <candidate> - the gap it closes (story ID) - capability - trust - tokens - recommend
Remove:     <component> - why it is no longer earning its context
Budget:     <current> of <budget> tokens; after these changes <projected>

Nothing above has been installed, updated or removed.
```

Then stop and wait. If the owner approves, the manifest entry - id, kind, scope, source, pinned
version, trust, capabilities, always-on tokens, purpose, and a decision with a rationale - is
written in the **same change** as the installation. A component installed without a manifest entry
fails `check` on the next commit, which is the intended outcome.
