# Threat Model

## Scope

cancellAI is a local developer tool with potential authority to move, rewrite, or permanently delete agent-generated state. The primary security objective is **preventing unauthorized or misunderstood data loss**, followed by integrity, privacy, and supply-chain trust.

This is a living threat model. Any new provider mutation capability, network/fleet feature, knowledge update mechanism, or platform backend requires threat-model delta review.

## Assets

Highest-value assets:

1. user source repositories and unrelated filesystem data;
2. provider authentication/configuration/plugins/skills/memory;
3. active/resumable agent sessions and file history;
4. quarantined/archived payloads;
5. local policy and pin/protection intent;
6. release/knowledge trust metadata;
7. cancellAI event/evidence integrity.

Disk space is valuable, but never more valuable than preserving protected data.

## Actors

- local user/owner;
- local coding agents and their processes;
- provider CLIs/apps;
- cancellAI CLI/TUI/Guardian;
- community provider contributor;
- knowledge/release publisher;
- future fleet controller;
- local malicious process with user-level permissions;
- compromised dependency/build/release pipeline.

cancellAI is not a sandbox against an attacker who already has arbitrary code execution as the same user. It must, however, avoid turning untrusted metadata/configuration into additional deletion or command-execution power.

## Trust boundaries

```text
Untrusted/variable provider filesystem
           |
           v
  [Inventory observation boundary]
           |
           v
  Structured facts + evidence
           |
           v
 [Policy/knowledge decision boundary] <--- signed knowledge / local policy
           |
           v
       Sealed plan
           |
           v
    [SAFETY KERNEL]
           |
      revalidation
           |
           v
 Filesystem / vendor-native mutation
```

Separate trust boundaries also exist at GitHub/release CI, package managers/installers, local DB/quarantine, and future remote-controller communication.

## Primary misuse cases

### TM-01 Misconfigured provider root points at ordinary user data

Example: `CODEX_HOME=/Users/me/project` and that project contains a `tmp/` folder.

Control: provider root fingerprinting; custom low-confidence root is inspection-only; root capability revalidation. See SI-002.

### TM-02 Symlink/junction/reparse escape

A candidate or intermediate path changes to point outside the approved root.

Control: link object classification, root containment, platform-native reparse handling, sealed identity revalidation. See SI-003, SI-013, SI-017.

### TM-03 TOCTOU target swap

Between scan and execute, an attacker/provider replaces a planned path with another object.

Control: identity tokens and immediate pre-mutation revalidation. See SI-013, SI-016.

### TM-04 Mount/volume boundary crossing

A provider tree contains a mount/junction or quarantine destination is on another volume.

Control: explicit filesystem boundary facts and action capability. See SI-018.

### TM-05 Provider layout drift

A vendor changes local storage and cancellAI misclassifies a formerly disposable path.

Control: capability/version/layout fingerprint, unknown-version downgrade, signed compatibility knowledge, fixtures. See SI-004.

### TM-06 Protected scanner regression

A future broad scanner begins emitting `settings`, `plugins`, `memory`, auth, or config paths.

Control: independent protected barriers at candidate and execution boundaries. See SI-001, SI-006.

### TM-07 Partial scan hides active/protected state

Permissions, disappearing paths, corruption, or I/O errors cause cancellAI to infer a tree is safe.

Control: completeness state propagation, unknown=protected. See SI-008..010.

### TM-08 Concurrent provider write is lost

cancellAI rewrites history/index metadata while Claude/Codex writes concurrently.

Control: active-state block, native operation when verified, streaming atomic rewrite, stale identity detection. See SI-011, SI-015.

### TM-09 Native vendor delete semantics change

`codex delete` or another provider command starts deleting broader data or changes cascade behavior.

Control: versioned capability evidence, fake CLI contract tests, provider drift downgrade, post-action reconciliation. Native does not mean unconditionally trusted.

### TM-10 Malicious provider manifest

A community manifest points a cache pattern at broad paths or embeds a command.

Control: manifests are declarative data, root-scoped, trust-bounded, non-executable; untrusted manifests Observe Only. See SI-021, SI-022.

E05-S02 implements "untrusted manifests Observe Only" directly: `cancellai_safety::authority::effective_authority`'s
`provider_trust_authority` constraint caps `ProviderTrust::Untrusted` at `AuthorityLevel::Observe`, and
`cancellai_safety::trust_promotion::promote` is the only function that can move a manifest off that
tier, requiring a named verifier and fixture evidence - a manifest cannot embed a higher trust claim
about itself and have it accepted (`docs/architecture/PROVIDER_MODEL.md` "Trust chain").

### TM-11 Compromised knowledge bundle

An attacker serves altered compatibility rules intended to label data disposable.

Control: signed/attested bundle, trusted publisher policy, rollback/replay defense, bundle authority ceiling. See SI-022, SI-029.

### TM-12 Compromised release/build pipeline

Malicious binary is published under a legitimate release name.

Control: protected source, minimal CI permissions, reproducible/verifiable build process, SBOM, provenance attestations, cryptographic verification, release evidence. See `SUPPLY_CHAIN.md`.

### TM-13 Quarantine consumes remaining disk

Under disk pressure, a cross-volume or copy-based quarantine duplicates many GB and makes pressure worse.

Control: move-first same-volume semantics, capacity check, explicit copy capability, net-free-space reporting.

### TM-14 Restore overwrites new provider state

The original destination was recreated after quarantine.

Control: restore preconditions and conflict policy; no silent overwrite.

### TM-15 Guardian panic escalation

Critical disk pressure causes an emergency path that skips ordinary safety.

Control: pressure never creates authority; Guardian shares normal plan/safety executor. See SI-027, SI-028.

### TM-16 Stale local database authorizes deletion

A cache says an artifact is old while the provider recreated/reused it.

Control: current-state DB is non-authoritative and preconditions are re-observed. See SI-024.

### TM-17 Remote/fleet control becomes remote delete

A future server sends a command that bypasses local review/safety.

Control: remote is intent/policy distribution only; target node retains authority and audit. See SI-031.

## Privacy threats

cancellAI must minimize collection because agent state can contain source code, prompts, paths, credentials, and proprietary context.

Default mitigation:

- do not store content;
- do not upload file paths/project names/transcripts;
- knowledge telemetry is opt-in and minimized to provider/version/category/error/size buckets where useful;
- logs use structured redaction rules;
- support bundles require explicit preview/redaction before sharing.

## Denial-of-service / performance threats

Huge agent trees can make the scanner itself expensive. Controls include single-pass inventory, bounded concurrency, no recursive symlink following, self-budget, scheduled heavy benchmarks, and graceful partial results under I/O errors.

## Threat-model update trigger

Mandatory update when a change:

- adds a mutation path;
- increases Authority Ceiling;
- adds a network input;
- adds a manifest/adapter trust level;
- stores new data classes;
- introduces a new OS/platform;
- changes quarantine/restore/purge;
- changes build/signing/update behavior.
