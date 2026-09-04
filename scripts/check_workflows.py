#!/usr/bin/env python3
"""Static safety checks for GitHub Actions workflow sources.

This intentionally avoids a YAML dependency. It checks repository-owned policy
that can be established safely from source text: third-party actions must be
pinned to immutable commit SHAs, broad write-all permissions are forbidden,
and pull_request_target requires an explicit project decision rather than
appearing accidentally.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
USES_RE = re.compile(r"^\s*-?\s*uses:\s*([^\s#]+)", re.MULTILINE)
FULL_SHA = re.compile(r"^[0-9a-fA-F]{40}$")

# E22-S01 (CR-TE-06): the tagged commit must face the same gate set as main, so release.yml's
# `verify`/`verify-rust` jobs are checked against their two sources of truth instead of a list
# hand-copied here, which is exactly what let them drift silently before (v1.8.0 reported
# success while `rust / quality (windows-latest)` failed on the same commit - no Rust check
# ran in release.yml at all).
PRECOMMIT_CONFIG = ROOT / ".pre-commit-config.yaml"
RELEASE_WORKFLOW = WORKFLOWS / "release.yml"
RUST_WORKFLOW = WORKFLOWS / "rust.yml"
AGENTS_MD = ROOT / "AGENTS.md"
HOOK_ID_RE = re.compile(r"^      - id:\s*(\S+)\s*$", re.MULTILINE)
ENTRY_RE = re.compile(r"^\s*entry:\s*(.+?)\s*$", re.MULTILINE)
STAGES_RE = re.compile(r"^\s*stages:\s*\[(.+?)\]\s*$", re.MULTILINE)
RUN_RE = re.compile(r"^\s*-?\s*run:\s*(.+?)\s*$", re.MULTILINE)
BLOCK_SCALAR_HEADERS = {"|", ">", "|-", ">-", "|+", ">+"}
IF_RE = re.compile(r"^\s*if:\s*.+$", re.MULTILINE)
CONTINUE_ON_ERROR_RE = re.compile(r"^\s*continue-on-error:\s*true\s*$", re.MULTILINE)
NEEDS_RE = re.compile(r"^\s*needs:\s*\[(.+?)\]\s*$", re.MULTILINE)

# E22-S01 (CR-TE-06 round 2): pytest and the remote ruff/mypy pre-commit hooks have no
# repository-owned `entry:` this checker can read (pytest is not a hook at all; ruff/mypy
# come from a third-party hook repo, not `repo: local`), so precommit_gate_commands() alone
# never notices them being dropped from release.yml. AGENTS.md's "Current Python checks"
# fenced command block is the actual documented contract for what main enforces - parsing it
# here, instead of hand-copying a second list, means a command added or changed there is the
# single source release.yml is compared against.
AGENTS_PYTHON_CHECKS_HEADER = "## Current Python checks"
AGENTS_NEXT_HEADER_RE = re.compile(r"^## ", re.MULTILINE)
FENCED_SH_RE = re.compile(r"```sh\n(.*?)```", re.DOTALL)

# Branch protection lives in GitHub settings and names checks as strings. A required check
# whose name does not match any job blocks every pull request forever and reports nothing,
# which is indistinguishable from a check that is simply slow. The desired configuration is
# recorded in REPOSITORY_GOVERNANCE.md; this verifies that every name it lists is a name a
# workflow can actually produce, including matrix expansion.
GOVERNANCE_DOC = ROOT / "docs" / "development" / "REPOSITORY_GOVERNANCE.md"
REQUIRED_CHECK_RE = re.compile(r"^\s*-\s*`([^`]+)`\s*$", re.MULTILINE)
REQUIRED_CHECKS_BLOCK = re.compile(r"<!-- required-checks:start -->(.*?)<!-- required-checks:end -->", re.DOTALL)
JOBS_KEY_RE = re.compile(r"^jobs:\s*$", re.MULTILINE)
JOB_RE = re.compile(r"^  ([a-zA-Z0-9_-]+):\s*$", re.MULTILINE)
JOB_NAME_RE = re.compile(r"^    name:\s*(.+?)\s*$", re.MULTILINE)
MATRIX_ENTRY_RE = re.compile(r"^\s*([a-zA-Z0-9_-]+):\s*\[(.+?)\]\s*$", re.MULTILINE)
OS_MATRIX_RE = re.compile(r"^\s*os:\s*\[(.+?)\]\s*$", re.MULTILINE)

# Third-party actions known to declare `runs.using: docker` in their own action.yml. GitHub
# only executes Docker container actions on Linux runners, so one of these scheduled into a
# job whose OS matrix also includes macOS/Windows fails before the action body ever runs
# (E02 verifier review round 1, E02-S02: EmbarkStudios/cargo-deny-action on windows-latest).
# This can't be derived from workflow source alone - it depends on the action's own
# metadata - so it is a repository-owned list a new docker-based action must be added to
# deliberately, not something this check can infer.
DOCKER_ONLY_ACTIONS = {
    "EmbarkStudios/cargo-deny-action",
}


class WorkflowError(RuntimeError):
    pass


def workflow_files() -> list[Path]:
    return sorted([*WORKFLOWS.glob("*.yml"), *WORKFLOWS.glob("*.yaml")])


def display_path(path: Path) -> Path:
    try:
        return path.relative_to(ROOT)
    except ValueError:
        return path


def _job_blocks(text: str) -> list[tuple[str, str]]:
    """Split one workflow file's `jobs:` section into `(job_id, body)` pairs."""
    jobs_start = JOBS_KEY_RE.search(text)
    if not jobs_start:
        return []
    jobs_text = text[jobs_start.end() :]
    blocks = list(JOB_RE.finditer(jobs_text))
    result: list[tuple[str, str]] = []
    for index, match in enumerate(blocks):
        job_id = match.group(1)
        end = blocks[index + 1].start() if index + 1 < len(blocks) else len(jobs_text)
        result.append((job_id, jobs_text[match.end() : end]))
    return result


