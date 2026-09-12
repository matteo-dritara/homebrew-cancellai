---
name: adversarial-cases
description: Generate concrete counterexample cases against a cancellAI change - the eleven falsification axes from the verifier procedure turned into named, runnable tests. Use before writing CR2/CR3/CR4 code, when designing tests for a safety boundary, when asked for "adversarial tests", "edge cases", "come si rompe", or when a test suite looks like it only proves the happy path.
allowed-tools: Read, Glob, Grep, Bash(rg:*), Bash(cargo test:*), Bash(python3 -m pytest:*)
---

# Adversarial cases

A passing test suite is evidence, not proof. This skill produces the counterexamples a suite has
to survive before a change is allowed to claim safety. It is used by the executor *before* code
(so verification is planned first) and by the verifier *against* code.

## The falsification axes

`docs/development/AGENT_PROTOCOL.md` names eleven. For the change under analysis, work each one and
either produce a concrete case or state why the axis cannot apply - "not applicable" is an answer
that must be justified, never a silent skip.

| # | Axis | The question to answer concretely |
|---|---|---|
| 1 | Path / identity | Does the thing acted on at decision time stay the same thing at mutation time? Rename, relink, remount, case-fold between the two. |
| 2 | Partial reads / permissions | An unreadable subtree: does it read as *empty* (a defect) or as *unknown* (correct)? Sizes must degrade to lower bounds. |
| 3 | Links / mounts / reparse points | Symlink, hardlink, junction, bind mount, APFS firmlink, crossing a device boundary in or out of the root. |
| 4 | Provider version / layout drift | An unrecognised directory shape: does the build find *nothing to clean* (correct) or guess (defect)? |
| 5 | Concurrency | Provider process starts mid-run; two cancellAI runs overlap; file written while planned. |
| 6 | Crash / failure / retry | Kill between plan and mutation, mid-mutation, after mutation before ledger write. Is the resumed state truthful? |
| 7 | Boundary values | Zero, one, exactly-`--days`, exactly-`--keep-latest`, empty root, root that is a file, maximum path length. |
| 8 | Policy / trust conflicts | Two policies disagree; a manifest claims a capability or trust level it was not granted. |
| 9 | Platform differences | macOS case-insensitivity, Linux permissions, Windows reparse points and locked files, WSL interop paths. |
| 10 | Malformed / untrusted input | Truncated JSON, wrong schema version, unicode and control characters in names, hostile provider manifest. |
| 11 | Performance / large datasets | 10k and 100k artifacts: does the safety probe still run per artifact, and does planning avoid re-walking? |

## Safety-specific generators

Beyond the axes, for anything that can reach a mutation:

- **Protection bypass.** For every protected name in `docs/security/SAFETY_INVARIANTS.md`, try it
  as written, after resolution, and case-folded. A barrier that depends on spelling is not a barrier.
- **Unknown-to-authority promotion.** Find any path where *unknown*, *partial* or *unobservable*
  ends up treated as *absent* or *safe*. Constitutionally this must refuse, not proceed.
- **Root escape.** Relative traversal, symlink out of an approved root, a custom root that mimics
  provider structure. Inspection is not authorization.
- **Exit-code honesty.** A safety boundary that fires mid-run reports 4; an I/O failure 3; an
  unexpected bug 3. A run that withheld work must never report success.
- **Dry-run purity.** `--dry-run` must have no side effects at all, including ledger, cache and
  config writes.
- **Second-path check.** Does any new code decide what is *permitted*, rather than what was
  *asked for*? UI/Guardian/adapters must carry no independent mutation logic (SI / ADR-0019).

## Output

One table, then the tests:

| # | Axis | Case | Expected under the contract | Currently tested? | Test name to add |

Then write the missing tests. Rust: prefer a `#[test]` with a synthetic tree under `tempfile`;
Python: `tests/` with a synthetic filesystem tree. **Never target real `~/.claude` or `~/.codex`
data, and never commit real transcripts, prompts, auth material, secrets, or home paths.**

A case you cannot express as a test is a finding, not a nuisance: report it as a gap with the
reason it is not expressible.
