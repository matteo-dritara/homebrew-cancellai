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


class WorkflowError(RuntimeError):
    pass


def workflow_files() -> list[Path]:
    return sorted([*WORKFLOWS.glob("*.yml"), *WORKFLOWS.glob("*.yaml")])


def declared_check_names() -> set[str]:
    """Every status-check context the workflows in this repository can report.

    A matrix job never reports its bare name: GitHub appends the combination, so `test`
    with a two-value matrix produces `test (3.10)` and `test (3.14)` and nothing else.
    Emitting the bare name here would defeat the whole check.
    """
    names: set[str] = set()
    for path in workflow_files():
        text = path.read_text(encoding="utf-8")
        jobs_start = JOBS_KEY_RE.search(text)
        if not jobs_start:
            continue
        jobs_text = text[jobs_start.end() :]
        blocks = list(JOB_RE.finditer(jobs_text))
        for index, match in enumerate(blocks):
            job_id = match.group(1)
            end = blocks[index + 1].start() if index + 1 < len(blocks) else len(jobs_text)
            body = jobs_text[match.end() : end]
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
