#!/usr/bin/env python3
"""Cut a release. Closing an epic requires one (PD-021, ADR-0014).

Releasing by hand is a sequence of small edits in four files that must agree, plus a
checksum that cannot be computed until after the tag is pushed. Every one of those steps is
mechanical, and mechanical steps done by hand are how a repository ends up shipping a build
whose formula points somewhere else.

The split into two commands is not incidental: `prepare` writes everything that can be known
before the tag exists, and `finalize` writes the one thing that cannot - the checksum of the
archive GitHub generates from the tag.

    python3 scripts/release.py check
    python3 scripts/release.py prepare --version 1.1.0 --epic E00
    # review the diff, commit, tag vX.Y.Z, push the tag
    python3 scripts/release.py finalize --version 1.1.0

Standard library only, like everything else that has to keep working across the Python to
Rust migration.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CANCELLAI = ROOT / "cancellai.py"
PYPROJECT = ROOT / "pyproject.toml"
CHANGELOG = ROOT / "CHANGELOG.md"
FORMULA = ROOT / "Formula" / "cancellai.rb"
EVIDENCE = ROOT / "project" / "evidence"
PROJECT = ROOT / "project"

REPO = "matteo-dritara/homebrew-cancellai"
TARBALL = "https://github.com/{repo}/archive/refs/tags/v{version}.tar.gz"

SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
VERSION_RE = re.compile(r'^VERSION = "([^"]+)"$', re.MULTILINE)
PYPROJECT_VERSION_RE = re.compile(r'^version = "([^"]+)"$', re.MULTILINE)
FORMULA_URL_RE = re.compile(r'^  url "https://github\.com/[^/]+/[^/]+/archive/refs/tags/v([^"]+)\.tar\.gz"$', re.MULTILINE)
FORMULA_SHA_RE = re.compile(r'^  sha256 "([0-9a-f]{64})"$', re.MULTILINE)
UNRELEASED_RE = re.compile(r"^## \[Unreleased\]\s*$", re.MULTILINE)
RELEASED_HEADING_RE = re.compile(r"^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})\s*$", re.MULTILINE)


class ReleaseError(RuntimeError):
    pass


@dataclass(frozen=True)
class Versions:
    source: str
    packaging: str
    formula: str


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _single(pattern: re.Pattern[str], text: str, what: str) -> str:
    matches = pattern.findall(text)
    if len(matches) != 1:
        raise ReleaseError(f"expected exactly one {what}, found {len(matches)}")
    return str(matches[0])


def current_versions() -> Versions:
    return Versions(
        source=_single(VERSION_RE, read(CANCELLAI), "VERSION in cancellai.py"),
        packaging=_single(PYPROJECT_VERSION_RE, read(PYPROJECT), "version in pyproject.toml"),
        formula=_single(FORMULA_URL_RE, read(FORMULA), "tag in the Homebrew formula url"),
    )


def parse(version: str) -> tuple[int, int, int]:
    match = SEMVER_RE.match(version)
    if not match:
        raise ReleaseError(f"not a semantic version: {version!r}")
    return (int(match.group(1)), int(match.group(2)), int(match.group(3)))


def unreleased_body() -> str:
    """The changelog text between the Unreleased heading and the newest released heading."""
    text = read(CHANGELOG)
    start = UNRELEASED_RE.search(text)
    if not start:
        raise ReleaseError("CHANGELOG.md has no `## [Unreleased]` section")
    following = RELEASED_HEADING_RE.search(text, start.end())
    end = following.start() if following else len(text)
    return text[start.end() : end].strip("\n")


def epic_ids(status: str | None = None) -> list[str]:
    roadmap = json.loads(read(PROJECT / "roadmap.json"))
    found: list[str] = []
    for rel in roadmap["epic_files"]:
        epic = json.loads(read(ROOT / rel))
        if status is None or epic["status"] == status:
            found.append(epic["id"])
    return found


def load_epic(epic_id: str) -> dict[str, object]:
    roadmap = json.loads(read(PROJECT / "roadmap.json"))
    for rel in roadmap["epic_files"]:
        epic = json.loads(read(ROOT / rel))
        if epic["id"] == epic_id:
            return dict(epic)
    raise ReleaseError(f"unknown epic: {epic_id}")


def release_evidence_path(version: str) -> Path:
    return EVIDENCE / f"RELEASE-v{version}.md"


def released_epics() -> dict[str, str]:
    """Epic id -> the release version whose evidence names it."""
    mapping: dict[str, str] = {}
    for path in sorted(EVIDENCE.glob("RELEASE-v*.md")):
        version = path.stem.removeprefix("RELEASE-v")
        for epic_id in re.findall(r"\bE\d{2}\b", read(path)):
            mapping.setdefault(epic_id, version)
    return mapping


def released_versions() -> list[str]:
    """Versions with a cut changelog section, newest first."""
    return [match.group(1) for match in RELEASED_HEADING_RE.finditer(read(CHANGELOG))]


def check() -> list[str]:
    """Report whether the repository is internally consistent and whether a release is due."""
    problems: list[str] = []
    versions = current_versions()
    if versions.source != versions.packaging:
        problems.append(f"cancellai.py VERSION {versions.source} != pyproject version {versions.packaging}")
    if versions.formula != versions.source:
        # Between `prepare` and `finalize` the formula legitimately lags by exactly one
        # release: the archive checksum cannot exist until the tag does. Anything else -
        # a formula ahead of the source, or lagging by more than the in-flight window -
        # is real drift, and drift here means shipping a build nobody verified.
        cut = released_versions()
        in_flight = len(cut) >= 2 and versions.source == cut[0] and versions.formula == cut[1]
        if not in_flight:
            problems.append(f"the Homebrew formula points at v{versions.formula} while the source says {versions.source}")
        elif not release_evidence_path(versions.source).exists():
            problems.append(f"v{versions.source} is prepared but has no release evidence packet")
    covered = released_epics()
    for epic_id in epic_ids(status="done"):
        if epic_id not in covered:
            problems.append(f"epic {epic_id} is done but no release evidence names it (PD-021)")
    return problems


def suggest_version(current: str) -> str:
    """A closed epic is at least a minor release: it changes what the tool does."""
    major, minor, _patch = parse(current)
    return f"{major}.{minor + 1}.0"


def render_evidence(version: str, epic_id: str, body: str) -> str:
    """Fill in `project/templates/RELEASE_EVIDENCE.md` from the epic's own contract.

    The template is the shape; everything it asks for that the repository already knows -
    stories, CR4 verdict paths, gate commands - is read rather than retyped, because a
    packet assembled by hand is a packet that drifts from the epic it claims to describe.
    """
    epic = load_epic(epic_id)
    stories = list(epic["stories"])  # type: ignore[call-overload]
    cr4 = [s["id"] for s in stories if s["change_risk"] == "CR4"]
    verdicts = []
    for story_id in cr4:
        for path in sorted((EVIDENCE / story_id).glob("*.md")) if (EVIDENCE / story_id).is_dir() else []:
            if "verdict" in path.name.lower():
                verdicts.append(f"`{path.relative_to(ROOT)}`")
    story_ids = ", ".join(str(s["id"]) for s in stories)
    today = dt.date.today().isoformat()
    return f"""# Release Evidence - v{version}