def declared_check_names() -> set[str]:
    """Every status-check context the workflows in this repository can report.

    A matrix job never reports its bare name: GitHub appends the combination, so `test`
    with a two-value matrix produces `test (3.10)` and `test (3.14)` and nothing else.
    Emitting the bare name here would defeat the whole check.
    """
    names: set[str] = set()
    for path in workflow_files():
        text = path.read_text(encoding="utf-8")
        for job_id, body in _job_blocks(text):
            display = JOB_NAME_RE.search(body)
            base = display.group(1).strip("\"'") if display else job_id
            combinations: set[str] = set()
            if "strategy:" in body and "matrix:" in body:
                for entry in MATRIX_ENTRY_RE.finditer(body):
                    if entry.group(1) in {"include", "exclude"}:
                        continue
                    for value in entry.group(2).split(","):
                        combinations.add(f"{base} ({value.strip().strip(chr(34)).strip(chr(39))})")
            names.update(combinations or {base})
    return names


def docker_only_action_errors() -> list[str]:
    """Reject a known Docker-only action scheduled into a macOS/Windows matrix job.

    Regression guard for E02 verifier review round 1 (E02-S02): a Docker container action
    is silently a Linux-only step, and a matrix that also lists macOS/Windows fails those
    legs before the action body runs at all.
    """
    errors: list[str] = []
    for path in workflow_files():
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(ROOT)
        for job_id, body in _job_blocks(text):
            os_match = OS_MATRIX_RE.search(body)
            if not os_match:
                continue
            os_values = [v.strip().strip("\"'") for v in os_match.group(1).split(",")]
            if not any(re.match(r"(macos|windows)-", v, re.IGNORECASE) for v in os_values):
                continue
            for uses_match in USES_RE.finditer(body):
                action = uses_match.group(1).rsplit("@", 1)[0]
                if action in DOCKER_ONLY_ACTIONS:
                    errors.append(
                        f"{rel}: job {job_id!r} schedules Docker-only action {action!r} on a matrix "
                        f"including {os_values!r}; Docker container actions only run on Linux runners"
                    )
    return errors


def job_run_commands(text: str, job_id: str) -> set[str]:
    """The literal `run:` command of every single-line step in one job.

    Block-scalar steps (`run: |`) are excluded: those are shell scripts embedded in the
    workflow (the tag/evidence assertions in release.yml's `verify` job), not pointers to a
    named repository gate, so they have nothing to compare against a pre-commit hook entry or
    a Rust quality command.
    """
    for candidate_id, body in _job_blocks(text):
        if candidate_id != job_id:
            continue
        return {m.group(1) for m in RUN_RE.finditer(body) if m.group(1) not in BLOCK_SCALAR_HEADERS}
    return set()


