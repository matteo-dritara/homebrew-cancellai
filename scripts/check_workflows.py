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


class WorkflowError(RuntimeError):
    pass


def workflow_files() -> list[Path]:
    return sorted([*WORKFLOWS.glob("*.yml"), *WORKFLOWS.glob("*.yaml")])


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