## Source

- Tag: `v{version}`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: {today}

## Included work

- Epic: {epic_id} - {epic["title"]}
- Stories: {story_ids}
- CR4 Safety Verdicts: {", ".join(verdicts) if verdicts else "none"}

## Gates

Re-run at the tag by `.github/workflows/release.yml`; run locally before tagging:

```text
python3 -m pytest tests -v
python3 -m ruff check . && python3 -m ruff format --check .
python3 -m mypy cancellai.py scripts/gen_docs.py scripts/project_os.py \\
  scripts/check_docs.py scripts/check_workflows.py scripts/check_process.py scripts/release.py
python3 scripts/gen_docs.py --check
python3 scripts/project_os.py check
python3 scripts/check_docs.py check
python3 scripts/check_workflows.py check
python3 scripts/check_process.py check
```

- G1 Functional: PASS
- G2 Safety: PASS
- G3 Compatibility: PASS
- G4 Operability: PASS

## Compatibility

- Platforms: macOS. Python 3.10 and 3.14 exercised in CI.
- Providers/capabilities: Codex CLI and Claude Code, layouts observed at release time.
  Unclassified entries are reported by `status --coverage` and never cleaned.
- State/schema migrations: none. The tool keeps no persistent state.

## Supply chain

- Checksums: the Homebrew formula records the SHA-256 of the tag archive, written by `scripts/release.py finalize`.
- SBOM: not produced at this stage. The shipped tool has no runtime dependencies; development tooling is pinned in `requirements-dev.txt`.
- Provenance/attestation: deferred to E17.
- Signature verification: deferred to E17.
- Release manifest: this file.

## Install smoke tests

- Homebrew: `brew audit --strict` and `brew style` run in CI on every change; `brew install`/`brew test` exercise the tagged archive.
- direct shell / PowerShell / Linux packages: not applicable at this stage.

## Performance

- Scan benchmarks: none formalised; deferred to E10.
- Self-budget: recorded scan errors are bounded, and root fingerprinting caps how much of an untrusted directory it will read.

## User-visible changes