def precommit_gate_commands() -> dict[str, str]:
    """Every local pre-commit hook's `entry` command, keyed by hook id.

    Hooks staged only at `commit-msg` are excluded: they lint the commit message text, which
    has no meaning re-run against an already-tagged commit.
    """
    if not PRECOMMIT_CONFIG.exists():
        return {}
    text = PRECOMMIT_CONFIG.read_text(encoding="utf-8")
    ids = list(HOOK_ID_RE.finditer(text))
    commands: dict[str, str] = {}
    for index, match in enumerate(ids):
        end = ids[index + 1].start() if index + 1 < len(ids) else len(text)
        block = text[match.end() : end]
        stages_match = STAGES_RE.search(block)
        if stages_match and "commit-msg" in stages_match.group(1):
            continue
        entry_match = ENTRY_RE.search(block)
        if entry_match:
            commands[match.group(1)] = entry_match.group(1)
    return commands


def agents_md_python_gate_commands() -> list[str]:
    """The exact `python3 ...` commands AGENTS.md's "Current Python checks" section lists.

    This is the documented ground truth for what main enforces, including pytest and the
    remote ruff/mypy hooks that have no `entry:` of their own. The dependency-install line is
    excluded: it provisions tooling, it verifies nothing.
    """
    if not AGENTS_MD.exists():
        return []
    text = AGENTS_MD.read_text(encoding="utf-8")
    header = text.find(AGENTS_PYTHON_CHECKS_HEADER)
    if header == -1:
        return []
    section_start = header + len(AGENTS_PYTHON_CHECKS_HEADER)
    next_header = AGENTS_NEXT_HEADER_RE.search(text, section_start)
    section_end = next_header.start() if next_header else len(text)
    fenced = FENCED_SH_RE.search(text, section_start, section_end)
    if not fenced:
        return []
    lines = [line.strip() for line in fenced.group(1).splitlines() if line.strip()]
    return [line for line in lines if line != "python3 -m pip install -r requirements-dev.txt"]


def job_body(text: str, job_id: str) -> str:
    """The raw body text of one job block, or the empty string if it does not exist."""
    for candidate_id, body in _job_blocks(text):
        if candidate_id == job_id:
            return body
    return ""


def blocking_job_errors(rel: Path, text: str, job_id: str) -> list[str]:
    """A required gate job must always run and every step in it must be allowed to fail the build.

    `continue-on-error: true` on any step, or an `if:` condition anywhere in the job (job- or
    step-level), lets the job report green while silently not enforcing what it claims to -
    the "disable_verify_rust" / "nonblocking_clippy" class of regression the round-1 review
    demonstrated against the previous version of this checker.
    """
    body = job_body(text, job_id)
    if not body:
        return [f"{rel}: job {job_id!r} is missing; it is a required release gate"]
    errors: list[str] = []
    if CONTINUE_ON_ERROR_RE.search(body):
        errors.append(f"{rel}: job {job_id!r} sets continue-on-error: true, so a failing step would not fail the build")
    if IF_RE.search(body):
        errors.append(f"{rel}: job {job_id!r} has a conditional 'if:' that could skip a required gate")
    return errors


def matrix_values(text: str, job_id: str, key_re: re.Pattern[str]) -> list[str]:
    body = job_body(text, job_id)
    match = key_re.search(body)
    if not match:
        return []
    return sorted(v.strip().strip("\"'") for v in match.group(1).split(","))


