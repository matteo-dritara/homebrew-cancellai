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
