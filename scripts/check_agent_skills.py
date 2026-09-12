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

Point 4 is the one with teeth, and the one a first implementation gets wrong. A cited path may
legitimately contain a placeholder - `project/epics/*.json`, `project/evidence/<STORY-ID>/` -
so the named instance cannot be required to exist. The weak reading is "ignore anything with a
placeholder", which validates `docs/*totally/fake/path.md` as `docs` and lets an arbitrary broken
pointer through; walking the segments before the placeholder is barely better, since it accepts
the same token one segment deeper. What is implemented here is stronger: a citation carrying a
placeholder is **expanded as a glob and must match at least one real file**. A pattern is
unresolvable to one name, but it is not thereby unknowable.

Stdlib-only, like every other governance checker here.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS_DIR = ROOT / ".claude" / "skills"

# Directories that carry contract, and the root-level files a skill may cite. The root files are
# listed explicitly rather than matched by shape: `AGENTS.md` is the single most-cited target in
# the pack, and a regex anchored only on directories never checks it at all.
CONTRACT_DIRECTORIES = ("docs", "project", "scripts", "rust", "tests", "Formula", ".github")
ROOT_FILES = (
    "AGENTS.md",
    "CLAUDE.md",
    "CHANGELOG.md",
    "README.md",
    "LICENSE",
    "cancellai.py",
    "pyproject.toml",
    "requirements-dev.txt",
    ".pre-commit-config.yaml",
    ".gitignore",
)

# A citation may be written relative to the repository root (`docs/X.md`) or, inside a skill,
# relative to the skill's own directory (`../../AGENTS.md`). Both forms are matched; a leading
# `./` or `../` must be part of the token rather than terminating it, which is the bug that made
# every dot-relative citation invisible to the first implementation.
_DIRS = "|".join(re.escape(d) for d in CONTRACT_DIRECTORIES)
_ROOTS = "|".join(re.escape(f) for f in ROOT_FILES)
REPOSITORY_PATH = re.compile(rf"(?<![\w#?=-])((?:\.{{1,2}}/)*(?:(?:{_DIRS})/[\w./*<>$-]*[\w/*>]|(?:{_ROOTS})))")
SCRIPT_COMMAND = re.compile(r"python3\s+(scripts/[\w./-]+\.py)")
# A description must contain a "use <when-ish>" clause; the harness loads only name and
# description at discovery time, so this sentence is the whole activation contract.
WHEN_CLAUSE = re.compile(r"\buse (when|at|before|after|for|during)\b", re.IGNORECASE)

# The Agent Skills spec keeps descriptions short because every skill's description is resident
# in context for the whole session.
DESCRIPTION_MAX = 1024

# A path segment containing any of these stands for a class of file rather than one file.
PLACEHOLDER_MARKERS = ("*", "<", ">", "$")


class AgentSkillsError(RuntimeError):
    pass


