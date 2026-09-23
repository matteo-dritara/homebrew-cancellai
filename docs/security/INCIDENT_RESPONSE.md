# Safety and Security Incident Response

This runbook covers incidents where cancellAI may have lost data, exceeded its authority, misclassified provider state, shipped compromised knowledge/release artifacts, or created a credible risk of doing so.

It complements GitHub vulnerability reporting in [`.github/SECURITY.md`](../../.github/SECURITY.md). It is an engineering runbook, not a promise of a specific support SLA.

## Incident classes

- **S0 - Constitutional safety breach:** actual or credible unintended irreversible deletion, path/root escape, protected/unknown data mutation, remote authority bypass, or release/knowledge compromise that can grant destructive authority.
- **S1 - High safety degradation:** reversible mutation outside expected policy, quarantine/restore integrity failure, stale-plan execution, material provider-layout misclassification, or broad false-positive plan with mutation prevented by a later barrier.
- **S2 - Integrity/availability defect:** partial execution, audit/evidence corruption, persistent-state corruption, severe runaway self-storage, or Guardian behavior that causes operational disruption without data-loss authority breach.
- **S3 - Ordinary defect:** non-destructive functional, UX, performance, documentation, or compatibility issue.

Severity is based on potential authority and blast radius, not report volume.

## Immediate containment

For S0/S1:

1. stop release promotion and destructive-capability rollout;
2. preserve relevant evidence without copying user artifact content unnecessarily;
3. identify affected version/provider/platform/capability;
4. use the narrowest available kill switch: revoke/downgrade a signed knowledge bundle, capability, provider version, release channel, or feature;
5. do not issue emergency cleanup instructions that bypass normal Safety Invariants;
6. open a tracked incident and owner-visible risk record.

A network-delivered compatibility update may **reduce** local authority during containment, but it may never create new destructive authority. Offline clients remain governed by their installed local safety kernel.

## Investigation

Reconstruct:

- source commit, release/knowledge provenance, installation source and channel;
- relevant story/RFC/ADR and Change Risk Level;
- Effective Policy and Authority Ceiling at decision time;
- inventory completeness and provider fingerprint;
- sealed plan/precondition evidence where available;
- platform/filesystem identity details;
- provider activity/concurrency state;
- executor/verifier and release-gate evidence;
- exact Safety Invariants and threat cases involved.

Prefer synthetic reproduction. Real user payload content should not enter project fixtures.

## Remediation

Safety fixes follow the same cEOS model under an expedited path:

- classify the fix at its true CR level;
- add a permanent counterexample/regression fixture first where practical;
- make the smallest change that restores the invariant;
- obtain independent verification;
- require owner-visible Safety Verdict for CR4;
- verify rollback/update/install behavior;
- publish advisories/release notes proportional to exposure.

Emergency never means bypassing Definition of Safe.

## Recovery and communication

When data may have been affected, communication should distinguish:

- confirmed impact;
- plausible impact not yet proven;
- versions/platforms/providers/capabilities affected;
- whether actions were reversible/quarantined or irreversible;
- remediation/update/rollback instructions;
- what evidence users can inspect locally without uploading sensitive content.

Do not imply recoverability when the action class was irreversible.

## Post-incident requirements

S0/S1 incidents require:

- root-cause analysis focused on system/control failure, not only the triggering line of code;
- update to Threat Model and/or Safety Invariants when the model was incomplete;
- regression/adversarial fixture retained permanently;
- review of why independent verification/release gates failed to catch the case;
- provider/knowledge capability downgrade until evidence justifies restoration;
- owner-visible closure decision.

## Kill-switch hierarchy

The preferred order is least invasive first:

1. knowledge/capability downgrade for the affected provider/version;
2. disable autonomous authority for the affected capability;
3. force Observe/Recommend mode for the affected release/provider/platform;
4. stop release distribution/promotion;
5. ship a narrowly scoped patched release.

There is no remote "delete switch". Central/federated systems can only reduce capability or inform a local node.

## Signed capability containment (E17-S07)

Rungs 1 and 3 of the hierarchy above are implemented as data, not as a control channel:
`cancellai-safety::incident`.

- **What a containment can say.** A `ContainmentNotice` is the payload of an ordinary signed
  knowledge bundle (`SUPPLY_CHAIN.md`, "Knowledge updates"). Each entry names an incident id,
  severity (`S0`/`S1`), a provider, and optionally the affected provider versions, action classes
  and platforms, plus a ceiling. The ceiling vocabulary has two members, `observe` and
  `recommend`; both sit below the authority any mutating action requires, and no field exists for
  a command, path, tier, restore or lift.
- **How it applies.** `effective_authority_under_containment` adds an
  `incident_containment_authority` constraint to the monotonic minimum, so containment can only
  lower the result. An installed provider version that is not known is treated as affected by a
  version-scoped containment.
- **Stop promotion.** While a containment names a provider, `ContainmentLedger::freezes_promotion_of`
  reports that its trust tier must not be raised; and even a raised tier cannot lift the
  containment ceiling, because the two constraints are independent.
