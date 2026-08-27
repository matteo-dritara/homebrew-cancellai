# Market and Engineering Standards Research - August 2026

This research snapshot grounds product/engineering decisions at the time the roadmap was created. It is not a permanent truth source; provider/standard facts must be refreshed when implementation reaches the relevant epic.

## Market direction

### Adoption is already broad enough to create a durable category

JetBrains' Developer Ecosystem Survey 2026 reports more than 15,000 professional developers surveyed and, for May-July 2026, 90% using AI coding agents at work at least weekly and 68% daily. Gartner separately forecasts that by 2027 more than 65% of engineering teams using agentic coding will treat the IDE as optional, shifting more control, governance, and validation toward automated platforms. These are directional market signals, not cancellAI TAM calculations.

Sources:

- https://blog.jetbrains.com/research/2026/08/ai-coding-agent-adoption-2026/
- https://www.gartner.com/en/newsroom/press-releases/2026-05-20-gartner-says-the-market-for-enterprise-ai-coding-agents-is-entering-a-new-phase-of-expansion-and-competitive-realignment

The product thesis therefore does not depend on one vendor retaining one storage bug. It depends on agentic work creating persistent local state across multiple tools and environments.

### Agentic development is creating persistent local state

Modern coding agents run longer, use subagents/parallel tasks, maintain resumable sessions, tool outputs, checkpoints, caches, and local indexes. Public issue trackers across major agents contain examples of runaway storage in the tens/hundreds of GB. These reports are anecdotes rather than market-size statistics, but they validate the failure mode cancellAI targets.

Representative public signals reviewed during product research included:

- a Claude Code file-history report describing roughly 300 GB of local growth: https://github.com/anthropics/claude-code/issues/10107
- OpenAI Codex storage/session and Git checkpoint reports: https://github.com/openai/codex/issues
- Anthropic Claude Code debug/file-history/storage reports: https://github.com/anthropics/claude-code/issues
- OpenCode local storage reports: https://github.com/anomalyco/opencode/issues
- Roo Code local storage reports: https://github.com/RooCodeInc/Roo-Code/issues

Issue reports are evidence that the failure mode exists, not prevalence estimates. cancellAI must not convert anecdotes into market-size claims.

### Vendor cleanup is becoming native

Claude Code exposes built-in cleanup retention configuration; Gemini CLI documents automatic session retention with age/count controls and manual session deletion; GitHub Copilot CLI documents local session state, logs, checkpoints/workspace artifacts, and a cross-session SQLite store. This is why cancellAI should not build its moat around deleting one vendor's old files.

Sources:

- https://code.claude.com/docs/en/claude-directory
- https://github.com/google-gemini/gemini-cli/blob/main/docs/cli/session-management.md
- https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference

Durable differentiation:

- one inventory across providers;
- explainability and compatibility confidence;
- policy/budgets across providers;
- reversible lifecycle controls;
- local anomaly/pressure detection;
- provider-neutral artifact model;
- verifiable local safety boundary.

### Competitive shape

Specialized cleaners already exist. For example, `claude-code-cleaner` is a Rust TUI focused on `~/.claude`, with scan/select/preview/clean flows and orphan detection. That validates demand for visibility and safe cleanup, while also demonstrating why cancellAI needs a broader provider-neutral control-plane position rather than a Claude-only category cleaner.

Source: https://github.com/garrickz2/claude-code-cleaner

The defensible product layer is therefore above provider-native cleanup and above single-provider cleaners: unified inventory, evidence/confidence, lifecycle semantics, cross-provider budgets/policy, reversible control, anomaly prevention, and eventually fleet governance.

### Platform direction

Coding agents increasingly target macOS, Linux, Windows native and/or WSL. Therefore Windows is not a later cosmetic port. The safety model must avoid Unix-only assumptions from the start of the Rust architecture.

## Secure engineering standards mapped into cEOS

### NIST SSDF

Current finalized baseline reviewed: NIST SP 800-218 SSDF v1.1. NIST published an initial public draft of Revision 1 / SSDF v1.2 in December 2025; the project tracks it but does not label draft guidance as finalized compliance.

Sources:

- https://csrc.nist.gov/pubs/sp/800/218/final
- https://csrc.nist.gov/pubs/sp/800/218/r1/ipd
- https://csrc.nist.gov/projects/ssdf/publications

cEOS mapping:

- Prepare the organization -> Constitution, Engineering System, threat model, role protocol.
- Protect software -> repository/rulesets, dependency controls, release provenance.
- Produce well-secured software -> CR levels, verification-first stories, safety invariants.
- Respond to vulnerabilities -> SECURITY.md, regression evidence, release/rollback model.