def _strip_quotes(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


def parse_frontmatter(text: str) -> tuple[dict[str, str], str] | None:
    """Frontmatter mapping and body, or None when the block is missing or unterminated.

    Deliberately not a YAML parser - taking a YAML dependency would contradict the stdlib-only
    rule every other checker here follows - but it must not *disagree* with YAML on the shapes a
    skill author will actually write. So it understands quoted scalars and folded/literal block
    scalars (`>` / `|`), whose continuation lines are joined. Structurally richer fields such as
    `hooks` are skipped rather than misparsed; this checker inspects none of them.
    """
    text = text.lstrip("﻿")
    if not text.startswith("---\n"):
        return None
    end = text.find("\n---\n", 3)
    if end == -1:
        return None
    fields: dict[str, str] = {}
    lines = text[4:end].splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        index += 1
        if not line.strip() or line.startswith((" ", "\t", "-")) or ":" not in line:
            continue
        key, _, raw = line.partition(":")
        value = raw.strip()
        if value in (">", "|", ">-", "|-", ">+", "|+"):
            block: list[str] = []
            while index < len(lines) and (not lines[index].strip() or lines[index].startswith((" ", "\t"))):
                block.append(lines[index].strip())
                index += 1
            value = " ".join(part for part in block if part)
        fields[key.strip()] = _strip_quotes(value)
    return fields, text[end + len("\n---\n") :]


def _looks_like_a_path(token: str, base: Path) -> bool:
    """Whether a token is a pointer at all, rather than prose that happens to contain a slash.

    `docs/metadata` is how `AGENTS.md` writes the CR0 category in a sentence; it is not a path,
    and reporting it as a broken one trains a reader to ignore this checker. A token counts as a
    pointer when it names a file extension or already resolves to a directory.
    """
    head = token.split("#", 1)[0]
    if Path(head).suffix:
        return True
    return (base / head).is_dir()


def _resolve_base(token: str, skill: Path, root: Path) -> tuple[Path, str]:
    """The directory a citation is relative to, and the citation with its prefix removed."""
    if token.startswith("../"):
        return skill, token
    if token.startswith("./"):
        return root, token[2:]
    return root, token


def _as_glob(segment: str) -> str:
    """A path segment as a glob: a placeholder stands for one unknown name, nothing wider."""
    if not any(marker in segment for marker in PLACEHOLDER_MARKERS):
        return glob_escape(segment)
    # `<STORY-ID>` and `$VAR` name one unknown segment; a `*` already is a glob and keeps
    # whatever literal text surrounds it, so `*.json` stays narrower than `*`.
    collapsed = re.sub(r"<[^>]*>|\$\{[^}]*\}|\$\w+", "*", segment)
    return collapsed if "*" in collapsed else glob_escape(collapsed)


def glob_escape(literal: str) -> str:
    return re.sub(r"([*?\[\]])", r"[\1]", literal)


def _check_citation(token: str, skill: Path, root: Path) -> str | None:
    """The problem with one cited path, or None.

    A citation containing a placeholder cannot be resolved to one file, but it is not thereby
    unknowable: expanded as a glob it must still match *something*. That is what separates
    `project/epics/*.json`, which matches twenty-five files, from `docs/*totally/fake/path.md`,
    which matches nothing and is simply a broken pointer wearing a placeholder as a disguise.
    """
    target = token.split("#", 1)[0].rstrip(".,);:`")
    base, relative = _resolve_base(target, skill, root)
    if not _looks_like_a_path(relative, base):
        return None

    segments = [segment for segment in relative.split("/") if segment not in ("", ".")]
    if not segments:
        return None
    if any(any(marker in segment for marker in PLACEHOLDER_MARKERS) for segment in segments):
        pattern = "/".join(_as_glob(segment) for segment in segments)
        try:
            if next(base.glob(pattern), None) is None:
                return f"cites a pattern that matches nothing: {token}"
        except (OSError, ValueError, IndexError):
            return f"cites a pattern this checker cannot evaluate: {token}"
        return None

    resolved = base
    for segment in segments:
        resolved = resolved / segment
    if not resolved.exists():
        return f"cites a path that does not exist: {token}"
    return None


def check_skill(skill: Path, root: Path) -> list[str]:
    """Problems with one skill directory, as human-readable strings."""
    errors: list[str] = []
    manifest = skill / "SKILL.md"
    if not manifest.is_file():
        return [f"{skill.name}: no SKILL.md"]

    raw = manifest.read_text(encoding="utf-8")
    parsed = parse_frontmatter(raw)
    if parsed is None:
        if raw.lstrip("﻿").startswith("---\n"):
            return [f"{skill.name}: YAML frontmatter is not terminated by a closing `---`"]
        return [f"{skill.name}: no YAML frontmatter"]
    fields, body = parsed
    if not fields:
        return [f"{skill.name}: YAML frontmatter is empty"]

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
        problem = _check_citation(cited, skill, root)
        if problem:
            errors.append(f"{skill.name}: {problem}")

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
        raise AgentSkillsError(f"no agent skill pack at {skills_dir}")
    skills = skill_directories(skills_dir)
    if not skills:
        raise AgentSkillsError(f"{skills_dir} contains no skills")
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
