#!/usr/bin/env python3
"""What a toolchain component *contains* is scanned, not trusted (E28-S01).

`check_agent_toolchain.py` answers a question about provenance: was this component declared, and
is its trust tier high enough for the capabilities it claims? That is the right question and only
half of it. A skill is prompt content injected into the agent that writes a file-deletion tool,
and prompt content can carry an instruction to ignore a rule, a path to exfiltrate a key, or a
tool permission far wider than its purpose. None of those change a trust tier, a pinned version,
or anything else the manifest records. `cargo deny` does not ask who published a crate; it reads
the crate. This does the same for the pack.

The instrument is NVIDIA's SkillSpector, and it enters as a pinned development dependency behind
this gate rather than as a component in the manifest it would be checking. Three design decisions
came from measuring it rather than from reading its README, and each one is a way a naive gate
would have been wrong:

**The exit code is not the verdict.** A skill planted with SSH-key exfiltration and
``eval "$(curl ...)"`` scored 42 with two HIGH findings, and the process still exited 0. A gate
wired to ``skillspector ... && echo ok`` would have passed it.

**The top-level recommendation is not the verdict either.** On that same planted skill the
report's `risk_recommendation` read `SAFE`, while the repository's own clean pack read `CAUTION`.
The per-skill `issues` are correct; the rolled-up field is not something to gate on. This gate
reads severities out of the per-skill records and ignores both summaries.

**Waivers key on `match_fingerprint`, not `finding_id`.** `finding_id` is regenerated per run -
two consecutive scans of an unchanged pack produced different ones - so a waiver keyed on it
would silently stop matching and the finding would return as new. `match_fingerprint` was stable
across runs and is the only identifier a waiver can be written against.

The scan runs with `--no-llm`, so no file contents leave the machine. SkillSpector's semantic
analyzers are skipped without an API key; that is a deliberate reduction in reach, and the report
names it rather than letting a static-only pass read as a full one. A mode that sends content to
a provider is opt-in and must be named in the evidence of whatever story turns it on.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
PROJECT = ROOT / "project"
SKILL_PACK = ROOT / ".claude" / "skills"
WAIVERS_FILE = PROJECT / "skill_content_waivers.json"

# Pinned because a scanner that changes what it detects changes the meaning of a passing run -
# the same reason project/coverage_baseline.json records the toolchain that measured it.
PINNED_SCANNER = "2.11.2"

# `skillspector --version` prints "SkillSpector v2.11.2", mixed with analyzer warnings. The
# version is matched rather than positionally split: the first attempt split on "V" and raised on
# the real output, which is a gate failing closed for the wrong reason.
VERSION_RE = re.compile(r"SkillSpector\s+v([0-9][0-9A-Za-z.\-+]*)", re.IGNORECASE)

# Severity ladder as SkillSpector emits it. Anything at or above REFUSE_AT fails the gate;
# everything below is reported so the owner sees it without the gate deciding for them.
SEVERITY_ORDER = ("NONE", "INFO", "LOW", "MEDIUM", "HIGH", "CRITICAL")
REFUSE_AT = "HIGH"


class SkillContentError(Exception):
    """A condition that stops the gate from producing a verdict at all."""


def severity_rank(severity: str) -> int:
    try:
        return SEVERITY_ORDER.index((severity or "NONE").upper())
    except ValueError:
        # An unknown severity is treated as the worst rather than ignored: a scanner that grows a
        # new level must not be able to introduce it silently below the refusal line.
        return len(SEVERITY_ORDER)


def scanner_version() -> str | None:
    """The installed scanner's version, or None when it is not installed at all."""
    binary = shutil.which("skillspector")
    if binary is None:
        return None
    proc = subprocess.run(  # noqa: S603 - fixed argument list, resolved executable
        [binary, "--version"], capture_output=True, text=True, check=False
    )
    for line in (proc.stdout + proc.stderr).splitlines():
        match = VERSION_RE.search(line)
        if match is not None:
            return match.group(1)
    return None


