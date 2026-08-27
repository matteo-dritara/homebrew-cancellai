# Platform Support Model

## Target Tier 1

- macOS: Apple Silicon and supported Intel transition targets where release economics justify it.
- Linux: x86_64 and aarch64 on mainstream glibc environments; musl support is capability/release tested separately.
- Windows native: x86_64 first, with Windows filesystem/reparse semantics modeled natively.
- WSL2: explicit hybrid environment with Linux guest semantics and Windows-mounted filesystem awareness.

A target moves to Tier 1 only after functional, safety, compatibility, and installer smoke gates exist in CI/release evidence.

## Tier 2 later

- SSH remote hosts.
- Dev Containers.
- ephemeral CI runners.
- Codespaces-like developer environments.

## Unsupported consumer platforms

Mobile operating systems are not a product target. cancellAI governs development machines and development execution environments.

## Cross-platform rule

The domain layer does not expose Unix-only identity assumptions. Platform implementations must provide capability-aware abstractions for:

- filesystem object identity;
- volume/filesystem boundary;
- symbolic links, junctions, mount points, and reparse points;
- allocated/reclaimable size estimation;
- process/activity observation;
- atomic rename/move guarantees;
- user-service runtime;
- notifications;
- path normalization and case behavior.
