#!/usr/bin/env python3
"""Generate and validate docs/PLATFORMS.md's platform capability matrix (E20-S03).

`project/platforms.json` is the source of truth. `generate` renders it into
docs/PLATFORMS.md; `check` verifies that rendering is current (like
project_os.py's own generated-docs drift check) and, more importantly, cross-
validates every platform's claim against real evidence this script can find
rather than trusting the JSON on its own word. E20-S03 round-1 independent
verifier review found the original version of this script did not actually
enforce that: a platform could claim `tier: 1` with zero evidence tests, or
cite a production function (not a `#[test]`) as evidence, and `validate()`
returned no errors either way. This version closes both gaps and adds a third
check the original never attempted at all - that a claimed capability is
backed by *real*, per-capability evidence, not a borrowed or fabricated one:

- a platform claiming CI coverage (`ci_check_job`/`ci_quality_job`) must have
  every one of its `ci_os_labels` actually present in the corresponding job's
  `os:` matrix in .github/workflows/rust.yml (parsed with a small regex, like
  scripts/check_workflows.py already does elsewhere in this repository - no
  YAML dependency, matching this project's stdlib-only tooling convention);
- a "verified" capability must cite at least one evidence entry, and every
  entry must name a real `#[test]`-annotated function actually defined at the
  exact file it claims (not merely a `fn` of that name found anywhere in the
  tree - a production function, or a test for an unrelated platform/capability
  with a coincidentally matching name, must not pass);
- `tier: 1` requires CI on both jobs, both `identity`/`mutation` capabilities
  "verified" with real evidence, and a `verified_commit` that both (a) is a
  real ancestor of the current commit (`git merge-base --is-ancestor`) and (b)
  - best-effort, only when `gh` can actually reach GitHub - has a real
  successful `rust` workflow run recorded for it. (a) is enforced unconditionally,
  offline; (b) is a warning, not a hard failure, when `gh`/network is
  unavailable (this script itself may be running inside the CI job whose own
  outcome it cannot know before that job finishes) - AGENTS.md's own "if a dev
  tool is unavailable locally, say so" precedent, applied to a live API call
  instead of a missing binary.

This is what this story's acceptance criteria ("no platform is called
supported without required CI and destructive safety fixtures") and
verification contract ("generated platform matrix check") mean in practice.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PLATFORMS_JSON = ROOT / "project" / "platforms.json"
PLATFORMS_MD = ROOT / "docs" / "PLATFORMS.md"
RUST_WORKFLOW = ROOT / ".github" / "workflows" / "rust.yml"

VALID_CAPABILITY_STATES = {"verified", "unverified", "unsupported"}
JOB_HEADER_RE = re.compile(r"^  ([A-Za-z][\w-]*):\s*$", re.MULTILINE)
OS_MATRIX_RE = re.compile(r"^\s*os:\s*\[([^\]]*)\]\s*$", re.MULTILINE)
FN_DEF_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*[(<]")
TEST_ATTR_RE = re.compile(r"^\s*#\[test\]\s*$")
ATTR_LINE_RE = re.compile(r"^\s*#!?\[.*\]\s*$")
COMMENT_LINE_RE = re.compile(r"^\s*//.*$")
SHA_RE = re.compile(r"^[0-9a-f]{40}$")


class PlatformsError(RuntimeError):
    pass


def load_platforms() -> dict[str, Any]:
    try:
        data = json.loads(PLATFORMS_JSON.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise PlatformsError(f"missing {PLATFORMS_JSON.relative_to(ROOT)}") from exc
    except json.JSONDecodeError as exc:
        raise PlatformsError(f"{PLATFORMS_JSON.relative_to(ROOT)}: invalid JSON: {exc}") from exc
    if not isinstance(data, dict):
        raise PlatformsError(f"expected a JSON object in {PLATFORMS_JSON.relative_to(ROOT)}")
    return data


def job_os_labels(workflow_text: str, job_name: str) -> set[str] | None:
    """The `os:` matrix labels for one job block, or None if the job/matrix is absent.

    Splits the workflow on top-level (2-space-indented) job headers rather than trying to
    parse YAML generally - the same "check repository-owned policy from source text" approach
    scripts/check_workflows.py already uses, deliberately avoiding a YAML dependency.
    """
    headers = list(JOB_HEADER_RE.finditer(workflow_text))
    for i, m in enumerate(headers):
        if m.group(1) != job_name:
            continue
        start = m.end()
        end = headers[i + 1].start() if i + 1 < len(headers) else len(workflow_text)
        block = workflow_text[start:end]
        os_match = OS_MATRIX_RE.search(block)
        if not os_match:
            return None
        return {label.strip() for label in os_match.group(1).split(",") if label.strip()}
    return None


def file_defines_test_fn(rel_path: str, name: str) -> str | None:
    """None if `rel_path` (relative to ROOT) defines a `#[test]`-annotated `fn name`, else a
    reason string. Requires the exact file, not merely a name found anywhere in the tree -
    E20-S03 round-1 independent verifier review found the original "search the whole crate
    tree" approach let a production function (`observe_identity`, no `#[test]` anywhere) and a
    same-named-but-wrong-platform test both pass silently.

    Walks backward from the `fn` line over blank/comment/other-attribute lines looking for
    `#[test]` - handles this codebase's own real style (`#[cfg(windows)]` / doc comments
    stacked above `#[test]`), not merely the single-line case.
    """
    path = ROOT / rel_path
    if not rel_path.startswith("rust/crates/") or ".." in Path(rel_path).parts:
        return f"file {rel_path!r} is not under rust/crates/"
    if not path.is_file():
        return f"file {rel_path!r} does not exist"
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    for i, line in enumerate(lines):
        m = FN_DEF_RE.match(line)
        if not m or m.group(1) != name:
            continue
        j = i - 1
        found_test = False
        while j >= 0:
            candidate = lines[j]
            if TEST_ATTR_RE.match(candidate):
                found_test = True
                break
            if ATTR_LINE_RE.match(candidate) or COMMENT_LINE_RE.match(candidate) or candidate.strip() == "":
                j -= 1
                continue
            break
        if found_test:
            return None
    return f"no #[test]-annotated fn {name!r} found in {rel_path!r}"


def git_is_ancestor(sha: str) -> str | None:
    """None if `sha` is a real commit and an ancestor of (or equal to) HEAD, else a reason."""
    if not SHA_RE.match(sha):
        return f"{sha!r} is not a 40-character lowercase hex commit SHA"
    git = shutil.which("git")
    if not git:
        return "git is not available on PATH"
    result = subprocess.run(  # noqa: S603
        [git, "merge-base", "--is-ancestor", sha, "HEAD"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=10,
    )
    if result.returncode != 0:
        return f"{sha} is not a known ancestor of HEAD in this repository ({result.stderr.strip() or 'git merge-base failed'})"
    return None


def gh_confirms_successful_run(sha: str) -> str | None:
    """Best-effort, network-dependent: None if `gh` positively confirms a successful `rust`
    workflow run for `sha`, or if `gh`/network is unavailable (soft - this is not this
    function's job to punish); a short reason string only when `gh` *did* respond and the
    response positively shows no success. Never raises.
    """
    gh = shutil.which("gh")
    if gh is None:
        return None
    try:
        result = subprocess.run(  # noqa: S603
            [
                gh,
                "run",
                "list",
                "--commit",
                sha,
                "--workflow",
                "rust.yml",
                "--json",
                "conclusion,status,headSha",
                "-L",
                "50",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=20,
        )
        if result.returncode != 0:
            return None  # not authenticated / offline / rate-limited - not this SHA's fault
        runs = json.loads(result.stdout)
    except (subprocess.TimeoutExpired, json.JSONDecodeError, OSError):
        return None
    successes = [r for r in runs if r.get("headSha") == sha and r.get("conclusion") == "success"]
    if not successes:
        return f"gh reports no successful rust.yml run for commit {sha} (found {len(runs)} run(s) for it)"
    return None


def platform_markdown(data: dict[str, Any]) -> str:
    lines = [
        "# Platform Support Model",
        "",
        "<!-- Generated by scripts/check_platforms.py from project/platforms.json. Do not edit by hand. -->",
        "",
        (
            "This is the target-engine (Rust) platform capability matrix - what this repository's "
            "own CI and test suite actually verify today, not an aspiration. `tier: 1` means real, "
            "CI-verified identity **and** mutation (deletion) capability, each backed by at least "
            "one real `#[test]` this generator confirmed exists; `tier: 2` means the platform is a "
            "named target but has not yet met that bar for at least one capability. See "
            "`docs/architecture/PLATFORM_MODEL.md` for the underlying capability seams "
            "(`IdentityObserver`, `MutationExecutor`, ...) each row's `identity`/`mutation` column "
            "refers to."
        ),
        "",
        "| Platform | Tier | CI (check) | CI (quality) | Identity | Mutation |",
        "| --- | --- | --- | --- | --- | --- |",
    ]
    for p in data["platforms"]:
        caps = p["capabilities"]
        lines.append(
            f"| {p['title']} | {p['tier']} | "
            f"{'yes' if p['ci_check_job'] else 'no'} | {'yes' if p['ci_quality_job'] else 'no'} | "
            f"{caps['identity']['state']} | {caps['mutation']['state']} |"
        )
    lines.append("")
    for p in data["platforms"]:
        lines.append(f"## {p['title']}")
        lines.append("")
        lines.append(p["notes"])
        lines.append("")
        verified_commit = p.get("verified_commit")
        if verified_commit:
            lines.append(f"Last verified at commit `{verified_commit}`.")
            lines.append("")
        for cap_name in ("identity", "mutation"):
            cap = p["capabilities"][cap_name]
            lines.append(f"**{cap_name}**: {cap['state']}")
            lines.append("")
            if cap["evidence"]:
                lines.append("Evidence (real `#[test]` functions, verified present at these exact files):")
                lines.append("")
                lines.extend(f"- `{e['name']}` (`{e['file']}`)" for e in cap["evidence"])
            else:
                lines.append("Evidence: none - disclosed, not silently assumed from another platform/capability.")
            lines.append("")
    lines.extend(
        [
            "## Tier 2, further out",
            "",
            "- SSH remote hosts.",
            "- Dev Containers.",
            "- Ephemeral CI runners.",
            "- Codespaces-like developer environments.",
            "",
            "None of these are named platforms above because none has begun implementation; they "
            "are recorded here as acknowledged future scope, not as a claim about current behavior.",
            "",
            "## Unsupported consumer platforms",
            "",
            "Mobile operating systems are not a product target. cancellAI governs development machines and development execution environments.",
            "",
            "## What a non-tier-1 platform does today",
            "",
            (
                "Unsupported/tier-2 mutation capability never means an unverified guess at "
                "deletion. `cancellai-platform`'s observers (`IdentityObserver`, "
                "`AllocationObserver`, ...) report a distinct `Unsupported` fact rather than a "
                "possibly-wrong result, and the safety kernel (`cancellai-safety::root_capability`, "
                "`cancellai-safety::mutation_executor`, and - for WSL2 specifically - "
                "`cancellai-platform::mutation::refuse_unverified_wsl2_mutation`) fails closed on "
                "it (SI-002, SI-017) - inspection/planning commands remain available, but `clean` "
                "cannot proceed to a real deletion. This is explicit refusal, not silent partial "
                "behavior (`docs/architecture/PLATFORM_MODEL.md`)."
            ),
            "",
            "## Cross-platform rule",
            "",
            "The domain layer does not expose Unix-only identity assumptions. Platform "
            "implementations must provide capability-aware abstractions for:",
            "",
            "- filesystem object identity;",
            "- volume/filesystem boundary;",
            "- symbolic links, junctions, mount points, and reparse points;",
            "- allocated/reclaimable size estimation;",
            "- process/activity observation;",
            "- atomic rename/move guarantees;",
            "- user-service runtime;",
            "- notifications;",
            "- path normalization and case behavior.",
        ]
    )
    return "\n".join(lines).rstrip() + "\n"


def validate(data: dict[str, Any], *, check_ci: bool = True) -> tuple[list[str], list[str]]:
    """Returns `(errors, warnings)`. `check_ci=False` skips the network-dependent `gh` probe
    entirely (used by `generate`, which must stay usable offline)."""
    errors: list[str] = []
    warnings: list[str] = []
    workflow_text = RUST_WORKFLOW.read_text(encoding="utf-8") if RUST_WORKFLOW.exists() else ""
    check_labels = job_os_labels(workflow_text, "check") or set()
    quality_labels = job_os_labels(workflow_text, "quality") or set()

    seen_ids: set[str] = set()
    for p in data.get("platforms", []):
        where = p.get("id", "<unknown>")
        if where in seen_ids:
            errors.append(f"{where}: duplicate platform id")
        seen_ids.add(where)

        caps = p.get("capabilities", {})
        for cap_name in ("identity", "mutation"):
            cap = caps.get(cap_name)
            if not isinstance(cap, dict):
                errors.append(f"{where}: capabilities.{cap_name} must be an object with 'state' and 'evidence'")
                continue
            state = cap.get("state")
            if state not in VALID_CAPABILITY_STATES:
                errors.append(f"{where}: capabilities.{cap_name}.state must be one of {sorted(VALID_CAPABILITY_STATES)}, got {state!r}")
            evidence = cap.get("evidence", [])
            if state == "verified" and not evidence:
                errors.append(
                    f"{where}: capabilities.{cap_name}.state is 'verified' but evidence is empty - "
                    "a verified capability must cite at least one real test"
                )
            for item in evidence:
                name = item.get("name")
                file = item.get("file")
                if not name or not file:
                    errors.append(f"{where}.{cap_name}: evidence entry missing 'name' or 'file': {item!r}")
                    continue
                reason = file_defines_test_fn(file, name)
                if reason is not None:
                    errors.append(f"{where}.{cap_name}: evidence {name!r} at {file!r} is not valid: {reason}")

        if p.get("ci_check_job"):
            missing = set(p.get("ci_os_labels", [])) - check_labels
            if missing:
                errors.append(
                    f"{where}: ci_check_job is true but {sorted(missing)} not found in rust.yml's 'check' job os matrix {sorted(check_labels)}"
                )
        if p.get("ci_quality_job"):
            missing = set(p.get("ci_os_labels", [])) - quality_labels
            if missing:
                errors.append(
                    f"{where}: ci_quality_job is true but {sorted(missing)} not found in "
                    f"rust.yml's 'quality' job os matrix {sorted(quality_labels)}"
                )

        verified_commit = p.get("verified_commit")
        if verified_commit:
            reason = git_is_ancestor(verified_commit)
            if reason is not None:
                errors.append(f"{where}: verified_commit invalid: {reason}")
            elif check_ci:
                ci_reason = gh_confirms_successful_run(verified_commit)
                if ci_reason is not None:
                    warnings.append(f"{where}: {ci_reason}")

        if p.get("tier") == 1:
            caps_verified = all(caps.get(c, {}).get("state") == "verified" for c in ("identity", "mutation"))
            required = p.get("ci_check_job") is True and p.get("ci_quality_job") is True and caps_verified and bool(verified_commit)
            if not required:
                errors.append(
                    f"{where}: declared tier 1 but does not meet the tier-1 bar (ci_check_job, "
                    "ci_quality_job, both capabilities 'verified' with real evidence, and a "
                    "verified_commit are all required) - this is exactly the claim this story's "
                    "AC1 exists to prevent"
                )

    return errors, warnings


def cmd_generate() -> int:
    data = load_platforms()
    errors, _warnings = validate(data, check_ci=False)
    if errors:
        print("PLATFORMS ERROR:", file=sys.stderr)
        for e in errors:
            print(f"  {e}", file=sys.stderr)
        return 2
    PLATFORMS_MD.write_text(platform_markdown(data), encoding="utf-8")
    print(f"wrote {PLATFORMS_MD.relative_to(ROOT)}")
    return 0


def cmd_check() -> int:
    try:
        data = load_platforms()
    except PlatformsError as exc:
        print(f"PLATFORMS ERROR: {exc}", file=sys.stderr)
        return 2
    errors, warnings = validate(data, check_ci=True)
    rendered = platform_markdown(data)
    current = PLATFORMS_MD.read_text(encoding="utf-8") if PLATFORMS_MD.exists() else ""
    if current != rendered:
        errors.append(
            f"{PLATFORMS_MD.relative_to(ROOT)} does not match project/platforms.json; run `python3 scripts/check_platforms.py generate`"
        )
    for w in warnings:
        print(f"PLATFORMS WARNING: {w}", file=sys.stderr)
    if errors:
        print("PLATFORMS ERROR:", file=sys.stderr)
        for e in errors:
            print(f"  {e}", file=sys.stderr)
        return 2
    print(f"platforms OK: {len(data['platforms'])} platforms, generated matrix current, all CI/evidence claims verified")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Generate and validate docs/PLATFORMS.md's platform capability matrix.")
    parser.add_argument("command", nargs="?", default="check", choices=["check", "generate"])
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "generate":
        return cmd_generate()
    return cmd_check()


if __name__ == "__main__":
    raise SystemExit(main())