def scan(target: Path) -> dict[str, Any]:
    """Run the scanner over `target` and return its JSON report.

    `--no-llm` keeps every file on this machine. The report is written to a temporary file rather
    than read from stdout because the scanner writes progress and analyzer warnings there.
    """
    binary = shutil.which("skillspector")
    if binary is None:
        raise SkillContentError(
            "skillspector is not installed, so no scan was performed and no conclusion about the "
            "skill pack's content can be drawn. Install it with "
            "`uv tool install git+https://github.com/NVIDIA/SkillSpector@v" + PINNED_SCANNER + "`, "
            "or state in evidence that this gate could not run."
        )
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "report.json"
        subprocess.run(  # noqa: S603 - fixed argument list, resolved executable
            [binary, "scan", str(target), "--recursive", "--no-llm", "--format", "json", "--output", str(out)],
            capture_output=True,
            text=True,
            check=False,
        )
        if not out.exists():
            raise SkillContentError(
                f"skillspector produced no report for {target}; the scan did not complete and its silence is not a clean result"
            )
        report: dict[str, Any] = json.loads(out.read_text(encoding="utf-8"))
        return report


def load_waivers() -> dict[str, Any]:
    if not WAIVERS_FILE.exists():
        return {"schema_version": 1, "scanner": PINNED_SCANNER, "waivers": []}
    data: dict[str, Any] = json.loads(WAIVERS_FILE.read_text(encoding="utf-8"))
    return data


def findings(report: dict[str, Any]) -> list[dict[str, Any]]:
    """Every per-skill issue, flattened, with the skill it came from attached.

    Deliberately does not read `risk_recommendation` or `max_risk_score`: see the module docstring
    for what those two said about a skill that exfiltrates SSH keys.
    """
    out: list[dict[str, Any]] = []
    for skill in report.get("skills", []):
        for issue in skill.get("issues", []) or []:
            location = issue.get("location") or {}
            out.append(
                {
                    "skill": skill.get("name", "?"),
                    "fingerprint": issue.get("match_fingerprint", ""),
                    "severity": (issue.get("severity") or "NONE").upper(),
                    "category": issue.get("category", "?"),
                    "pattern": issue.get("pattern", "?"),
                    "file": location.get("file", "?"),
                    "line": location.get("start_line"),
                    "explanation": (issue.get("explanation") or "").strip(),
                }
            )
    return out


def provenance_errors(installed: str | None) -> list[str]:
    """Refuse before scanning when the scanner is absent or is not the pinned one."""
    if installed is None:
        return ["skillspector is not installed. This gate draws no conclusion rather than reporting a clean pack it never scanned."]
    if installed != PINNED_SCANNER:
        return [
            f"skillspector {installed} is installed but this gate is pinned to {PINNED_SCANNER}. A "
            "scanner that changes what it detects changes the meaning of a passing run; update the "
            "pin deliberately, in a change that says what the new version detects."
        ]
    return []


def evaluate(found: list[dict[str, Any]], waivers: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[str]]:
    """Split findings into refusals, waived and reported, and report stale waivers.

    A waiver that no longer matches anything is reported rather than ignored: it is either a
    finding that was fixed - in which case the waiver should go - or a fingerprint that drifted,
    in which case the finding is back and nobody was told.
    """
    by_fingerprint = {w["fingerprint"]: w for w in waivers.get("waivers", [])}
    matched: set[str] = set()
    refusals: list[dict[str, Any]] = []
    waived: list[dict[str, Any]] = []
    for finding in found:
        waiver = by_fingerprint.get(finding["fingerprint"])
        if waiver is not None:
            matched.add(finding["fingerprint"])
            waived.append({**finding, "waiver": waiver})
            continue
        if severity_rank(finding["severity"]) >= severity_rank(REFUSE_AT):
            refusals.append(finding)
    stale = [f"waiver {fp} matches nothing in the current scan" for fp in by_fingerprint if fp not in matched]
    return refusals, waived, stale


def describe(report: dict[str, Any]) -> list[str]:
    """What the scan did and did not reach, stated rather than left to the reader to assume."""
    completeness = report.get("analysis_completeness") or {}
    lines = [
        f"  skills scanned: {report.get('skills_scanned', '?')}  omitted: {report.get('skills_omitted', '?')}",
        f"  file inspection: {completeness.get('status', '?')}"
        f" ({completeness.get('fully_inspected_files', '?')} full,"
        f" {completeness.get('partially_inspected_files', '?')} partial,"
        f" {completeness.get('entirely_uninspected_files', '?')} uninspected)",
        "  semantic analysis: not run (--no-llm; no file content left this machine)",
    ]
    return lines


