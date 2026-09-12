# Hazard Analysis — the mutation control loop

An STPA pass over the one loop that can destroy something (E25-S09).

[`THREAT_MODEL.md`](THREAT_MODEL.md) is adversary-shaped, and good at that. It asks who would
attack this and how. STPA (Leveson) asks the other question: **where does a loss occur when no
component failed and no attacker existed** — when every part did what it was told and the
*interaction* was unsafe.

For this product that is not an academic distinction. Every one of the seven P0 defects the
2026-08-27 audit found was a control-structure defect and none of them needed an adversary: a
protected name believed enforced that was not, a root believed validated that was accepted on path
depth, an unreadable directory believed empty, an `--aggressive` flag believed bounded by
retention. In STPA's vocabulary each is the same shape — **the controller acted on a process model
that did not match the world** — and a threat model structurally cannot surface that class,
because there is nobody to attribute it to.

## Losses and hazards

| ID | Loss |
| --- | --- |
| L-1 | Data the user needed is destroyed and cannot be recovered. |
| L-2 | Data the user wanted removed is retained, and they believe it is gone. |
| L-3 | The user acts on a false account of what is on disk or what was done. |
| L-4 | The tool consumes the resource it exists to govern. |

| ID | Hazard | Leads to |
| --- | --- | --- |
| H-1 | A mutation is applied to an artifact that is protected, unknown, active, or outside the approved root. | L-1 |
| H-2 | A mutation is applied to a *different* artifact from the one the plan named. | L-1 |
| H-3 | A refusal, a partial result, or a failure is reported as success. | L-2, L-3 |
| H-4 | The inventory presented to the user does not describe the filesystem. | L-3 |
| H-5 | cancellAI's own state grows without bound. | L-4 |

## Control structure

```text
          ┌──────────────────────────────────────────────┐
          │ USER                                         │
          │ intent: status / plan / clean / configure    │
          └───────────────┬──────────────────────────────┘
                          │ command + flags
          ┌───────────────▼──────────────────────────────┐
          │ CLI / TUI                       (cancellai-cli, -tui)
          │ process model: what the user asked for       │
          └───────────────┬──────────────────────────────┘
                          │ requested action
          ┌───────────────▼──────────────────────────────┐
          │ POLICY                          (cancellai-policy)
          │ process model: age, keep-latest, category,   │
          │                retention, eligibility        │
          └───────────────┬──────────────────────────────┘
                          │ sealed plan
          ┌───────────────▼──────────────────────────────┐
          │ SAFETY KERNEL                   (cancellai-safety)
          │ process model: protection, root capability,  │
          │   activity, scan completeness, identity,     │
          │   reversibility, provider trust              │
          └───────────────┬──────────────────────────────┘
                          │ authorised mutation
          ┌───────────────▼──────────────────────────────┐
          │ PLATFORM SEAM     (cancellai-platform/src/mutation.rs)
          │ the only code permitted to call remove_*     │
          └───────────────┬──────────────────────────────┘
                          │ syscalls
          ┌───────────────▼──────────────────────────────┐
          │ FILESYSTEM / PROVIDER STATE                  │
          └───────────────┬──────────────────────────────┘
                          │ feedback
   inventory scan · identity probe · activity probe · error returns
```

Every arrow downward is a control action; every arrow upward is feedback that populates a process
model. **The process models are where the hazards live**, because a controller can only be as right
as its beliefs.

## Unsafe Control Actions

Four per control action: *not provided when needed*, *provided when unsafe*, *wrong timing or
order*, *wrong duration or scope*. Each is mapped to the invariant that constrains it, or marked
as a gap.

### CA-1 — Safety kernel authorises a delete

| # | Type | Unsafe control action | Constrained by |
| --- | --- | --- | --- |
| UCA-1.1 | Provided when unsafe | Authorises a delete of an artifact that is protected, unknown, or unclassified. | SI-001, SI-006 |
| UCA-1.2 | Provided when unsafe | Authorises a delete while the scan that produced the inventory was incomplete. | SI-008, SI-009, SI-010 |
| UCA-1.3 | Provided when unsafe | Authorises a delete while the provider process is running, or while its activity cannot be observed. | SI-001, C-02 |
| UCA-1.4 | Provided when unsafe | Authorises a delete outside the approved root, or one that escapes it through a link. | SI-003, SI-018 |
| UCA-1.5 | Wrong timing | Authorises on an identity established during planning, when the target changed between plan and execution. | SI-013, SI-016 |
| UCA-1.6 | Wrong scope | Authorises a category expansion (`--aggressive`) that silently widens past the retention rule. | SI-005 |
| UCA-1.7 | Not provided | Refuses everything because one artifact is unresolvable, so the user reaches for a blunter tool. | **Gap** — no invariant constrains over-refusal. See below. |

### CA-2 — Platform seam performs the deletion