- **Nothing remote can undo it.** The ledger is monotonic: a replayed or older bundle is refused,
  a newer bundle that omits an incident does not lift it, rolling the knowledge store back does
  not lift it, and a bundle's expiry does not lift it. Records are append-only, so a re-issue of
  the same incident - by the same publisher or another - adds to the scope in force and can only
  keep or tighten its ceiling; it never narrows or relaxes one. Only `ContainmentLedger::lift_locally`
  removes a containment, and it is for local code acting on the owner-visible closure decision
  this runbook requires.
- **Offline.** An unreachable knowledge service, an unparseable response, a bad signature, an
  unknown publisher, an expired bundle and a replay all leave the ledger exactly as it was
  (`RefreshOutcome::Offline` / `Refused`). A node that never reaches the service computes
  authority from its installed kernel alone.
- **Evidence.** Each applied containment yields an `IncidentEvidence` record: incident id,
  severity, provider, versions, action classes, platforms, ceiling, Safety Invariant ids, affected
  releases, knowledge provenance (publisher, sequence, issue time, payload digest) and the
  running build's release provenance (version, channel). The release provenance is read from
  compile-time metadata inside the safety crate; no caller can supply it. Evidence records have
  private fields and no public constructor, so only verified ingestion creates one and nothing
  can edit one afterwards.
- **Bounded.** The ledger holds at most `MAX_ACTIVE_RECORDS` (4096) containment records and
  tracks at most `MAX_TRACKED_PUBLISHERS` (256) publishers. A notice that would exceed either is
  refused whole (`LedgerFull`) and every existing containment stays. Scope and evidence lists are
  stored as canonical sets (sorted, each member once), and a re-issue that says exactly what a
  held record says - incident, scope, ceiling, severity, invariants, affected releases - takes no
  capacity; one that adds evidence is kept as its own record. Capacity is never made by evicting
  or weakening a containment. Every string is checked against a
  bounded identifier alphabet on the way in, so no provider payload content can reach the record.

## Containment on the live mutation path (E06-S07, ADR-0039)

The Rust CLI obeys the ledger. `plan` and `clean` compute every deletion's authority with
`effective_authority_under_containment` from the artifact's facts, the build's compiled release
channel and the persisted ledger; a contained deletion becomes an observation that names the
incident, and `clean` re-reads the ledger immediately before every deletion, so a notice installed
while it waits at its prompt still binds.

- **Installing a notice.** `cancellai-cli containment install <file>` reads at most
  `MAX_NOTICE_BYTES` (256 KiB) before parsing, verifies the bundle in full - expiry included - and
  appends it to `<state>/containment_log.jsonl` (`$CANCELLAI_HOME/state`, else
  `$HOME/.cancellai/state`). A refused notice leaves the history byte-identical. `containment
  list` shows what binds; `containment lift <incident-id> --confirm` is the only lift.
- **Who can sign.** The binary compiles in the cancellAI incident-response public key (publisher
  `cancellai-incident`). The owner may add publishers in `<state>/trusted_publishers.json`
  (`[{"publisher_id": ..., "public_key": "<64 hex>"}]`); none may reuse the project id, none may
  appear twice, and none gains any tier - containment can only lower authority.
- **Unknown is not empty.** A missing history is an empty ledger. A history or trust file that
  exists and cannot be read, parsed or re-verified caps every deletion at `Recommend`, and says
  why, until the owner repairs or removes it.
- **What the history resists, and what it does not.** Every event is re-verified on every load, so
  nothing arriving from outside - a notice, a replayed or expired bundle, a forged signature - can
  lift a containment. The history is owner state in the owner's account: deleting it, truncating
  it to an earlier valid prefix, or appending a lift all lift containment, and a process running as
  the same user can do any of them. The owner accepted this on 2026-09-23; a same-user attacker can
  delete the provider data directly, so the history adds nothing that attacker lacks.
- **Signing a notice** (maintainers). The private key lives only encrypted to the owner's GPG key
  (`~/cancellai-incident.key.gpg`, with an offline copy). Decrypt it into a pipe, never a file:
  build the bundle's `signing_bytes` as `cancellai-safety::KnowledgeBundle` defines them, sign
  with `openssl pkeyutl -sign -rawin -inkey <(gpg --decrypt ~/cancellai-incident.key.gpg)`, and
  write the bundle with publisher `cancellai-incident` and a sequence above every earlier one.

- **Fetching the published notice (E33-S01).** `cancellai-cli containment refresh` fetches the
  latest cumulative notice from
  `https://raw.githubusercontent.com/matteo-dritara/homebrew-cancellai/main/containment/notice.json`
  (or the single `https://` URL in `<state>/containment_feed_url`) with the system `curl` - https
  only, redirects included, 30 s, at most 256 KiB read - and installs it through exactly the
  `install` path. The transport is not trusted for authenticity; the signature is. An unreachable
  feed, a missing `curl`, a non-200 answer, an oversized, malformed, replayed, rolled-back or
  untrusted notice all leave the history byte-identical; a 404 means nothing is published; a
  notice already installed is "already current".
- **Publishing a notice** (maintainers). Sign it as above with a sequence above every earlier one,
  make it cumulative (every incident still in force), and commit it as `containment/notice.json`
  on `main`. Tell users to run `containment refresh`.

Not yet in place: fetching on a schedule. The Guardian's `run` loop does not exist yet
(E33-S02).