def completeness_errors(report: dict[str, Any]) -> list[str]:
    """Refuse when the scanner confirms it skipped a skill or file entirely.

    ``--no-llm`` deliberately leaves this scan partially inspected: static analysis cannot make
    the stronger semantic claim without sending prompt content to a provider. That is an explicit
    residual. A skill or file omitted altogether is different: it is outside even the static scan,
    so reporting it while passing would contradict the promise to scan every carried skill.
    """
    errors: list[str] = []
    omitted = report.get("skills_omitted")
    if not isinstance(omitted, int):
        errors.append("skillspector report has no integer skills_omitted count; scan coverage cannot be determined")
    elif omitted:
        errors.append(f"skillspector omitted {omitted} skill(s); every carried prompt-bearing skill must be scanned")

    completeness = report.get("analysis_completeness")
    if not isinstance(completeness, dict):
        errors.append("skillspector report has no analysis_completeness record; file coverage cannot be determined")
    else:
        uninspected = completeness.get("entirely_uninspected_files")
        if not isinstance(uninspected, int):
            errors.append("skillspector report has no integer entirely_uninspected_files count; file coverage cannot be determined")
        elif uninspected:
            errors.append(f"skillspector entirely skipped {uninspected} file(s); a file outside the scan cannot be treated as clean")
    return errors


def render(refusals: list[dict[str, Any]], waived: list[dict[str, Any]], stale: list[str]) -> None:
    for finding in refusals:
        line = f":{finding['line']}" if finding["line"] else ""
        print(
            f"REFUSED: {finding['skill']} - {finding['severity']} {finding['category']}"
            f" ({finding['pattern']}) at {finding['file']}{line}\n    {finding['explanation'][:200]}",
            file=sys.stderr,
        )
    for finding in waived:
        print(f"  waived: {finding['skill']} {finding['severity']} {finding['category']} - {finding['waiver']['reason'][:90]}")
    for message in stale:
        print(f"  warning: {message}")


def cmd_check() -> int:
    installed = scanner_version()
    errors = provenance_errors(installed)
    if errors:
        for error in errors:
            print(f"SKILL CONTENT ERROR: {error}", file=sys.stderr)
        return 2
    report = scan(SKILL_PACK)
    coverage_errors = completeness_errors(report)
    refusals, waived, stale = evaluate(findings(report), load_waivers())
    print(f"skill content scanned by skillspector {installed}")
    for line in describe(report):
        print(line)
    render(refusals, waived, stale)
    for error in coverage_errors:
        print(f"SKILL CONTENT ERROR: {error}", file=sys.stderr)
    if coverage_errors:
        return 1
    if refusals:
        print(
            f"\nSKILL CONTENT ERROR: {len(refusals)} finding(s) at {REFUSE_AT} or above. Repair the "
            "skill, or record a dated waiver in project/skill_content_waivers.json naming the "
            "specific finding and why it does not apply.",
            file=sys.stderr,
        )
        return 1
    print(f"\nskill content OK: no finding at {REFUSE_AT} or above ({len(waived)} waived)")
    return 0


def cmd_report() -> int:
    installed = scanner_version()
    if installed is None:
        print("SKILL CONTENT ERROR: skillspector is not installed", file=sys.stderr)
        return 2
    report = scan(SKILL_PACK)
    print(f"skillspector {installed} over {SKILL_PACK.relative_to(ROOT)}")
    for line in describe(report):
        print(line)
    found = findings(report)
    if not found:
        print("  no findings at any severity")
        return 0
    for finding in sorted(found, key=lambda f: -severity_rank(f["severity"])):
        line = f":{finding['line']}" if finding["line"] else ""
        print(f"  {finding['severity']:8} {finding['skill']:20} {finding['category']} ({finding['pattern']}) {finding['file']}{line}")
        print(f"           fingerprint {finding['fingerprint']}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Scan what the agent skill pack contains, rather than trusting its trust tier.")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("check", help="fail on any finding at or above the refusal severity")
    sub.add_parser("report", help="print every finding with the fingerprint a waiver would name")
    parser.set_defaults(command="check")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.command == "report":
            return cmd_report()
        return cmd_check()
    except (SkillContentError, OSError, ValueError, KeyError) as exc:
        print(f"SKILL CONTENT ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