### SLSA v1.2

SLSA v1.2 is the current specification reviewed. cancellAI uses SLSA as a supply-chain assurance model, not as a badge. Exact achieved levels must be established from release architecture and attestations.

Source: https://slsa.dev/spec/v1.2/

### GitHub artifact attestations

GitHub supports signed provenance attestations for build artifacts and SBOM attestations. Its current documentation describes how artifact attestations establish build provenance and how reusable workflows can strengthen build assurance.

Sources:

- https://docs.github.com/en/actions/concepts/security/artifact-attestations
- https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations

Target mapping: E17 canonical release evidence.

### OpenSSF

OpenSSF Scorecard automates checks around source/repository/supply-chain practices. OpenSSF also publishes source-management best practices including branch controls, review, and automation.

Sources:

- https://best.openssf.org/SCM-BestPractices/
- https://github.com/ossf/scorecard-action

Target mapping: continuous repository posture monitoring rather than a one-time badge exercise.

### CNCF Software Supply Chain Security Best Practices v2

CNCF TAG Security emphasizes defense in depth, signing/verification, artifact metadata, policy, testing, provenance and automation across a software factory.

Sources:

- https://tag-security.cncf.io/blog/software-supply-chain-security-best-practices-v2/
- https://tag-security.cncf.io/community/working-groups/supply-chain-security/supply-chain-security-paper-v2/sscbpv2/
- https://tag-security.cncf.io/community/working-groups/supply-chain-security/secure-software-factory/secure-software-factory/

cEOS adopts the software-factory idea but keeps tooling proportional to a developer CLI rather than importing enterprise ceremony wholesale.

### Sigstore

Sigstore/cosign supports identity-based keyless signing of files/blobs and transparency-linked verification bundles. It is a candidate supplemental verification mechanism if GitHub attestations/package-specific signing do not satisfy all distribution channels.

Sources:

- https://docs.sigstore.dev/quickstart/quickstart-ci/
- https://docs.sigstore.dev/cosign/signing/signing_with_blobs/

Selection belongs in a release tooling ADR rather than being hard-coded before Rust distribution exists.

### RustSec and cargo-deny

RustSec maintains the Rust advisory database and tooling such as `cargo-audit`; `cargo-deny` can enforce advisories, licenses, sources and dependency constraints.

Sources:

- https://rustsec.org/
- https://embarkstudios.github.io/cargo-deny/checks/cfg.html

Target mapping: E02 Rust quality baseline.

### Cross-platform Rust distribution

`dist`/cargo-dist currently supports generating shell, PowerShell, npm, Homebrew and MSI installer outputs and is a strong candidate for the future release factory.

Sources:

- https://axodotdev.github.io/cargo-dist/book/reference/config.html
- https://axodotdev.github.io/cargo-dist/book/installers/homebrew.html

It is a candidate, not a constitutional dependency. E17 evaluates it against security, maintenance and packaging needs at implementation time.

## Engineering workflow influences

### Small changes / trunk-based development

DORA describes trunk-based development as short-lived branches and frequent mainline integration, paired with automated tests. Google's public engineering practices explain why small changes are easier to review, test, rollback and reason about.

Sources:

- https://dora.dev/capabilities/trunk-based-development/
- https://dora.dev/capabilities/continuous-delivery/
- https://google.github.io/eng-practices/review/developer/small-cls.html
- https://google.github.io/eng-practices/review/reviewer/looking-for.html

cEOS uses this as the default merge model, but raises verification depth according to CR level.

### ADRs

Michael Nygard's Architecture Decision Record pattern captures context, decision, status and consequences in small version-controlled documents.

Source: https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions

cancellAI uses ADRs for accepted architecture decisions and RFCs for unresolved competing proposals.

### Policy separation

Open Policy Agent demonstrates the durable architecture pattern of separating policy decision-making from enforcement over structured input. cancellAI adopts the separation and deterministic/explainable policy concept, but does not commit to embedding OPA/Rego; a bespoke typed local resolver is currently a better fit for the small deterministic authority kernel.

Source: https://www.openpolicyagent.org/docs

## Explicit non-adoptions

The project intentionally does not adopt by default:

- heavyweight SAFe/enterprise portfolio ceremony;
- story points as a proxy for value;
- line coverage as a safety KPI;
- an LLM in the destructive decision loop;
- OPA/Rego runtime merely because policy-as-code is fashionable;
- Kubernetes-style complexity where a local deterministic library suffices;
- microservices for the single-machine core.

The goal is corporate-grade evidence and automation without corporate-grade accidental bureaucracy.
