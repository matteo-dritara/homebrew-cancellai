#!/usr/bin/env python3
"""The agent skill pack points at the repository contract and never restates it (E24-S01).

`.claude/skills/` holds the procedures `AGENTS.md` and `docs/development/AGENT_PROTOCOL.md`
already define, in the open Agent Skills format so the same pack loads for the executor and for
the independent reviewer, which `AGENTS.md` assigns to a different agent.

A skill that copies a rule into its own prose becomes a second source of truth, and a second
source of truth drifts - the exact defect this repository already refuses for generated
documentation. The rule that prevents it is that a skill *points* at the contract; this checker
is what makes the rule enforced rather than aspirational, by requiring every pointer to resolve:

1. the file carries YAML frontmatter with `name` and `description`;
2. `name` matches its directory, so the skill cannot be addressed under two identities;
3. `description` says *when* the skill applies - a skill the agent cannot decide to load is
   dead weight in the discovery budget, since only name and description are loaded up front;
4. every repository path the body names still exists;
5. every `python3 scripts/*.py` command the body names still exists.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS_DIR = ROOT / ".claude" / "skills"

# Repository-relative paths a skill body may cite. Anchored on the directories that actually
# carry contract, so prose like "docs/metadata" in a sentence about categories is not mistaken
# for a path. Placeholder characters are captured as part of the token rather than terminating
# it, so `project/epics/*.json` arrives here whole and is recognised as a pattern instead of
# being truncated into a different, accidentally-valid path.
REPOSITORY_PATH = re.compile(r"(?<![\w/.-])((?:docs|project|scripts|rust|tests|Formula|\.github)/[\w./*<>$-]*[\w/*>])")
SCRIPT_COMMAND = re.compile(r"python3\s+(scripts/[\w./-]+\.py)")
# A description must contain a "use <when-ish>" clause; the harness loads only name and
# description at discovery time, so this sentence is the whole activation contract.
WHEN_CLAUSE = re.compile(r"\buse (when|at|before|after|for|during)\b", re.IGNORECASE)

# The Agent Skills spec keeps descriptions short because every skill's description is resident
# in context for the whole session.
DESCRIPTION_MAX = 1024

# A cited path may stand for a class of file rather than one file - `project/epics/*.json`,
# `project/evidence/<STORY-ID>/EVIDENCE.md`, `$CLAUDE_PROJECT_DIR/...`. The named instance
# cannot be required to exist, but the directory the pattern lives in can be, and must be:
# that is what catches a skill still pointing at a directory the repository has since moved.
PLACEHOLDER_MARKERS = ("*", "<", "$")


class AgentSkillsError(RuntimeError):
    pass


def parse_frontmatter(text: str) -> tuple[dict[str, str], str] | None:
    """Frontmatter mapping and body, or None when the block is missing or unterminated.

    Deliberately not a YAML parser: the fields this checker validates are flat scalars, and
    taking a YAML dependency for them would contradict the stdlib-only rule every other checker
    in this repository follows. Continuation and nested lines are skipped rather than
    misparsed, so a structurally richer field like `hooks` is simply not inspected here.
    """
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---\n", 4)
    if end == -1:
        return None
    fields: dict[str, str] = {}
    for line in text[4:end].splitlines():
        if line.startswith((" ", "\t", "-")) or ":" not in line:
            continue
        key, _, value = line.partition(":")
        fields[key.strip()] = value.strip()
    return fields, text[end + len("\n---\n") :]


def check_skill(skill: Path, root: Path) -> list[str]:
    """Problems with one skill directory, as human-readable strings."""
    errors: list[str] = []
    manifest = skill / "SKILL.md"
    if not manifest.is_file():
        return [f"{skill.name}: no SKILL.md"]

    parsed = parse_frontmatter(manifest.read_text(encoding="utf-8"))
    if parsed is None:
        return [f"{skill.name}: missing or unterminated YAML frontmatter"]
    fields, body = parsed

    name = fields.get("name", "")
    if not name:
        errors.append(f"{skill.name}: frontmatter has no `name`")
    elif name != skill.name:
        errors.append(f"{skill.name}: frontmatter name {name!r} does not match its directory")

    description = fields.get("description", "")
    if not description:
        errors.append(f"{skill.name}: frontmatter has no `description`")
    else:
        if len(description) > DESCRIPTION_MAX:
            errors.append(f"{skill.name}: description is {len(description)} characters (max {DESCRIPTION_MAX})")
        if not WHEN_CLAUSE.search(description):
            errors.append(f"{skill.name}: description does not say when the skill applies")

    for cited in sorted(set(REPOSITORY_PATH.findall(body))):
        target = cited.rstrip(".,);:`")
        marker = next((target.index(m) for m in PLACEHOLDER_MARKERS if m in target), None)
        if marker is not None:
            directory = target[:marker].rsplit("/", 1)[0]
            if directory and not (root / directory).is_dir():
                errors.append(f"{skill.name}: cites a pattern under a directory that does not exist: {target}")
            continue
        if not (root / target).exists():
            errors.append(f"{skill.name}: cites a path that does not exist: {target}")

    for script in sorted(set(SCRIPT_COMMAND.findall(body))):
        if not (root / script).is_file():
            errors.append(f"{skill.name}: names a script that does not exist: {script}")

    return errors


def skill_directories(skills_dir: Path) -> list[Path]:
    """Skill directories, in a stable order. A leading underscore marks pack-local support
    files rather than a skill, matching how the harness itself ignores them."""
    return sorted(path for path in skills_dir.iterdir() if path.is_dir() and not path.name.startswith((".", "_")))


def validate(skills_dir: Path, root: Path) -> int:
    """Number of skills checked. Raises AgentSkillsError with every problem found at once.

    A missing or empty pack is an error rather than a vacuous pass: this checker exists to
    prove the pack is present and consistent, and "zero skills are all valid" would report
    success for a deleted directory.
    """
    if not skills_dir.is_dir():
        raise AgentSkillsError(f"no agent skill pack at {skills_dir.relative_to(root)}")
    skills = skill_directories(skills_dir)
    if not skills:
        raise AgentSkillsError(f"{skills_dir.relative_to(root)} contains no skills")
    errors = [error for skill in skills for error in check_skill(skill, root)]
    if errors:
        raise AgentSkillsError("\n".join(errors))
    return len(skills)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Validate the cancellAI agent skill pack.")
    parser.add_argument("command", nargs="?", default="check", choices=["check"])
    return parser


def main(argv: list[str] | None = None) -> int:
    build_parser().parse_args(argv)
    try:
        count = validate(SKILLS_DIR, ROOT)
    except (AgentSkillsError, OSError, UnicodeError) as exc:
        print(f"AGENT SKILLS ERROR: {exc}", file=sys.stderr)
        return 2
    print(f"agent skills OK: {count} skills; frontmatter, names and every cited path resolve")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
