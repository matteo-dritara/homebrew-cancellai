# Security Policy

cancellAI can receive authority to mutate agent-generated state. Violating its safety boundary is therefore a security issue even when no code execution is involved.

## Report privately

Use GitHub private vulnerability reporting (Security tab -> Report a vulnerability) for issues that can bypass a documented safety property. Do not publish destructive proof-of-concept details before triage.

## In scope

Examples include:

- deletion/mutation outside an approved provider root;
- deletion of protected, unknown, active, or partial-scan state contrary to `docs/security/SAFETY_INVARIANTS.md`;
- symlink/junction/mount/reparse escape;
- TOCTOU identity swap that defeats sealed-plan revalidation;
- custom root confusion that causes unrelated user data to be treated as provider data;
- policy/configuration/Guardian behavior that elevates authority above constitutional ceilings;
- malicious provider manifest/knowledge bundle gaining unearned authority or command execution;
- remote/fleet request bypassing local node safety;
- release/update provenance or channel constraints being bypassed;
- command injection or unsafe provider-native invocation;
- quarantine/restore behavior that silently overwrites unrelated state.

## Security contract

The canonical rules are:

- [Product Constitution](../docs/CONSTITUTION.md)
- [Safety Invariants](../docs/security/SAFETY_INVARIANTS.md)
- [Threat Model](../docs/security/THREAT_MODEL.md)
- [Incident Response](../docs/security/INCIDENT_RESPONSE.md)

A security fix that changes destructive behavior is CR4 and follows the expedited-but-not-bypassed safety workflow in `docs/development/RELEASE_GATES.md`.

## Supported versions

Until a formal support policy is introduced, only the latest stable tagged release is supported for security fixes. Beta/nightly channels may intentionally carry lower default authority and different support expectations.
