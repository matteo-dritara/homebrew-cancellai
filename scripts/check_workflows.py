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
