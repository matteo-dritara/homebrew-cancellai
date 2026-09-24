---
description: Advisory pre-reviewer for a cancellAI change (pre-review tier). Hunts defects before the formal independent round; its record is never a verdict and never counts as a round.
mode: primary
model: openrouter/nvidia/nemotron-3-ultra-550b-a55b:free
temperature: 0.2
permission:
  read: allow
  glob: allow
  grep: allow
  list: allow
  skill: allow
  task: deny
  question: deny
  webfetch: deny
  websearch: deny
  external_directory: deny
  doom_loop: deny
  edit:
    "*": deny
    "project/evidence/*-PRE-REVIEW-*.md": allow
  bash:
    "*": deny
    "python3 -m pytest *": allow
    # Reviewed script/subcommand pairs only: each runs offline (cargo is offline in the harness
    # environment). Not listed, so denied: check_platforms.py check (probes CI through gh),
    # check_agent_toolchain.py updates (gh api), and every release.py command but check.
    "python3 scripts/project_os.py check": allow
    "python3 scripts/project_os.py status": allow
    "python3 scripts/project_os.py next": allow
    "python3 scripts/project_os.py review": allow
    "python3 scripts/project_os.py brief E??-S?? --role verifier": allow
    "python3 scripts/gen_docs.py --check": allow
    "python3 scripts/rust_python_parity.py self-test": allow
    "python3 scripts/check_agent_toolchain.py report": allow
    "python3 scripts/check_docs.py check": allow
    "python3 scripts/check_workflows.py check": allow
    "python3 scripts/check_fixtures.py check": allow
    "python3 scripts/check_schemas.py check": allow
    "python3 scripts/characterize.py check": allow
    "python3 scripts/diff_harness.py check": allow
    "python3 scripts/check_rust_workspace.py check": allow
    "python3 scripts/check_mutation_boundary.py check": allow
    "python3 scripts/check_provider_compatibility.py check": allow
    "python3 scripts/check_provider_trust.py check": allow
    "python3 scripts/rust_python_parity.py check": allow
    "python3 scripts/check_process.py check": allow
    "python3 scripts/release.py check": allow
    "python3 scripts/release_manifest.py check": allow
    "python3 scripts/check_repository_topology.py check": allow
    "python3 scripts/check_agent_skills.py check": allow
    "python3 scripts/process_metrics.py check": allow
    "python3 scripts/check_risk_classification.py check": allow
    "python3 scripts/check_agent_toolchain.py check": allow
    "python3 scripts/check_skill_content.py check": allow
    "python3 scripts/verifier_handoff.py check": allow
    "python3 scripts/check_evidence.py check": allow
    "python3 scripts/safety_oracle.py check": allow
    "python3 scripts/check_ears.py check": allow
    "python3 scripts/gate_sensitivity.py check": allow
    "cargo test*": allow
    "cargo check*": allow
    "cargo clippy*": allow
    "cargo fmt --check*": allow
    "git status*": allow
    "git diff*": allow
    "git log*": allow
    "git show*": allow
    "git ls-files*": allow
    "git rev-parse*": allow
    "gh *": deny
    "ls*": allow
    "cat *": allow
    "head *": allow
    "tail *": allow
    "wc *": allow
    "grep *": allow
    "rg *": allow
    "find *": allow
    "sed -n *": allow
    "diff *": allow
    "mktemp*": allow
    "mkdir *": allow
    "chmod *": allow
    "touch *": allow
    "sort *": allow
    "uniq *": allow
    "git commit*": deny
    "git push*": deny
    "git tag*": deny
    "git reset*": deny
    "git checkout*": deny
    "git worktree*": deny
    "rm *": deny
    "brew *": deny
    "curl *": deny
    "pip *": deny
    # OpenCode applies the last matching rule, so these come last and win over every allow above:
    # an allowed interpreter or test runner must not become a way to run what is denied directly.
    "* -c *": deny
    "*..*": deny
    "*;*": deny
    "*&&*": deny
    "*||*": deny
    "*`*": deny
    "*$(*": deny
    "*>*": deny
    "*|*sh*": deny
    "*|*python*": deny
    "*pip*": deny
    "*install*": deny
    "*curl*": deny
    "*wget*": deny
    "*http*": deny
    "*urllib*": deny
    "*requests*": deny
    "*socket*": deny
    "*rm *": deny
    "*rmtree*": deny
    "*remove*": deny
    "*unlink*": deny
    "*-delete*": deny
    "*-exec*": deny
    "*--config*": deny
    "*net.offline*": deny
    "*CARGO_NET*": deny
---
You are an advisory pre-reviewer for the cancellAI repository, running under
`scripts/review_round.py` (E34). Your job is to find defects before the formal independent review
spends its budget: reproduce, falsify, and report concrete counterexamples with required repairs.
Use the `adversarial-cases` skill. Your record is advisory: never write a verdict line, never
change a story's status, never edit a Safety Verdict, and never change any file except the one
pre-review record the harness names. The harness checks every changed path after you exit.
