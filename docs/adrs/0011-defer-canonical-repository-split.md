# ADR-0011: Defer canonical repository split until cross-platform release cutover

- Status: Accepted
- Date: 2026-08-27
- Decision owners: project owner / cEOS
- Related: PD-010, PD-017, E17-S06

## Context

The current public repository is `matteo-dritara/homebrew-cancellai`. That name correctly reflects the shipping Python v1 distribution, but the target product is cross-platform and should eventually have a product-named canonical source repository while the Homebrew repository acts as a tap/distribution surface.

Renaming or splitting immediately would create migration and release risk during the more important P0 trust-floor and Python-to-Rust contract work.

## Decision

Keep `homebrew-cancellai` as the canonical source repository through the Python reference stage and early Rust proof/cutover work. Treat repository topology as a versioned distribution migration in E17-S06.

The target topology is conceptually:

```text
matteo-dritara/cancellai           canonical source + cross-platform releases
matteo-dritara/homebrew-cancellai  Homebrew tap / compatibility surface
```

The exact timing and GitHub migration mechanics require release evidence before execution.

## Consequences

Positive:

- no needless disruption during safety remediation;
- existing Homebrew users keep a stable tap;
- future provenance can point at a clean product-named canonical source;
- repository movement receives compatibility/rollback treatment rather than being a cosmetic rename.

Costs:

- documentation must clearly distinguish the current remote from the target topology;
- the eventual split requires issue/tag/release/history and package-path migration planning.

## Rejected alternatives

- **Rename now:** unnecessary risk before Rust becomes canonical.
- **Keep one `homebrew-*` repo forever:** makes a package-manager-specific surface the conceptual owner of a cross-platform product.
- **Create the new repository now and duplicate development:** creates two sources of truth during the highest-risk migration stage.
