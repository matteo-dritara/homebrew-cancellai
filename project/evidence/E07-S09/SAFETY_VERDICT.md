# Safety Verdict - E07-S09

- Change: Provider-root intermediate-link containment
- Risk: CR4
- Commit/PR: `a4cb802..c519f86`
- Independent verifier: Codex (`/root`)
- Date: 2026-09-02

## Verdict

`FAIL`

## Safety surface changed

The change makes `cancellai-sealedfs::SealedRoot::establish` walk configuration roots
handle-relatively. Cleanup still establishes its provider root through the separate
`ApprovedRoot` path, which accepts the same intermediate-link resolution and can purge data
outside the lexical provider root.

## Invariants

| Invariant | Required property | Evidence | Result |
| --- | --- | --- | --- |
| SI-002 | Every mutation has a positively bounded provider root. | With `$HOME` an intermediate symlink to `outside`, a real `outside/.claude` is treated as default for cleanup and is approved. | FAIL |
| SI-003 | Mutation cannot escape through link indirection. | Native `clean --yes` deleted the stale session under `outside/.claude/projects/...`. | FAIL |
| SI-013 | Root identity/link state is bound immediately before mutation. | `configure` binds a descriptor safely; cleanup re-resolves the lexical intermediate-link path through `ApprovedRoot`. | FAIL |
| SI-019 | Mutation is evidence-gated through a complete authority boundary. | The leaf-only `roots::is_symlink` diagnostic allows the cleanup mutation path to bypass the newly complete configuration boundary. | FAIL |

## Adversarial cases

- Direct `SealedRoot::establish` intermediate-link, relative-path, and `..` refusal tests pass.
- End-to-end `configure` with symlinked `$HOME` exits 4 and preserves the outside settings file.
- End-to-end `clean --yes` with the identical root topology exits 0 and deletes the outside
  stale session. This is a present authority escape, not an acceptable residual.
- `SealedRoot`'s `mkdirat` EEXIST retry and byte/NUL handling were inspected; no separate
  configure-walk bypass was found.
- Non-Unix `SealedRoot::establish` remains `Unsupported`; no Windows junction capability is
  claimed.

## Differential / compatibility evidence

`cargo fmt --check`, clippy with warnings denied, workspace check, workspace test, and
`cargo deny check` pass. Cargo deny reports only existing unmatched BSD-2-Clause, BSD-3-Clause,
and ISC allow-list warnings.

## Known residual risks

The cleanup intermediate-link escape is unresolved. It blocks this CR4 change from closing.

## Rollback / recovery

No user data outside the synthetic verifier fixture was touched. Return E07-S09 to
`in_progress`; a round-2 repair must bind cleanup roots with the same whole-path authority
property before any cleanup mutation.

## Owner decision

`REJECT`

Owner note: Do not accept until both configuration and cleanup refuse the same intermediate-link
topology and preserve their outside sentinels.
