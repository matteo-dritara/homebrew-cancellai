# Contract Fixtures

This directory becomes the normative cross-engine behavior corpus during P0/P1.

Fixtures are **synthetic**. Never commit real Claude/Codex transcripts, prompts, source code, credentials, absolute personal paths, or copied provider state.

## Contract

Each fixture added by E01 should contain enough declarative metadata to describe:

- provider and provider-layout/version assumption;
- filesystem tree and platform semantics;
- observation/scan completeness;
- expected discovered artifacts and relationships;
- expected classifications/confidence;
- expected plan/actions or explicit safety block;
- relevant Safety Invariant IDs;
- expected diagnostics/exit semantics.

The same fixture corpus will be consumed by the frozen Python reference and the Rust engine. Differences require either a defect fix or an explicit contract-change record; they must not be normalized away in the comparator.

Large synthetic fixtures should be generated deterministically from small recipes rather than committed as multi-gigabyte blobs.
