# Release Gates

A release is eligible only when the gates required by its changes are green. The project distinguishes feature completion from safety evidence.

## G1 Functional

- acceptance criteria pass;
- unit/integration/contract tests pass;
- CLI/API schemas and generated docs are consistent;
- user-visible behavior has changelog/release-note coverage;
- no known regression is hidden as a warning.

## G2 Safety

- required Safety Invariants preserved;
- threat-model delta reviewed;
- CR3/CR4 adversarial tests pass;
- CR4 independent Safety Verdict is PASS or explicitly owner-accepted PASS_WITH_RESIDUALS;
- no unknown/partial condition is incorrectly promoted to destructive authority.

## G3 Compatibility

- tier-1 OS matrix appropriate to the release passes;
- provider compatibility fixtures pass for capabilities claimed;
- schema/policy/state migration compatibility is verified;
- unknown versions/platforms degrade truthfully.

## G4 Operability

- performance/self-budget regressions within thresholds;
- crash/recovery/rollback requirements pass;
- installer/update/uninstall smoke tests pass;
- documentation and troubleshooting paths exist;
- observability/audit evidence is sufficient for failures.

## Gate matrix by Change Risk

| Risk | G1 | G2 | G3 | G4 | Independent verifier | Owner Safety Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| CR0 | required | docs consistency | as relevant | as relevant | optional | no |
| CR1 | required | basic | required where exposed | as relevant | optional | no |
| CR2 | required | targeted | required | targeted | safety-relevant changes | no |
| CR3 | required | required | required | required | required | residual-risk summary |
| CR4 | required | required + adversarial | required | required | required | required |

## Epic closure

Closing an epic is what triggers a release (ADR-0014, PD-021). An epic may close when:

- every story is `done`;
- CR4 stories carry a Safety Verdict recording `PASS` or `PASS_WITH_RESIDUALS`;
- at most two independent review rounds were run, and anything surviving round two exists as
  a new backlog work item rather than as an unresolved finding;
- the gates required by the highest Change Risk Level in the epic are green.

`scripts/project_os.py` enforces the first two, `scripts/check_process.py` the third, and
`scripts/release.py check` refuses to let a closed epic sit unreleased.

## Release evidence packet

A canonical release records:

- source/tag/commit;
- included story/ADR/RFC IDs;
- risk distribution;
- G1-G4 results;
- CR4 Safety Verdict links;
- compatibility matrix;
- benchmark summary;
- dependency/security scan summary (for the Rust workspace: `cargo deny check` - licenses, sources, bans, RustSec advisories - `rust/deny.toml`, ADR-0015);
- SBOM/provenance/signature/attestation references;
- installer smoke results;
- known residual risks and rollback instructions.

## Emergency security fixes

Emergency does not mean bypass safety. It may reduce ceremony by using a narrowly scoped CR4 patch and expedited independent verification, but the invariant-specific regression test and Safety Verdict remain required before public release when the fix changes destructive behavior.