def release_gate_drift_errors() -> list[str]:
    """release.yml must re-run every pre-commit gate and every Rust quality gate.

    This is what makes AC3 of E22-S01 real: it derives the required gate set from the two
    files that already define it (`.pre-commit-config.yaml`, `rust.yml`'s `quality` job)
    instead of a copy hand-maintained inside this checker, so a gate added to either one and
    not mirrored into release.yml fails here rather than silently shipping a weaker release
    workflow again.
    """
    errors: list[str] = []
    if not RELEASE_WORKFLOW.exists():
        return ["release.yml is missing; it must re-run every gate at the tagged commit"]
    release_text = RELEASE_WORKFLOW.read_text(encoding="utf-8")
    release_verify_commands = job_run_commands(release_text, "verify")
    release_rel = display_path(RELEASE_WORKFLOW)

    for hook_id, entry in precommit_gate_commands().items():
        if entry not in release_verify_commands:
            errors.append(
                f".github/workflows/release.yml: job 'verify' is missing pre-commit gate "
                f"{hook_id!r} ({entry!r}); the release workflow must re-run every gate "
                f"pre-commit enforces on main"
            )

    agents_commands = agents_md_python_gate_commands()
    if not agents_commands:
        errors.append("AGENTS.md's 'Current Python checks' fenced command block is missing or empty")
    for command in agents_commands:
        if command not in release_verify_commands:
            errors.append(
                f".github/workflows/release.yml: job 'verify' is missing gate {command!r} "
                f"listed in AGENTS.md's 'Current Python checks'; pytest and the remote "
                f"ruff/mypy hooks have no pre-commit 'entry:' this check can otherwise see"
            )

    errors.extend(blocking_job_errors(release_rel, release_text, "verify"))
    errors.extend(blocking_job_errors(release_rel, release_text, "verify-rust"))

    publish_needs = matrix_values(release_text, "publish", NEEDS_RE)
    for required_dep in ("verify", "verify-rust"):
        if required_dep not in publish_needs:
            errors.append(
                f"{release_rel}: job 'publish' does not depend on {required_dep!r}; a release could be cut without that gate ever passing"
            )

    if not RUST_WORKFLOW.exists():
        errors.append("rust.yml is missing; release.yml has no Rust quality gate to compare against")
        return errors
    rust_text = RUST_WORKFLOW.read_text(encoding="utf-8")
    rust_rel = display_path(RUST_WORKFLOW)
    quality_commands = job_run_commands(rust_text, "quality")
    release_rust_commands = job_run_commands(release_text, "verify-rust")
    for command in sorted(quality_commands):
        if command.startswith("cargo install "):
            continue  # toolchain setup, not itself a gate
        if command not in release_rust_commands:
            errors.append(
                f".github/workflows/release.yml: job 'verify-rust' is missing Rust quality gate {command!r} from rust.yml's 'quality' job"
            )
    errors.extend(blocking_job_errors(rust_rel, rust_text, "quality"))

    quality_os = matrix_values(rust_text, "quality", OS_MATRIX_RE)
    verify_rust_os = matrix_values(release_text, "verify-rust", OS_MATRIX_RE)
    if quality_os and quality_os != verify_rust_os:
        errors.append(
            f"{release_rel}: job 'verify-rust' runs on {verify_rust_os}, but rust.yml's "
            f"'quality' job runs the same gates on {quality_os}; a platform dropped from "
            f"either matrix must be dropped from both deliberately, not silently"
        )
    return errors


def required_check_names() -> list[str]:
    if not GOVERNANCE_DOC.exists():
        return []
    block = REQUIRED_CHECKS_BLOCK.search(GOVERNANCE_DOC.read_text(encoding="utf-8"))
    if not block:
        return []
    return REQUIRED_CHECK_RE.findall(block.group(1))


def validate_workflows() -> None:
    errors: list[str] = []
    for path in workflow_files():
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(ROOT)
        if "pull_request_target:" in text:
            errors.append(f"{rel}: pull_request_target is forbidden without an explicit security ADR")
        if re.search(r"^\s*permissions:\s*write-all\s*$", text, flags=re.MULTILINE):
            errors.append(f"{rel}: permissions: write-all is forbidden")
        if "permissions:" not in text:
            errors.append(f"{rel}: workflow must declare explicit permissions")
        for match in USES_RE.finditer(text):
            spec = match.group(1)
            if spec.startswith(("./", "docker://")):
                continue
            if "@" not in spec:
                errors.append(f"{rel}: action has no immutable revision: {spec}")
                continue
            action, revision = spec.rsplit("@", 1)
            if not action or not FULL_SHA.fullmatch(revision):
                line = text.count("\n", 0, match.start()) + 1
                errors.append(f"{rel}:{line}: action must be pinned to a full 40-hex commit SHA: {spec}")
    errors.extend(docker_only_action_errors())
    errors.extend(release_gate_drift_errors())
    required = required_check_names()
    if not required:
        errors.append("docs/development/REPOSITORY_GOVERNANCE.md must list required checks in a required-checks block")
    else:
        declared = declared_check_names()
        for name in required:
            if name not in declared:
                errors.append(
                    f"required status check {name!r} matches no workflow job; it would never report and would "
                    f"block every pull request. Known job contexts: {sorted(declared)}"
                )

    if errors:
        raise WorkflowError("\n".join(errors))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate cancellAI GitHub Actions policy.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        validate_workflows()
        print(f"workflow policy OK: {len(workflow_files())} workflow files use explicit permissions and immutable action SHAs")
        return 0
    except (WorkflowError, OSError, UnicodeError) as exc:
        print(f"WORKFLOW ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
