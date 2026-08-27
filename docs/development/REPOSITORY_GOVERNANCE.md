# Repository Governance

Some controls live in GitHub settings rather than Git. This document is the desired configuration for the current public repository and, later, the canonical `cancellai` repository. It prevents invisible platform settings from becoming an undocumented part of the engineering system.

## Current repository

Current canonical remote during the Python/reference stage:

```text
https://github.com/matteo-dritara/homebrew-cancellai
```

ADR-0011 defers the product-source/tap split until the cross-platform release factory is ready.

## Default branch ruleset

Target for `main`:

- require pull request before merge for implementation changes;
- squash merge as the normal merge strategy;
- block force pushes and branch deletion;
- require branch to be up to date when GitHub can do so without creating excessive merge churn;
- require conversation resolution;
- require the repository's cEOS checks appropriate to the current stage;
- prevent bypass except deliberate owner emergency administration;
- record every emergency bypass in an incident/evidence record.

For a single-maintainer repository, mandatory approval by a separate human may be impossible. Do not fake separation of duties. Independent Claude/Codex verification is engineering evidence, not a GitHub human-review identity.

## Applying branch protection

The controls above are GitHub settings, so nothing in this repository can enforce them.
`main` currently uses **classic branch protection**, not a ruleset; documenting the
mechanism the repository does not use is the drift this page exists to prevent.

The required checks are listed below, and `scripts/check_workflows.py` verifies that each
name matches a context a workflow can actually report - including matrix expansion. This
guard exists because the repository was found requiring a check named `test` while the
matrix produces `test (3.10)` and `test (3.14)`: a name that matches no job never reports,
is indistinguishable from a slow check, and blocks every pull request permanently.

<!-- required-checks:start -->
- `test (3.10)`
- `test (3.14)`
- `lint`
- `homebrew`
- `project-os`
- `pre-commit`
- `commit-convention`
- `Python reference security analysis`
<!-- required-checks:end -->

Apply, or reapply after drift:

```sh
gh api -X PUT repos/matteo-dritara/homebrew-cancellai/branches/main/protection --input - <<'JSON'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "test (3.10)", "test (3.14)", "lint", "homebrew",
      "project-os", "pre-commit", "commit-convention",
      "Python reference security analysis"
    ]
  },
  "enforce_admins": false,
  "required_pull_request_reviews": null,
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_conversation_resolution": true
}
JSON

# Tags naming a published release are immutable history.
gh api -X PUT repos/matteo-dritara/homebrew-cancellai/rulesets --input - <<'JSON'
{
  "name": "release tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "rules": [{ "type": "deletion" }, { "type": "non_fast_forward" }, { "type": "update" }]
}
JSON
```

`required_pull_request_reviews` is null deliberately. A single-maintainer repository cannot
produce a second human approver, and a required count satisfied by self-approval would be
theatre. Independent Claude/Codex verification is engineering evidence recorded under
`project/evidence/`; it is not a GitHub review identity, and this document does not pretend
otherwise. Add a required review the day a second maintainer exists.

Adding a job to a workflow does not add it to this list. Update the block above, run
`python3 scripts/check_workflows.py check`, and reapply the command.

Inspect the live state with:

```sh
gh api repos/matteo-dritara/homebrew-cancellai/branches/main/protection
```

## Required checks in the Python/reference stage

Use the exact check names reported by GitHub, covering at least:

- test suite;
- lint/type/format/docs drift;
- Homebrew formula audit/style;
- governance/project control-plane validation;
- documentation/workflow/process policy validation;
- commit-message convention over the pull-request range;
- the full pre-commit hook set, so hooks bind contributors who never installed them;
- CodeQL/security scanning where GitHub exposes it as a required compatible check.

When Rust becomes canonical, E02/E17 replace/extend these with workspace, dependency, cross-platform, differential, installer, provenance and release-evidence gates.

## Tag and release controls

Protect release tags matching `v*` from casual deletion/rewriting. A published canonical tag is immutable project history.

When E17 introduces automated releases:

- create a protected GitHub `release` environment;
- keep workflow permissions least-privilege;
- use OIDC/attestation identity rather than long-lived signing secrets where appropriate;
- require owner approval for stable CR4 release promotion until a documented maintainer model supersedes it;
- separate build from promotion so already-built evidence is promoted rather than rebuilt ad hoc.

## GitHub Actions

Repository default workflow token permission should be read-only unless a workflow explicitly needs more. Individual workflows declare permissions in source.

Active third-party/first-party actions are pinned to immutable full commit SHAs; Dependabot can propose updates, but supply-chain workflow updates are not auto-merged. `scripts/check_workflows.py` enforces the source-visible part of this policy.

Avoid `pull_request_target` for code execution. Introducing it requires an explicit security ADR and threat-model review.

## Security features

Enable where available:

- private vulnerability reporting;
- Dependabot vulnerability alerts/security updates;
- secret scanning and push protection;
- CodeQL/code scanning;
- dependency graph;
- security advisories;
- OpenSSF Scorecard monitoring when it can be added without weakening workflow trust.

Security tooling findings are evidence inputs, not automatic authority to release or mutate user data.

## CODEOWNERS

The current single owner is explicit in `.github/CODEOWNERS`. As maintainers appear, delegate ordinary areas narrowly while retaining explicit review ownership for:

- Product Constitution / product decision register;
- Safety Invariants and safety kernel;
- provider/knowledge trust boundaries;
- release workflows/signing;
- remote/fleet authority.

CODEOWNERS is a review-routing mechanism, not a substitute for the cEOS Change Risk Level and Safety Verdict.

## Configuration drift review

At every stable release, the release evidence should state whether repository settings still match this document. When practical, E17 should automate GitHub API checks for source rulesets/security settings using read-only credentials and produce a drift report rather than silently mutating repository governance.