| # | Type | Unsafe control action | Constrained by |
| --- | --- | --- | --- |
| UCA-2.1 | Provided when unsafe | Deletes without a sealed plan, invoked by a caller that skipped the kernel. | SI-016, SI-019 |
| UCA-2.2 | Provided when unsafe | Follows a symlink out of the root while deleting. | SI-003, SI-017 |
| UCA-2.3 | Wrong duration | A recursive delete continues after the root check stops holding mid-traversal. | SI-003 |
| UCA-2.4 | Wrong timing | Reports completion before the operation is durable. | SI-014 |

### CA-3 — Kernel authorises a configuration write

| # | Type | Unsafe control action | Constrained by |
| --- | --- | --- | --- |
| UCA-3.1 | Provided when unsafe | Rewrites provider configuration in a root that was only inspected, never validated as the provider's own. | SI-002, ADR-0013 |
| UCA-3.2 | Wrong timing | Rewrites while the provider is writing, losing concurrent changes. | SI-011, SI-015 |
| UCA-3.3 | Wrong scope | Rewrites more of the document than the setting it was asked to change. | SI-015 |

### CA-4 — CLI reports the outcome

| # | Type | Unsafe control action | Constrained by |
| --- | --- | --- | --- |
| UCA-4.1 | Provided when unsafe | Reports success when the run was safety-blocked, partial, or withheld. | SI-014 |
| UCA-4.2 | Provided when unsafe | Reports a size or count as exact when part of the scan was unreadable. | SI-008, SI-010 |
| UCA-4.3 | Wrong content | Reports the dry-run plan while execution would select a different one. | SI-012 |

### CA-5 — Guardian or policy raises effective authority

| # | Type | Unsafe control action | Constrained by |
| --- | --- | --- | --- |
| UCA-5.1 | Provided when unsafe | A detection's severity is treated as permission to act. | SI-027, SI-028 |
| UCA-5.2 | Provided when unsafe | Policy raises a ceiling the constitution sets. | SI-025 |
| UCA-5.3 | Provided when unsafe | Cached or database state is treated as truth about the filesystem. | SI-024 |

## Loss scenarios from process-model inconsistency

These are the scenarios STPA exists to surface, and the ones a threat model does not reach. In each,
every component behaves exactly as specified.

**LS-1 — The kernel believes an artifact is classified when it is unrecognised.** A provider changes
its layout; the adapter's pattern still matches the *name* but no longer the *meaning*. The kernel's
process model says "disposable cache"; the world says "session the user is mid-way through".
Constrained by SI-004 and C-13 — unknown versions lose unsafe capability — and the residual is that
a layout change which keeps the old names is indistinguishable from no change at all.

**LS-2 — The kernel believes the scan is complete because no error was returned.** A directory the
process cannot traverse yields an empty listing rather than an error on some filesystems. The
inventory is not wrong about what it saw; it is wrong about what there was. SI-008/SI-009/SI-010
constrain this, and they are the invariants the 2026-08-27 audit found unenforced.

**LS-3 — The kernel believes the target is the artifact it planned against.** Between planning and
execution the path is re-created, re-linked, or re-mounted. Nothing failed; the name simply stopped
referring to the same object. SI-013 requires revalidation immediately before mutation, and SI-017
requires it in platform-native terms rather than by string comparison.

**LS-4 — Two controllers each believe they hold the metadata.** cancellAI rewrites provider history
while the provider appends to it. Neither is faulty; the interaction loses writes. SI-011 and SI-015
constrain it.

**LS-5 — The user believes the tool reported everything.** A tool that refuses one artifact silently
and proceeds with the rest produces a truthful report of what it *did* and a misleading impression
of what it *found*. SI-010 and SI-014 constrain the reporting; the residual is that a user who
trusts a clean summary may never open the coverage view.

## Gaps this analysis found

**G-1 — No invariant constrains over-refusal (UCA-1.7).** Every invariant here pushes toward
refusing. That is the correct default for a destructive tool and it is deliberate, but it makes one
failure mode invisible to the whole set: a tool that refuses too readily is a tool the user works
around with `rm -rf`, and the loss then happens outside the system entirely. This is a real hazard
with no constraint and no gate, and naming it is the point of doing the analysis. Carried to the
backlog rather than answered here, because the answer is a product decision about how much refusal
is too much, not an engineering one.

**G-2 — No invariant constrains the *reporting* of an over-refusal.** Related and cheaper: when the
tool withholds, the exit code says so (exit 4) but nothing requires the reason to be actionable.

**G-3 — LS-5 has no gate.** SI-010 requires scan errors to be visible; nothing requires them to be
visible *in the summary the user actually reads*.

Each gap is a candidate mutant for `scripts/gate_sensitivity.py`, which is where an unconstrained
UCA becomes a measured one rather than a documented one.

## Relationship to the threat model

The two documents do not overlap and neither replaces the other.
[`THREAT_MODEL.md`](THREAT_MODEL.md) covers an actor with intent: a hostile provider manifest, a
forged knowledge bundle, a remote controller, a compromised release. This one covers loss without
intent. Where they meet — a manifest that claims a capability it was not granted — the threat model
owns the adversarial case (SI-021, SI-022) and this analysis owns the case where the manifest is
merely *wrong* and the kernel believes it.
