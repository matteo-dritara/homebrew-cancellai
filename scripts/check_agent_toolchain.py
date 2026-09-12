#!/usr/bin/env python3
"""The agent toolchain is a dependency, and dependencies need a manifest (E26-S01).

Skills, hooks, subagents, plugins, MCP servers and language servers are not configuration. They
are third-party code and third-party *prompt content* entering the agent that writes this
repository's code, and they arrive by `plugin install` with no review, no pin, no expiry and no
record of why. Every other dependency here is governed - `cargo deny` for crates, provider trust
for manifests, `dependabot` for updates - and this class was governed by nobody.

`project/agent_toolchain.json` is the manifest. This checker enforces four things:

**Nothing unmanaged.** A component installed into the project and absent from the manifest fails.
That is the whole supply-chain control: additions become visible, and visible additions get a
decision.

**Capability drives the trust bar.** A component that executes code, reaches the network, or
touches credentials must be `FirstParty` or `Vendor` and must carry a decision with a rationale.
`Community` or `Unknown` trust with those capabilities is refused - not because community work is
bad, but because "it was convenient" is not a reason to give an unreviewed third party a shell.

**Decisions expire.** A decision older than the review cadence is reported until it is renewed.
A toolchain that is never re-reviewed silently becomes a list of things nobody chose and nobody
can now justify removing.

**Context is budgeted.** Every always-on component costs tokens in every session forever. The
constitution (C-11) already says a storage-governance tool may not become an unbounded storage
producer; by the same argument an agent toolchain may not become an unbounded context producer.
The manifest carries a budget and this checker enforces it.

What it deliberately does **not** do is install, update or remove anything. Installing a plugin
executes third-party code and injects third-party prompt content; that is an owner decision, and
an agent proposing it is the correct division of labour. `report` produces the proposal.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "project" / "agent_toolchain.json"
USAGE = ROOT / "project" / "agent_toolchain_usage.json"
SKILLS_DIR = ROOT / ".claude" / "skills"
HOOKS_DIR = ROOT / ".claude" / "hooks"
SETTINGS = ROOT / ".claude" / "settings.json"

KINDS = ("skill", "hook", "subagent", "command", "plugin", "mcp-server", "lsp", "output-style", "statusline")
SCOPES = ("project", "user")
TRUST = ("FirstParty", "Vendor", "Community", "Unknown")
CAPABILITIES = ("prompt", "subagent", "executes-code", "network", "credentials", "writes-files")
# Capabilities that reach outside the agent's own reasoning. A component holding one of these is
# not "some text the model may read"; it is software running on the developer's machine.
PRIVILEGED = ("executes-code", "network", "credentials", "writes-files")
TRUSTED_FOR_PRIVILEGE = ("FirstParty", "Vendor")
STATUSES = ("approved", "trial", "retired")


class ToolchainError(RuntimeError):
    pass


def load_manifest() -> dict[str, Any]:
    data: dict[str, Any] = json.loads(MANIFEST.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise ToolchainError(f"unsupported schema_version {data.get('schema_version')!r}")
    return data


def parse_date(value: str, where: str) -> dt.date:
    try:
        return dt.date.fromisoformat(value)
    except (TypeError, ValueError) as exc:
        raise ToolchainError(f"{where}: {value!r} is not an ISO date") from exc


def validate_component(component: dict[str, Any]) -> list[str]:
    """Structural and policy problems with one manifest entry."""
    errors: list[str] = []
    identifier = component.get("id", "<unnamed>")
    for field in ("id", "kind", "scope", "source", "version", "trust", "capabilities", "always_on_tokens", "purpose", "decision"):
        if field not in component:
            errors.append(f"{identifier}: missing required field {field!r}")
    if errors:
        return errors

    if component["kind"] not in KINDS:
        errors.append(f"{identifier}: unknown kind {component['kind']!r}")
    if component["scope"] not in SCOPES:
        errors.append(f"{identifier}: unknown scope {component['scope']!r}")
    if component["trust"] not in TRUST:
        errors.append(f"{identifier}: unknown trust {component['trust']!r}")
    unknown = [c for c in component["capabilities"] if c not in CAPABILITIES]
    if unknown:
        errors.append(f"{identifier}: unknown capabilities {unknown}")
    if not str(component["purpose"]).strip():
        errors.append(f"{identifier}: has no purpose; a component nobody can justify is a component nobody should carry")

    decision = component["decision"]
    if decision.get("status") not in STATUSES:
        errors.append(f"{identifier}: decision status {decision.get('status')!r} is not one of {STATUSES}")
    if not decision.get("rationale", "").strip():
        errors.append(f"{identifier}: decision records no rationale")
    if not decision.get("by"):
        errors.append(f"{identifier}: decision records no decider")
    if not decision.get("date"):
        # Omitting the date exempted a component from expiry forever, because `stale_decisions`
        # skipped an entry with no date rather than refusing it.
        errors.append(f"{identifier}: decision records no date; a decision with no date never expires")

    privileged = [c for c in component["capabilities"] if c in PRIVILEGED]
    if privileged and component["trust"] not in TRUSTED_FOR_PRIVILEGE:
        errors.append(
            f"{identifier}: trust {component['trust']!r} with capabilities {privileged} - a component that "
            f"runs code, reaches the network or touches credentials must be {' or '.join(TRUSTED_FOR_PRIVILEGE)}"
        )
    return errors


# Per-machine state that lives under `.claude/` and is not a component. Everything else directly
# under `.claude/` is reported as unrecognised rather than ignored, so a component kind this
# enumerator has never heard of fails closed instead of arriving unseen.
LOCAL_STATE = {"settings.local.json", "scheduled_tasks.lock", "worktrees", "shell-snapshots", "todos"}
COMPONENT_DIRECTORIES = {"skills", "hooks", "agents", "commands", "output-styles"}
SETTINGS_FILES = ("settings.json", "settings.local.json")


def _settings_components(path: Path, present: dict[str, str]) -> None:
    """MCP servers, inline hook commands and a status line declared in a settings file.

    An inline hook is the case that matters most: `"command": "curl … | sh"` with no file in
    `.claude/hooks/` is the most direct code-execution path there is, and an enumerator that only
    looks at files never sees it.
    """
    try:
        settings = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        present[f"unreadable:{path.name}"] = str(path)
        return
    where = str(path.relative_to(ROOT))
    for name in settings.get("mcpServers", {}):
        present[f"mcp:{name}"] = where
    if settings.get("statusLine"):
        present["statusline"] = where
    for event, matchers in (settings.get("hooks") or {}).items():
        for matcher in matchers if isinstance(matchers, list) else []:
            for hook in matcher.get("hooks", []) if isinstance(matcher, dict) else []:
                command = str(hook.get("command", "")).strip()
                if not command:
                    continue
                # A hook whose command is one of this repository's own hook files is already
                # enumerated as that file. Anything else is an inline command in its own right.
                if ".claude/hooks/" in command:
                    continue
                present[f"hook-command:{event}:{command.split()[0]}"] = where


def installed_project_components() -> dict[str, str]:
    """Every component present in the repository, by manifest id.

    Only project scope is inspected. User-scoped plugins live outside the repository and are not
    present in CI, so the manifest records them as declared intent and this checker reports them
    rather than enforcing them - claiming to verify what it cannot see would be worse than
    admitting the boundary.

    Within project scope the enumeration is deliberately exhaustive and fails closed. An
    independent review installed seven unmanaged components of six kinds at once - an
    extensionless hook, a `.py` hook, a subagent, a slash command, an eighth skill, a nested
    `.claude/skills/`, an output style - and the first version of this function reported
    "nothing unmanaged". The control's entire purpose is that an addition becomes visible.
    """
    present: dict[str, str] = {}
    claude = ROOT / ".claude"
    if not claude.is_dir():
        return present

    for entry in sorted(claude.iterdir()):
        if entry.name in LOCAL_STATE or entry.name.startswith("."):
            continue
        if entry.is_dir() and entry.name in COMPONENT_DIRECTORIES:
            continue
        if entry.is_file() and entry.name in SETTINGS_FILES:
            continue
        present[f"unrecognised:{entry.name}"] = str(entry.relative_to(ROOT))

    # Skills, at any depth: a nested `<subdir>/.claude/skills/` loads for sessions under that
    # directory and is exactly as unmanaged as one at the root.
    for skills_dir in sorted(ROOT.rglob(".claude/skills")):
        if not skills_dir.is_dir() or "worktrees" in skills_dir.parts:
            continue
        for skill in sorted(skills_dir.iterdir()):
            if skill.is_dir() and not skill.name.startswith((".", "_")):
                present[f"skill:{skill.relative_to(ROOT).as_posix()}"] = str(skill.relative_to(ROOT))

    for directory, prefix, pattern in (
        (claude / "hooks", "hook", "*"),
        (claude / "agents", "subagent", "*.md"),
        (claude / "commands", "command", "**/*.md"),
        (claude / "output-styles", "output-style", "*"),
    ):
        if not directory.is_dir():
            continue
        for item in sorted(directory.glob(pattern)):
            if item.is_file() and not item.name.startswith("."):
                present[f"{prefix}:{item.stem}"] = str(item.relative_to(ROOT))

    for name in SETTINGS_FILES:
        path = claude / name
        if path.is_file():
            _settings_components(path, present)
    project_mcp = ROOT / ".mcp.json"
    if project_mcp.is_file():
        _settings_components(project_mcp, present)
    return present


def stale_decisions(components: list[dict[str, Any]], cadence: int, today: dt.date) -> list[tuple[str, int]]:
    stale: list[tuple[str, int]] = []
    for component in components:
        decision = component.get("decision", {})
        if decision.get("status") == "retired":
            continue
        raw = decision.get("date")
        if not raw:
            continue
        age = (today - parse_date(raw, f"{component['id']} decision date")).days
        if age > cadence:
            stale.append((component["id"], age))
    return sorted(stale, key=lambda item: -item[1])


def evaluate(data: dict[str, Any], installed: dict[str, str], today: dt.date) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    components = data.get("components", [])
    for component in components:
        errors.extend(validate_component(component))

    declared = {component["id"] for component in components if "id" in component}
    # A component may declare `members`, so a pack of skills is one decision rather than eight
    # entries - but every member is named, so an eighth skill appearing in the pack is unmanaged.
    members = {member for component in components for member in component.get("members", [])}
    duplicates = [item for item in declared if sum(1 for c in components if c.get("id") == item) > 1]
    if duplicates:
        errors.append(f"duplicate component ids: {sorted(set(duplicates))}")

    for identifier, where in sorted(installed.items()):
        if identifier not in declared and identifier not in members:
            errors.append(
                f"{identifier} is installed at {where} but is not in the manifest - an unmanaged component is one nobody decided to carry"
            )

    # A component that declares members is present when its members are: the pack is one decision,
    # and `cancellai-skill-pack` is not itself a path on disk.
    for component in components:
        if component.get("scope") != "project" or component.get("decision", {}).get("status") == "retired":
            continue
        declared_members = component.get("members", [])
        present = any(member in installed for member in declared_members) if declared_members else component["id"] in installed
        if not present:
            errors.append(f"{component['id']} is in the manifest with project scope but is not installed")

    retired = {c["id"] for c in components if c.get("decision", {}).get("status") == "retired"}
    retired_members = {m for c in components if c.get("decision", {}).get("status") == "retired" for m in c.get("members", [])}
    for identifier in sorted((retired | retired_members) & set(installed)):
        errors.append(
            f"{identifier} is marked retired but is still present at {installed[identifier]} - "
            "a retired component leaves the budget and the expiry accounting while continuing to run"
        )

    cadence = int(data.get("review_cadence_days", 90))
    for identifier, age in stale_decisions(components, cadence, today):
        warnings.append(f"{identifier}: decision is {age} days old, past the {cadence}-day review cadence")

    budget = int(data.get("context_budget_tokens", 0))
    total = sum(int(c.get("always_on_tokens", 0)) for c in components if c.get("decision", {}).get("status") != "retired")
    if budget and total > budget:
        errors.append(
            f"always-on context cost is {total} tokens against a budget of {budget}. "
            "Every session pays this before any work begins; retire something or raise the budget deliberately"
        )

    for entry in data.get("rejected", []):
        if not entry.get("reason", "").strip():
            errors.append(f"rejected entry {entry.get('id')!r} records no reason; a rejection nobody can revisit is folklore")

    return errors, warnings


def render_report(data: dict[str, Any], installed: dict[str, str], today: dt.date) -> str:
    components = [c for c in data.get("components", []) if c.get("decision", {}).get("status") != "retired"]
    cadence = int(data.get("review_cadence_days", 90))
    budget = int(data.get("context_budget_tokens", 0))
    total = sum(int(c.get("always_on_tokens", 0)) for c in components)
    lines = [
        "Agent toolchain",
        "",
        f"  components: {len(components)}   always-on cost: {total} of {budget} tokens per session",
        f"  review cadence: {cadence} days",
        "",
        f"  {'id':42} {'kind':10} {'scope':8} {'trust':11} {'tokens':>7}  capabilities",
    ]
    for component in sorted(components, key=lambda c: (c["scope"], c["id"])):
        privileged = "*" if any(c in PRIVILEGED for c in component["capabilities"]) else " "
        lines.append(
            f"  {component['id']:42} {component['kind']:10} {component['scope']:8} "
            f"{component['trust']:11} {component.get('always_on_tokens', 0):>7}{privileged} {','.join(component['capabilities'])}"
        )
    lines += ["", "  * runs code, reaches the network, or touches credentials", ""]

    stale = stale_decisions(components, cadence, today)
    if stale:
        lines += ["  Decisions past their review date - renew or retire:", ""]
        lines += [f"    {identifier} ({age} days)" for identifier, age in stale]
    else:
        lines.append("  No decision is past its review date.")

    unmanaged = sorted(set(installed) - {c["id"] for c in data.get("components", [])})
    if unmanaged:
        lines += ["", "  Installed but unmanaged - decide or remove:", ""]
        lines += [f"    {identifier} at {installed[identifier]}" for identifier in unmanaged]

    rejected = data.get("rejected", [])
    if rejected:
        lines += ["", f"  Previously rejected ({len(rejected)}) - do not re-evaluate without new information:", ""]
        lines += [f"    {entry['id']}" for entry in rejected]
    return "\n".join(lines)


# ---------------------------------------------------------------------------- E26-S02, E26-S03


def load_usage() -> dict[str, Any]:
    """Recorded invocations, or an empty record.

    Absence degrades to *unknown*, never to *unused*: a component with no record has not been
    shown to be idle, and retiring it on that basis would be a decision made from missing data.
    """
    if not USAGE.is_file():
        return {"schema_version": 1, "components": {}}
    data: dict[str, Any] = json.loads(USAGE.read_text(encoding="utf-8"))
    return data


def record_usage(identifier: str, today: dt.date) -> int:
    """Note that a component was invoked. Identity and a count only - no transcript, no paths.

    Refuses an id the manifest does not name: a usage record for an unmanaged component would
    make the retirement report describe something nobody decided to carry.
    """
    try:
        known = {component["id"] for component in load_manifest().get("components", [])}
    except (ToolchainError, OSError, ValueError):
        known = set()
    if known and identifier not in known:
        print(f"refused: {identifier!r} is not in the manifest; add the component before recording its use", file=sys.stderr)
        return 2
    data = load_usage()
    entry = data.setdefault("components", {}).setdefault(identifier, {"invocations": 0, "last": ""})
    entry["invocations"] = int(entry.get("invocations", 0)) + 1
    entry["last"] = today.isoformat()
    USAGE.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"recorded: {identifier} ({entry['invocations']} invocations, last {entry['last']})")
    return 0


def usage_since_decision(component: dict[str, Any], usage: dict[str, Any]) -> str:
    """Invocations since the decision, `0 since decision`, or `unknown`.

    Dates are compared as `datetime.date`, not as strings. A lexical comparison happens to work
    for well-formed ISO-8601 and silently gives a wrong answer for anything else, and a wrong
    answer here retires a component that is in use.
    """
    entry = usage.get("components", {}).get(component["id"])
    if entry is None:
        return "unknown"
    raw_decided = component.get("decision", {}).get("date", "")
    raw_last = entry.get("last", "")
    if raw_decided and raw_last:
        try:
            if dt.date.fromisoformat(str(raw_last)) < dt.date.fromisoformat(str(raw_decided)):
                return "0 since decision"
        except (TypeError, ValueError):
            return "unknown"
    return f"{entry.get('invocations', 0)}"


def upstream(source: str) -> tuple[str, str] | None:
    if not source.startswith("github:"):
        return None
    owner, _, name = source.removeprefix("github:").partition("/")
    return (owner, name) if owner and name else None


def cmd_updates(data: dict[str, Any]) -> int:
    """Compare each pinned version against upstream, and say so plainly when it cannot.

    A check that needs the network cannot be a gate, so this is a report. It degrades truthfully:
    with no `gh`, no network, or an unreachable source it says it *could not compare* - never that
    nothing changed. Nothing is fetched from a source the manifest does not already name.
    """
    binary = shutil.which("gh")
    print("Upstream comparison\n")
    if binary is None:
        print("  `gh` is not available, so no comparison was made.")
        print("  This is not 'everything is current'; it is 'nothing was checked'.")
        return 0
    checked = 0
    for component in data.get("components", []):
        remote = upstream(str(component.get("source", "")))
        if remote is None:
            continue
        checked += 1
        owner, name = remote
        try:
            result = subprocess.run(  # noqa: S603
                [binary, "api", f"repos/{owner}/{name}", "--jq", r'"\(.pushed_at)"'],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
        except OSError:
            print(f"  {component['id']:42} could not compare (gh failed)")
            continue
        if result.returncode != 0:
            print(f"  {component['id']:42} could not compare ({result.stderr.strip()[:60] or 'unreachable'})")
            continue
        pushed = result.stdout.strip().strip('"')[:10]
        # The pin itself, not only the push date: reporting "pinned 2.0.2, last push <date>" next
        # to each other looks like a comparison and is not one.
        latest = "unknown"
        try:
            tag = subprocess.run(  # noqa: S603
                [binary, "api", f"repos/{owner}/{name}/releases/latest", "--jq", ".tag_name"],
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            if tag.returncode == 0 and tag.stdout.strip():
                latest = tag.stdout.strip().lstrip("v")
        except OSError:
            latest = "could not compare"
        pinned = str(component.get("version", ""))
        if latest in ("unknown", "could not compare"):
            drift = f"latest release {latest}"
        elif latest == pinned:
            drift = "current"
        else:
            drift = f"**behind: upstream {latest}**"
        signal = ""
        try:
            age = (dt.date.today() - dt.date.fromisoformat(pushed)).days
            if age > 365:
                signal = f"  <- last push {age} days ago: abandonment candidate, not only a stale pin"
        except ValueError:
            pushed = "unknown"
        print(f"  {component['id']:42} pinned {pinned:10} {drift:34} last push {pushed}{signal}")
    print(f"\n  {checked} components have a comparable upstream.")
    print("  A new release that adds a hook or an MCP server turns prompt content into software with")
    print("  a shell. That is a new decision, not an update: re-record the capabilities before pinning it.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Govern the agent toolchain as a dependency.")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("check", help="reconcile the manifest against what is installed")
    sub.add_parser("report", help="the owner-facing review")
    sub.add_parser("updates", help="compare pinned versions against upstream (network, never a gate)")
    used = sub.add_parser("record", help="note that a component was invoked")
    used.add_argument("component")
    parser.set_defaults(command="check")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    command = args.command
    try:
        data = load_manifest()
        installed = installed_project_components()
        today = dt.date.today()
        if command == "report":
            print(render_report(data, installed, today))
            return 0
        if command == "updates":
            return cmd_updates(data)
        if command == "record":
            return record_usage(args.component, today)
        errors, warnings = evaluate(data, installed, today)
    except (ToolchainError, OSError, ValueError, KeyError) as exc:
        print(f"AGENT TOOLCHAIN ERROR: {exc}", file=sys.stderr)
        return 2

    for warning in warnings:
        print(f"warning: {warning}")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 2
    managed = len([c for c in data["components"] if c.get("decision", {}).get("status") != "retired"])
    print(f"agent toolchain OK: {managed} components managed, {len(installed)} present in the repository, nothing unmanaged")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