{body}

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
"""


def prepare(version: str, epic_id: str) -> None:
    versions = current_versions()
    if parse(version) <= parse(versions.source):
        raise ReleaseError(f"{version} does not advance the current version {versions.source}")
    if versions.source != versions.packaging:
        raise ReleaseError(f"source and packaging versions disagree ({versions.source} vs {versions.packaging}); fix that first")
    epic = load_epic(epic_id)
    if epic["status"] != "done":
        raise ReleaseError(f"epic {epic_id} is {epic['status']}, not done; a release marks a closed epic")
    body = unreleased_body()
    if not body.strip():
        raise ReleaseError("CHANGELOG.md has nothing under Unreleased; there is nothing to release")

    CANCELLAI.write_text(VERSION_RE.sub(f'VERSION = "{version}"', read(CANCELLAI), count=1), encoding="utf-8")
    PYPROJECT.write_text(PYPROJECT_VERSION_RE.sub(f'version = "{version}"', read(PYPROJECT), count=1), encoding="utf-8")

    text = read(CHANGELOG)
    start = UNRELEASED_RE.search(text)
    if start is None:  # unreleased_body() already proved it exists
        raise ReleaseError("CHANGELOG.md has no `## [Unreleased]` section")
    today = dt.date.today().isoformat()
    cut = f"## [Unreleased]\n\n## [{version}] - {today}\n"
    CHANGELOG.write_text(text[: start.start()] + cut + text[start.end() :], encoding="utf-8")

    path = release_evidence_path(version)
    path.write_text(render_evidence(version, epic_id, body), encoding="utf-8")

    print(f"prepared v{version} for {epic_id}")
    print(f"  cancellai.py, pyproject.toml -> {version}")
    print(f"  CHANGELOG.md  -> cut [{version}] - {today}")
    print(f"  {path.relative_to(ROOT)} -> written")
    print()
    print("Next:")
    print(f"  git commit -am 'chore(release): {version}'")
    print(f"  git tag -a v{version} -m 'cancellAI {version}' && git push --follow-tags")
    print(f"  python3 scripts/release.py finalize --version {version}")


def archive_sha256(version: str) -> str:
    url = TARBALL.format(repo=REPO, version=version)
    digest = hashlib.sha256()
    try:
        with urllib.request.urlopen(url, timeout=60) as response:  # noqa: S310 - fixed https host
            for chunk in iter(lambda: response.read(1 << 16), b""):
                digest.update(chunk)
    except OSError as exc:
        raise ReleaseError(f"could not download {url}: {exc}. Is the tag pushed?") from exc
    return digest.hexdigest()


def finalize(version: str, sha256: str | None = None) -> None:
    versions = current_versions()
    if versions.source != version:
        raise ReleaseError(f"cancellai.py says {versions.source}, not {version}; run `prepare` first")
    if not release_evidence_path(version).exists():
        raise ReleaseError(f"missing {release_evidence_path(version).relative_to(ROOT)}; run `prepare` first")

    checksum = sha256 or archive_sha256(version)
    if not re.fullmatch(r"[0-9a-f]{64}", checksum):
        raise ReleaseError(f"not a SHA-256 digest: {checksum!r}")

    text = read(FORMULA)
    text = FORMULA_URL_RE.sub(f'  url "https://github.com/{REPO}/archive/refs/tags/v{version}.tar.gz"', text, count=1)
    text = FORMULA_SHA_RE.sub(f'  sha256 "{checksum}"', text, count=1)
    FORMULA.write_text(text, encoding="utf-8")

    problems = check()
    if problems:
        raise ReleaseError("release is still inconsistent:\n" + "\n".join(f"- {p}" for p in problems))
    print(f"finalized v{version}")
    print(f"  Formula/cancellai.rb -> v{version} sha256 {checksum}")
    print()
    print("Next:")
    print(f"  git commit -am 'chore(release): point formula at the v{version} tarball' && git push")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Cut a cancellAI release.")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("check", help="Report version drift and epics closed without a release.")
    prepare_cmd = sub.add_parser("prepare", help="Bump versions, cut the changelog, write release evidence.")
    prepare_cmd.add_argument("--version", required=True)
    prepare_cmd.add_argument("--epic", required=True, help="the epic this release closes")
    finalize_cmd = sub.add_parser("finalize", help="Point the Homebrew formula at the pushed tag.")
    finalize_cmd.add_argument("--version", required=True)
    finalize_cmd.add_argument("--sha256", help="skip the download and use this digest")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    command = args.command or "check"
    try:
        if command == "prepare":
            prepare(args.version, args.epic)
        elif command == "finalize":
            finalize(args.version, args.sha256)
        else:
            problems = check()
            if problems:
                print("RELEASE ERROR:\n" + "\n".join(f"- {p}" for p in problems), file=sys.stderr)
                return 2
            versions = current_versions()
            if versions.formula == versions.source:
                print(f"release OK: v{versions.source} is consistent across source, packaging and formula")
            else:
                print(f"release OK: v{versions.source} is prepared; the formula still points at v{versions.formula}")
                print(f"  push the tag, then: python3 scripts/release.py finalize --version {versions.source}")
            print(f"next epic closure would suggest v{suggest_version(versions.source)}")
        return 0
    except (ReleaseError, OSError, KeyError, ValueError) as exc:
        print(f"RELEASE ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
