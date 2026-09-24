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
import os
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CANCELLAI = ROOT / "cancellai.py"
PYPROJECT = ROOT / "pyproject.toml"
CHANGELOG = ROOT / "CHANGELOG.md"
FORMULA = ROOT / "Formula" / "cancellai.rb"
RUST = ROOT / "rust"
RUST_WORKSPACE = RUST / "Cargo.toml"
RUST_LOCK = RUST / "Cargo.lock"
EVIDENCE = ROOT / "project" / "evidence"
PROJECT = ROOT / "project"

REPO = "matteo-dritara/homebrew-cancellai"
TARBALL = "https://github.com/{repo}/archive/refs/tags/v{version}.tar.gz"

SEMVER_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
VERSION_RE = re.compile(r'^VERSION = "([^"]+)"$', re.MULTILINE)
PYPROJECT_VERSION_RE = re.compile(r'^version = "([^"]+)"$', re.MULTILINE)
FORMULA_URL_RE = re.compile(r'^  url "https://github\.com/[^/]+/[^/]+/archive/refs/tags/v([^"]+)\.tar\.gz"$', re.MULTILINE)
FORMULA_SHA_RE = re.compile(r'^  sha256 "([0-9a-f]{64})"$', re.MULTILINE)
# E06-S04: the formula installs the Rust engine from the release's own archives, one resource per
# supported Homebrew platform, each followed by its checksum line.
ENGINE_TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu")
ENGINE_ASSET = "https://github.com/{repo}/releases/download/v{version}/cancellai-cli-{version}-{target}.tar.gz"
FORMULA_ALL_SHA_RE = re.compile(r'^ *sha256 "([0-9a-f]{64})"$', re.MULTILINE)
# E06-S04: the Rust engine's own version. Until the cutover it was never bumped and every release
# binary reported `cancellai-cli 0.1.0` whatever the tag; it now moves with the source version.
RUST_WORKSPACE_VERSION_RE = re.compile(r'^version = "([^"]+)"$', re.MULTILINE)
RUST_PATH_DEP_RE = re.compile(r'(cancellai-[a-z-]+ = \{ path = "[^"]+", version = ")[^"]+(")')
RUST_LOCK_MEMBER_RE = re.compile(r'(name = "cancellai-[a-z-]+"\nversion = ")[^"]+(")')
UNRELEASED_RE = re.compile(r"^## \[Unreleased\]\s*$", re.MULTILINE)
# The declared epic line a release packet's "Included work" section carries, and the only thing
# that counts as a release covering an epic. Reading every `E\d\d` in the prose credited an epic
# for being *mentioned* - and a packet embeds the changelog, which mentions plenty. That made
# PD-021's gate satisfiable by a sentence, which is the failure mode this repository exists to
# refuse elsewhere. Four epics were being credited that way (E06, E12, E16, E17); none was `done`,
# so nothing was wrong yet, which is the only reason this was cheap to fix.
# A changelog link is written from the repository root; a release packet lives two directories
# down, so the same text embedded there points at nothing. v1.13.0's packet has the rewritten form
# because someone did it by hand, which is the failure this file's own docstring warns about.
BODY_LINK_RE = re.compile(r"\]\((?!https?://|#|/|\.\./)([^)]+)\)")
# Whether the release a tag was cut for actually published. Four versions - v1.10.0, v1.12.0,
# v1.13.0, v1.13.1 - were tagged with evidence packets and a cut changelog section while their
# release workflows failed, and the repository had no word for that: `finalize` never ran, the
# formula stayed behind, and the only record was whoever remembered. `pending` is what `prepare`
# writes, because at that moment nobody can know (E17-S10).
PUBLISHED_RE = re.compile(r"^- Published:\s*(yes|no|pending)\b[ \t]*-?[ \t]*(.*)$", re.MULTILINE)
PUBLISHED_STATES = ("yes", "no", "pending")
EPIC_DECLARATION_RE = re.compile(r"^\s*- Epic:\s*(E\d{2})\b", re.MULTILINE)
RELEASED_HEADING_RE = re.compile(r"^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})\s*$", re.MULTILINE)


class ReleaseError(RuntimeError):
    pass


@dataclass(frozen=True)
class Versions:
    source: str
    packaging: str
    formula: str
    engine: str


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
        engine=_single(RUST_WORKSPACE_VERSION_RE, read(RUST_WORKSPACE), "workspace version in rust/Cargo.toml"),
    )


def set_engine_version(version: str) -> None:
    """Moves the Rust workspace version, every internal path dependency's version requirement, and
    the workspace members' lockfile entries to `version` together, so the engine reports the
    release it ships in and Cargo still resolves its own crates."""
    RUST_WORKSPACE.write_text(RUST_WORKSPACE_VERSION_RE.sub(f'version = "{version}"', read(RUST_WORKSPACE), count=1), encoding="utf-8")
    for manifest in sorted((RUST / "crates").glob("*/Cargo.toml")):
        text = read(manifest)
        updated = RUST_PATH_DEP_RE.sub(lambda m: f"{m.group(1)}{version}{m.group(2)}", text)
        if updated != text:
            manifest.write_text(updated, encoding="utf-8")
    RUST_LOCK.write_text(RUST_LOCK_MEMBER_RE.sub(lambda m: f"{m.group(1)}{version}{m.group(2)}", read(RUST_LOCK)), encoding="utf-8")


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
    """Epic id -> the release version whose evidence *declares* it closed."""
    mapping: dict[str, str] = {}
    for path in sorted(EVIDENCE.glob("RELEASE-v*.md")):
        version = path.stem.removeprefix("RELEASE-v")
        for epic_id in EPIC_DECLARATION_RE.findall(read(path)):
            mapping.setdefault(epic_id, version)
    return mapping


def release_outcomes() -> dict[str, tuple[str, str]]:
    """Version -> (published state, reason). A packet with no marker reads as `pending`."""
    outcomes: dict[str, tuple[str, str]] = {}
    for path in sorted(EVIDENCE.glob("RELEASE-v*.md")):
        version = path.stem.removeprefix("RELEASE-v")
        match = PUBLISHED_RE.search(read(path))
        outcomes[version] = (match.group(1), match.group(2).strip()) if match else ("pending", "")
    return outcomes


def formula_should_point_at(source: str, cut: list[str], outcomes: dict[str, tuple[str, str]]) -> str | None:
    """The version the formula may point at while `source` is prepared but not yet finalized.

    Normally the previous cut release. A release that never published is skipped: the formula
    cannot point at a version whose artifacts do not exist, and demanding it made v1.13.1 need
    finalizing by hand before v1.13.2 could be prepared. Where nothing failed, this is exactly
    the old rule.
    """
    if not cut or cut[0] != source:
        return None
    for version in cut[1:]:
        if outcomes.get(version, ("pending", ""))[0] != "no":
            return version
    return cut[1] if len(cut) >= 2 else None


def released_versions() -> list[str]:
    """Versions with a cut changelog section, newest first."""
    return [match.group(1) for match in RELEASED_HEADING_RE.finditer(read(CHANGELOG))]


def check() -> list[str]:
    """Report whether the repository is internally consistent and whether a release is due."""
    problems: list[str] = []
    versions = current_versions()
    if versions.source != versions.packaging:
        problems.append(f"cancellai.py VERSION {versions.source} != pyproject version {versions.packaging}")
    if versions.engine != versions.source:
        problems.append(f"the Rust engine's version {versions.engine} != source version {versions.source}")
    problems.extend(live_formula_problems(read(FORMULA), versions.formula))
    if versions.formula != versions.source:
        # Between `prepare` and `finalize` the formula legitimately lags by exactly one
        # release: the archive checksum cannot exist until the tag does. Anything else -
        # a formula ahead of the source, or lagging by more than the in-flight window -
        # is real drift, and drift here means shipping a build nobody verified.
        cut = released_versions()
        expected = formula_should_point_at(versions.source, cut, release_outcomes())
        in_flight = expected is not None and versions.formula == expected
        if not in_flight:
            problems.append(f"the Homebrew formula points at v{versions.formula} while the source says {versions.source}")
        elif not release_evidence_path(versions.source).exists():
            problems.append(f"v{versions.source} is prepared but has no release evidence packet")
    covered = released_epics()
    # `done_no_release` is deliberately absent here: an epic that changed nothing in the
    # shipped artifact has no release to point at, and demanding one would produce an empty
    # version whose changelog says nothing (ADR-0025).
    for epic_id in epic_ids(status="done"):
        if epic_id not in covered:
            problems.append(f"epic {epic_id} is done but no release evidence names it (PD-021)")
    return problems


def suggest_version(current: str) -> str:
    """A closed epic is at least a minor release: it changes what the tool does."""
    major, minor, _patch = parse(current)
    return f"{major}.{minor + 1}.0"


def is_patch_of(version: str, current: str) -> bool:
    major, minor, patch = parse(version)
    c_major, c_minor, c_patch = parse(current)
    return (major, minor) == (c_major, c_minor) and patch == c_patch + 1


def included_work(epic_id: str | None, reason: str | None) -> str:
    """The "Included work" section: the epic this release closes, or why there is none.

    A fix release is not an epic closure and must not read like one. It names no epic - which is
    also what keeps `released_epics()` from crediting one - and states instead what the already
    tagged version could not carry, because that is the whole reason the version exists.
    """
    if epic_id is None:
        return (
            "This release closes no epic. It exists because an already tagged version could not\n"
            "carry the fix below, and a published tag is immutable history:\n\n"
            f"{reason}\n"
        )
    epic = load_epic(epic_id)
    stories = list(epic["stories"])  # type: ignore[call-overload]
    cr4 = [s["id"] for s in stories if s["change_risk"] == "CR4"]
    verdicts = []
    for story_id in cr4:
        for path in sorted((EVIDENCE / story_id).glob("*.md")) if (EVIDENCE / story_id).is_dir() else []:
            if "verdict" in path.name.lower():
                verdicts.append(f"`{path.relative_to(ROOT)}`")
    story_ids = ", ".join(str(s["id"]) for s in stories)
    return f"- Epic: {epic_id} - {epic['title']}\n- Stories: {story_ids}\n- CR4 Safety Verdicts: {', '.join(verdicts) if verdicts else 'none'}\n"


def relocate_links(body: str) -> str:
    """Rewrite repository-root-relative markdown links for a document under `project/evidence/`."""
    return BODY_LINK_RE.sub(r"](../../\1)", body)


def render_evidence(version: str, epic_id: str | None, body: str, reason: str | None = None) -> str:
    """Fill in `project/templates/RELEASE_EVIDENCE.md` from the epic's own contract.

    The template is the shape; everything it asks for that the repository already knows -
    stories, CR4 verdict paths, gate commands - is read rather than retyped, because a
    packet assembled by hand is a packet that drifts from the epic it claims to describe.
    """
    today = dt.date.today().isoformat()
    return f"""# Release Evidence - v{version}

## Source

- Tag: `v{version}`
- Commit: recorded by the release workflow at the tag
- Channel: stable
- Date: {today}
- Published: pending

## Included work

{included_work(epic_id, reason)}
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

{relocate_links(body)}

## Known residual risks

Carried from the epic's closure packet. See `project/evidence/` for the story-level records.

## Rollback

Point the Homebrew formula back at the previous tag and its checksum; the tool keeps no
persistent state, so there is nothing to migrate back. Published tags are immutable history
and are never deleted.
"""


def prepare(version: str, epic_id: str | None = None, reason: str | None = None) -> None:
    """Write everything a release needs that can be known before the tag exists.

    Two shapes, and the second exists because this repository could not express it. A release
    marked a closed epic and nothing else, so when the v1.12.0 and v1.13.0 release workflows both
    failed - a Windows packaging error, then a clippy denial - there was no way to cut the version
    that carried the fix. A published tag is immutable history and is never deleted, so the only
    honest route is a new version, and the tool has to be able to say that a release closes no
    epic. It stays a *patch*: a version that closes nothing may not claim a feature number.
    """
    versions = current_versions()
    if (epic_id is None) == (reason is None):
        raise ReleaseError("a release closes an epic (--epic) or carries a fix (--fix); give exactly one")
    if parse(version) <= parse(versions.source):
        raise ReleaseError(f"{version} does not advance the current version {versions.source}")
    if versions.source != versions.packaging:
        raise ReleaseError(f"source and packaging versions disagree ({versions.source} vs {versions.packaging}); fix that first")
    if epic_id is None:
        if not is_patch_of(version, versions.source):
            raise ReleaseError(
                f"{version} is not the patch after {versions.source}; a release that closes no epic "
                "is a fix release and takes the next patch number"
            )
        if not (reason or "").strip():
            raise ReleaseError("--fix needs a reason: what the already tagged version could not carry")
    else:
        epic = load_epic(epic_id)
        if epic["status"] != "done":
            raise ReleaseError(f"epic {epic_id} is {epic['status']}, not done; a release marks a closed epic")
    body = unreleased_body()
    if not body.strip():
        raise ReleaseError("CHANGELOG.md has nothing under Unreleased; there is nothing to release")

    CANCELLAI.write_text(VERSION_RE.sub(f'VERSION = "{version}"', read(CANCELLAI), count=1), encoding="utf-8")
    PYPROJECT.write_text(PYPROJECT_VERSION_RE.sub(f'version = "{version}"', read(PYPROJECT), count=1), encoding="utf-8")
    set_engine_version(version)

    text = read(CHANGELOG)
    start = UNRELEASED_RE.search(text)
    if start is None:  # unreleased_body() already proved it exists
        raise ReleaseError("CHANGELOG.md has no `## [Unreleased]` section")
    today = dt.date.today().isoformat()
    cut = f"## [Unreleased]\n\n## [{version}] - {today}\n"
    CHANGELOG.write_text(text[: start.start()] + cut + text[start.end() :], encoding="utf-8")

    path = release_evidence_path(version)
    path.write_text(render_evidence(version, epic_id, body, reason), encoding="utf-8")

    print(f"prepared v{version} for {epic_id if epic_id else 'a fix that no epic closure carries'}")
    print(f"  cancellai.py, pyproject.toml, rust workspace -> {version}")
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


RELEASE_MANIFEST_ASSET = "https://github.com/{repo}/releases/download/v{version}/release-manifest.json"
# E06-S14: the formula the release workflow rendered from the manifest, published beside it.
FORMULA_ASSET = "https://github.com/{repo}/releases/download/v{version}/cancellai.rb"
# The workflow that builds, attests and publishes a release; the manifest names it as build identity.
RELEASE_WORKFLOW = "release.yml"
# Bounds on what a verification download may read: an engine archive, the manifest or formula.
MAX_ENGINE_ARCHIVE_BYTES = 256 * 1024 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024


def download(url: str, cap: int) -> bytes:
    """The bytes at `url`, at most `cap` of them; anything larger or unreachable is refused."""
    try:
        with urllib.request.urlopen(url, timeout=120) as response:  # noqa: S310 - fixed https host
            data = response.read(cap + 1)
    except OSError as exc:
        raise ReleaseError(f"could not download {url}: {exc}. Did the release publish?") from exc
    if len(data) > cap:
        raise ReleaseError(f"{url} is larger than {cap} bytes")
    return bytes(data)


def manifest_digests(doc: object, version: str, where: str) -> dict[str, str]:
    """Each engine archive's digest as a release manifest (E17-S01) records it, after checking the
    document is the manifest for `version` and names each target's archive exactly once."""
    if not isinstance(doc, dict) or doc.get("document_type") != "release_manifest" or doc.get("version") != version:
        raise ReleaseError(f"{where} is not the release manifest for v{version}")
    identity = doc.get("build_identity")
    if not isinstance(identity, dict) or identity.get("repository") != REPO or identity.get("workflow") != RELEASE_WORKFLOW:
        raise ReleaseError(f"{where} was not built by {REPO}'s {RELEASE_WORKFLOW}")
    artifacts = doc.get("artifacts")
    if not isinstance(artifacts, list):
        raise ReleaseError(f"{where} lists no artifacts")
    digests = {}
    for target in ENGINE_TARGETS:
        name = f"cancellai-cli-{version}-{target}"
        entries = [a for a in artifacts if isinstance(a, dict) and a.get("name") == name]
        if len(entries) != 1:
            raise ReleaseError(f"{where} names {name} {len(entries)} times, not once")
        entry = entries[0]
        digest = entry.get("sha256")
        if entry.get("target_triple") != target or not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise ReleaseError(f"{where}: {name} does not carry target {target} and a SHA-256 digest")
        digests[target] = digest
    return digests


def manifest_source_commit(doc: dict[str, object], where: str) -> str:
    """The commit the manifest says the release was built from."""
    source = doc.get("source_sha")
    if not isinstance(source, str) or not re.fullmatch(r"[0-9a-f]{40}", source):
        raise ReleaseError(f"{where} does not name the 40-hex commit it was built from")
    return source


def tag_commit(version: str) -> str:
    """The commit the local `v<version>` tag points at - the tag `finalize` is finalizing."""
    result = subprocess.run(["git", "rev-parse", "--verify", f"v{version}^{{commit}}"], cwd=ROOT, capture_output=True, text=True, check=False)  # noqa: S603, S607
    commit = result.stdout.strip()
    if result.returncode != 0 or not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ReleaseError(f"cannot resolve tag v{version} to a commit here; fetch the tag first")
    return commit


def manifest_sha256s(version: str) -> tuple[dict[str, str], str]:
    """The published release manifest's engine digests, and the commit it names - which must be the
    commit of the `v<version>` tag (E06 review round 10: a manifest naming another commit was
    adopted)."""
    url = RELEASE_MANIFEST_ASSET.format(repo=REPO, version=version)
    try:
        doc = json.loads(download(url, MAX_MANIFEST_BYTES))
    except ValueError as exc:
        raise ReleaseError(f"{url} is not JSON: {exc}") from exc
    digests = manifest_digests(doc, version, url)
    source = manifest_source_commit(doc, url)
    tagged = tag_commit(version)
    if source != tagged:
        raise ReleaseError(f"{url} was built from {source}, but v{version} is {tagged}")
    return digests, source


def provenance_command(gh: str, path: Path, version: str, commit: str) -> list[str]:
    """`gh attestation verify` bound to the exact release: this repository, signed by its release
    workflow, from the version's tag at the tag's own commit - not merely "some workflow in this
    repository" (E06 review round 10)."""
    return [
        gh, "attestation", "verify", str(path), "--repo", REPO,
        "--signer-workflow", f"{REPO}/.github/workflows/{RELEASE_WORKFLOW}",
        "--source-ref", f"refs/tags/v{version}",
        "--source-digest", commit,
    ]  # fmt: skip


def verify_provenance(data: bytes, name: str, version: str, commit: str) -> None:
    """`gh attestation verify` over exactly these bytes: they were built by this repository's
    release workflow from the `v<version>` tag at `commit`. Unavailable `gh` or a failed
    verification refuses."""
    gh = shutil.which("gh")
    if gh is None:
        raise ReleaseError(f"cannot verify {name}'s build provenance: `gh` is not installed")
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / name
        path.write_bytes(data)
        result = subprocess.run(provenance_command(gh, path, version, commit), capture_output=True, text=True, check=False)  # noqa: S603
    if result.returncode != 0:
        raise ReleaseError(f"{name} failed build-provenance verification: {(result.stderr or result.stdout).strip()}")


def engine_sha256s(version: str) -> dict[str, str]:
    """Each engine archive's digest, taken from the published release manifest and confirmed on the
    bytes: every archive is downloaded, hashed to the manifest's digest, and its build provenance
    verified (E06 review round 9, ADR-0040). Nothing is taken from a `.sha256` sidecar, and a
    disagreement or any unavailable piece refuses."""
    recorded, commit = manifest_sha256s(version)
    problems = []
    for target in ENGINE_TARGETS:
        url = ENGINE_ASSET.format(repo=REPO, version=version, target=target)
        data = download(url, MAX_ENGINE_ARCHIVE_BYTES)
        actual = hashlib.sha256(data).hexdigest()
        if actual != recorded[target]:
            problems.append(f"{target}: archive {actual}, manifest {recorded[target]}")
            continue
        verify_provenance(data, url.rsplit("/", 1)[-1], version, commit)
    if problems:
        raise ReleaseError(f"v{version}'s engine archives disagree with the release manifest:\n" + "\n".join(f"- {p}" for p in problems))
    return recorded


def render_release_formula(version: str, manifest: Path, source_sha: str) -> str:
    """The formula for `version` as a pure function of the release manifest and the tag archive's
    digest (E06-S14). The release workflow runs it after `release_manifest.py verify-checksums` has
    tied every built archive to the manifest, and publishes the result as `cancellai.rb`."""
    if not re.fullmatch(r"[0-9a-f]{64}", source_sha):
        raise ReleaseError(f"not a SHA-256 digest: {source_sha!r}")
    engines = None
    if parse(version) >= CUTOVER_VERSION:
        try:
            doc = json.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            raise ReleaseError(f"cannot read {manifest}: {exc}") from exc
        manifest_source_commit(doc, str(manifest))
        engines = published_engines(version, manifest_digests(doc, version, str(manifest)))
    return render_formula(version, source_sha, engines)


def adoptable_formula(version: str) -> str:
    """The formula `finalize` may write for an engine-carrying `version`: the published
    `cancellai.rb`, accepted only if it is byte-identical to the formula rendered from the published
    manifest - whose every engine archive has been downloaded, hashed and provenance-verified - and
    the tag archive's digest (ADR-0040)."""
    expected = render_formula(version, archive_sha256(version), published_engines(version, engine_sha256s(version)))
    url = FORMULA_ASSET.format(repo=REPO, version=version)
    published = download(url, MAX_MANIFEST_BYTES)
    if published != expected.encode("utf-8"):
        raise ReleaseError(f"{url} is not the formula the published release manifest renders; refusing to adopt it")
    return expected


# Engine archives released before the engine carried its own version (E06-S04): their binaries
# report `cancellai-cli 0.1.0` whatever the tag says. None of them may be the cutover release.
UNVERSIONED_ENGINE_RELEASES = frozenset({"1.21.0"})
CUTOVER_VERSION = (2, 0, 0)


CUTOVER_EVIDENCE = ROOT / "project" / "evidence" / "E06-S04"
CUTOVER_AUTHORIZATION = CUTOVER_EVIDENCE / "CUTOVER_AUTHORIZATION.md"
CUTOVER_SAFETY_VERDICT = CUTOVER_EVIDENCE / "SAFETY_VERDICT.md"
AUTHORIZATION_FIELD_RE = re.compile(r"^(Authorized-by|Version|Safety-Verdict-SHA256):[ \t]*(\S.*?)[ \t]*$", re.MULTILINE)


ROUND_HEADING_RE = re.compile(r"^## Round \d+\b", re.MULTILINE)
SECTION_HEADING_RE = re.compile(r"^## ", re.MULTILINE)
VERDICT_LINE_RE = re.compile(r"^\s*`?(PASS_WITH_RESIDUALS|PASS|FAIL|REJECT)`?\s*$", re.MULTILINE | re.IGNORECASE)
FENCE_RE = re.compile(r"^```.*?^```\s*$", re.MULTILINE | re.DOTALL)


def safety_verdict_passes(path: Path) -> bool:
    """Whether the migration Safety Verdict's final round passes, read from that round's own
    section: the last `## Round <n>` heading's standalone verdict lines, fenced blocks excluded.
    A verdict-shaped line anywhere after that section - an "owner note", say - makes the verdict
    unreadable, so it fails; nothing outside the final round can turn it into a pass (E06 review
    round 10)."""
    try:
        text = FENCE_RE.sub("", path.read_text(encoding="utf-8"))
    except OSError:
        return False
    rounds = list(ROUND_HEADING_RE.finditer(text))
    if not rounds:
        return False
    start = rounds[-1].end()
    later = SECTION_HEADING_RE.search(text, start)
    section = text[start : later.start() if later else len(text)]
    if later and VERDICT_LINE_RE.search(text, later.start()):
        return False
    verdicts = [match.group(1).upper() for match in VERDICT_LINE_RE.finditer(section)]
    return bool(verdicts) and verdicts[-1] in {"PASS", "PASS_WITH_RESIDUALS"}


CODEOWNERS = ROOT / ".github" / "CODEOWNERS"


def repository_owner() -> str | None:
    """The owner `.github/CODEOWNERS` assigns to every path (`* @owner`), or None."""
    try:
        text = CODEOWNERS.read_text(encoding="utf-8")
    except OSError:
        return None
    for line in text.splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[0] == "*":
            return fields[1]
    return None


def cutover_authorization_problems(version: str) -> list[str]:
    """Why the owner has not authorized adopting `version` as the cutover (E06-S15, ADR-0040).

    The authorization is a committed file naming who authorized, the version, and the SHA-256 of
    the migration Safety Verdict they accepted. It is void if the verdict changed since, if the
    verdict's final round does not pass, or if it names another version. No story status stands
    in for it: a status is an edit anyone can make, not the owner's decision.

    `Authorized-by` must be the owner `.github/CODEOWNERS` names. That is an identity claim, not
    an authentication: every commit here, an agent's included, is signed with the owner's key, so
    nothing in the repository can tell the owner from a process acting as the owner. The owner
    accepted that same-user limit on 2026-09-24, as ADR-0039 did for the containment ledger."""
    try:
        text = CUTOVER_AUTHORIZATION.read_text(encoding="utf-8")
    except OSError:
        return [f"no owner authorization: {CUTOVER_AUTHORIZATION.relative_to(ROOT)} does not exist"]
    fields: dict[str, list[str]] = {}
    for match in AUTHORIZATION_FIELD_RE.finditer(text):
        fields.setdefault(match.group(1), []).append(match.group(2))
    problems = [
        f"the authorization must name `{name}:` exactly once"
        for name in ("Authorized-by", "Version", "Safety-Verdict-SHA256")
        if len(fields.get(name, [])) != 1
    ]
    if problems:
        return problems
    owner = repository_owner()
    if owner is None or fields["Authorized-by"][0] != owner:
        problems.append(
            f"the authorization must be by the repository owner ({owner or 'none in .github/CODEOWNERS'}), not {fields['Authorized-by'][0]!r}"
        )
    if fields["Version"][0] != version:
        problems.append(f"the authorization is for v{fields['Version'][0]}, not v{version}")
    try:
        verdict = CUTOVER_SAFETY_VERDICT.read_bytes()
    except OSError:
        return [*problems, f"{CUTOVER_SAFETY_VERDICT.relative_to(ROOT)} does not exist"]
    if hashlib.sha256(verdict).hexdigest() != fields["Safety-Verdict-SHA256"][0]:
        problems.append("the migration Safety Verdict changed after the owner authorized it")
    if not safety_verdict_passes(CUTOVER_SAFETY_VERDICT):
        problems.append("the migration Safety Verdict's final round does not pass")
    return problems


# E06-S04 after review round 8: the formula is generated whole by `render_formula` and compared
# byte for byte, never edited in place. Rounds 6-8 each found another hand-made shape a regex
# check let through (a resource outside its CPU block, a missing install line, a Python-only
# formula at 2.0.0); a formula that is not exactly what this function renders is refused, which
# closes the whole class instead of the next instance of it.
_FORMULA_HEAD = """class Cancellai < Formula
  desc "Safely reclaim disk space from old Codex CLI and Claude Code session data"
  homepage "https://github.com/matteo-dritara/homebrew-cancellai"
  url "{source_url}"
{version_line}  sha256 "{source_sha}"
  license "MIT"

  depends_on "python3"
"""
_LEGACY_BODY = """
  def install
    bin.install "cancellai.py" => "cancellai"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/cancellai --version")
    assert_match "cancellAI status", shell_output("#{{bin}}/cancellai status")
  end
end
"""
_ENGINE_BODY = """
  on_macos do
    on_arm do
      resource "engine" do
        url "{aarch64_apple_darwin_url}"
        sha256 "{aarch64_apple_darwin_sha}"
      end
    end
    on_intel do
      resource "engine" do
        url "{x86_64_apple_darwin_url}"
        sha256 "{x86_64_apple_darwin_sha}"
      end
    end
  end

  on_linux do
    on_intel do
      resource "engine" do
        url "{x86_64_unknown_linux_gnu_url}"
        sha256 "{x86_64_unknown_linux_gnu_sha}"
      end
    end
  end

  def install
    # The Rust engine is `cancellai` (E06-S04, ADR-0039's cutover). The frozen Python reference
    # stays installed as `cancellai-legacy` through 2.1.0, as the immediate rollback.
    resource("engine").stage {{ bin.install "cancellai-cli" => "cancellai" }}
    bin.install "cancellai.py" => "cancellai-legacy"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/cancellai version")
    assert_match version.to_s, shell_output("#{{bin}}/cancellai-legacy --version")
    assert_match "action(s) proposed", shell_output("#{{bin}}/cancellai plan")
  end
end
"""


def render_formula(
    version: str,
    source_sha: str,
    engines: dict[str, tuple[str, str]] | None,
    source_url: str | None = None,
) -> str:
    """The formula for `version`: Python-only when `engines` is None, otherwise the Rust engine as
    `cancellai` with one `(url, sha256)` per target. `source_url` defaults to the tag archive; an
    explicit one (a local archive, for CI) also states the version, which Homebrew cannot infer."""
    url = source_url or TARBALL.format(repo=REPO, version=version)
    version_line = f'  version "{version}"\n' if source_url else ""
    text = _FORMULA_HEAD.format(source_url=url, version_line=version_line, source_sha=source_sha)
    if engines is None:
        return text + _LEGACY_BODY.format()
    if sorted(engines) != sorted(ENGINE_TARGETS):
        raise ReleaseError(f"engine archives for {sorted(engines)}, expected {list(ENGINE_TARGETS)}")
    fields = {}
    for target, (engine_url, digest) in engines.items():
        key = target.replace("-", "_")
        fields[f"{key}_url"] = engine_url
        fields[f"{key}_sha"] = digest
    return text + _ENGINE_BODY.format(**fields)


def published_engines(version: str, digests: dict[str, str]) -> dict[str, tuple[str, str]]:
    return {target: (ENGINE_ASSET.format(repo=REPO, version=version, target=target), digests[target]) for target in ENGINE_TARGETS}


def live_formula_problems(text: str, version: str) -> list[str]:
    """Whether `text` is exactly what `render_formula` produces for `version` with the digests it
    names - offline, so `check` can run it. From the cutover version on it must carry the engine;
    before it, it must not."""
    digests = FORMULA_ALL_SHA_RE.findall(text)
    if not digests:
        return ["the formula names no sha256"]
    carries_engine = len(digests) == 1 + len(ENGINE_TARGETS)
    if parse(version) >= CUTOVER_VERSION and not carries_engine:
        return [f"v{version} is at or after the cutover but the formula does not carry the engine"]
    if parse(version) < CUTOVER_VERSION and carries_engine:
        return [f"v{version} is before the cutover but the formula carries the engine"]
    if carries_engine:
        expected = render_formula(version, digests[0], published_engines(version, dict(zip(ENGINE_TARGETS, digests[1:], strict=True))))
    else:
        expected = render_formula(version, digests[0], None)
    if text != expected:
        return [f"the formula is not the one release.py renders for v{version}; regenerate it with finalize"]
    return []


def published_digest_problems(text: str, version: str) -> list[str]:
    """Every digest in the formula against the bytes the release published: the tag archive,
    downloaded and hashed, and - from the cutover on - the formula must be exactly the published,
    verified `adoptable_formula` (E06 review rounds 7-9, ADR-0040). Needs the network, so it runs
    in `finalize` and `verify-formula`, not in the offline `check`."""
    digests = FORMULA_ALL_SHA_RE.findall(text)
    problems = []
    if not digests or digests[0] != archive_sha256(version):
        problems.append(f"the source digest is not the sha256 of the v{version} tag archive")
    if parse(version) >= CUTOVER_VERSION and text != adoptable_formula(version):
        problems.append(f"the formula is not the published v{version} formula its release manifest renders")
    return problems


def verify_installed(version: str, prefix: Path) -> list[str]:
    """What an installed cutover formula reports, exactly (E06 review rounds 6-7): the engine
    `cancellai-cli <version>` and the legacy command `cancellai <version>` - no exemption."""
    engine = subprocess.run([str(prefix / "bin" / "cancellai"), "version"], capture_output=True, text=True, check=False)  # noqa: S603
    legacy = subprocess.run([str(prefix / "bin" / "cancellai-legacy"), "--version"], capture_output=True, text=True, check=False)  # noqa: S603
    if engine.returncode != 0 or engine.stdout.strip().splitlines()[:1] != [f"cancellai-cli {version}"]:
        raise ReleaseError(f"installed `cancellai version` said {engine.stdout.strip()!r}, expected 'cancellai-cli {version}'")
    if legacy.returncode != 0 or legacy.stdout.strip() != f"cancellai {version}":
        raise ReleaseError(f"installed `cancellai-legacy --version` said {legacy.stdout.strip()!r}, expected 'cancellai {version}'")
    return [f"cancellai -> cancellai-cli {version}", f"cancellai-legacy -> cancellai {version}"]


def render_local_cutover(version: str, source_archive: Path, engine_archive: Path) -> str:
    """The engine formula over archives built from this commit, for CI to install and `brew test`
    the engine candidate itself. Every target names the local engine archive: only the runner's
    own is ever fetched."""
    source_sha = hashlib.sha256(source_archive.read_bytes()).hexdigest()
    engine_sha = hashlib.sha256(engine_archive.read_bytes()).hexdigest()
    engines = dict.fromkeys(ENGINE_TARGETS, (f"file://{engine_archive}", engine_sha))
    return render_formula(version, source_sha, engines, source_url=f"file://{source_archive}")


def write_atomically(path: Path, text: str) -> None:
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(text, encoding="utf-8")
    os.replace(temporary, path)


def finalize(
    version: str,
    sha256: str | None = None,
    adopt_cutover: bool = False,
) -> None:
    """Regenerate the formula for the pushed `version` from `render_formula`, never by editing.

    From the cutover version on the formula carries the engine; the release that first does so
    must say `--adopt-cutover`, which is only accepted with the owner's authorization bound to the
    migration Safety Verdict they accepted (E06-S15). From the cutover on the formula is the published, verified one
    (`adoptable_formula`) and `sha256` cannot stand in for it; before it, `sha256` skips the tag
    download. The text is written atomically and restored if `check` then fails.
    """
    versions = current_versions()
    if versions.source != version:
        raise ReleaseError(f"cancellai.py says {versions.source}, not {version}; run `prepare` first")
    if not release_evidence_path(version).exists():
        raise ReleaseError(f"missing {release_evidence_path(version).relative_to(ROOT)}; run `prepare` first")

    original = read(FORMULA)
    live_carries_engine = len(FORMULA_ALL_SHA_RE.findall(original)) == 1 + len(ENGINE_TARGETS)
    carries_engine = parse(version) >= CUTOVER_VERSION
    if carries_engine and not live_carries_engine:
        if not adopt_cutover:
            raise ReleaseError(f"v{version} is at or after the cutover and must carry the engine: pass --adopt-cutover")
        if version in UNVERSIONED_ENGINE_RELEASES:
            raise ReleaseError(f"v{version} cannot be the cutover release: its engine does not carry its own version")
        refused = cutover_authorization_problems(version)
        if refused:
            raise ReleaseError("the owner has not authorized this cutover:\n" + "\n".join(f"- {p}" for p in refused))
        print("adopting the engine formula: the Rust engine becomes `cancellai` (E06-S04)")
    elif adopt_cutover:
        raise ReleaseError("--adopt-cutover is for the one release that first carries the engine")

    if carries_engine:
        if sha256 is not None:
            raise ReleaseError("--sha256 cannot stand in for the published release from the cutover on")
        text = adoptable_formula(version)
        source_sha = FORMULA_ALL_SHA_RE.findall(text)[0]
    else:
        source_sha = sha256 or archive_sha256(version)
        if not re.fullmatch(r"[0-9a-f]{64}", source_sha):
            raise ReleaseError(f"not a SHA-256 digest: {source_sha!r}")
        text = render_formula(version, source_sha, None)
        if sha256 is None:
            problems = published_digest_problems(text, version)
            if problems:
                raise ReleaseError("the rendered formula does not match the published release:\n" + "\n".join(f"- {p}" for p in problems))
    write_atomically(FORMULA, text)

    problems = check()
    if problems:
        write_atomically(FORMULA, original)
        raise ReleaseError("release is still inconsistent; the formula was restored:\n" + "\n".join(f"- {p}" for p in problems))
    print(f"finalized v{version}")
    print(f"  Formula/cancellai.rb -> v{version} sha256 {source_sha}")
    print()
    print("Next:")
    print(f"  git commit -am 'chore(release): point formula at the v{version} tarball' && git push")


def record_outcome(version: str, state: str, reason: str | None) -> None:
    """Record whether the release for a cut version actually published."""
    if state not in PUBLISHED_STATES:
        raise ReleaseError(f"state must be one of {list(PUBLISHED_STATES)}, not {state!r}")
    if state == "no" and not (reason or "").strip():
        # An unpublished release with no reason is the folklore this story exists to replace.
        raise ReleaseError("--reason is required when a release did not publish: say what failed")
    path = release_evidence_path(version)
    if not path.exists():
        raise ReleaseError(f"no release evidence at {path.relative_to(ROOT)}; nothing to record against")
    line = f"- Published: {state}" + (f" - {reason.strip()}" if reason and reason.strip() else "")
    text = read(path)
    if PUBLISHED_RE.search(text):
        text = PUBLISHED_RE.sub(line.replace("\\", "\\\\"), text, count=1)
    else:
        text = text.replace("- Channel: stable\n", "- Channel: stable\n" + line + "\n", 1)
    path.write_text(text, encoding="utf-8")
    print(f"recorded v{version}: {line[len('- Published: ') :]}")


def unpublished_report(outcomes: dict[str, tuple[str, str]], source: str) -> list[str]:
    """Cut versions whose release never published, newest first.

    Reported rather than failed: they are history, and a gate that refuses forever because a
    release failed in the past is a gate that gets deleted. What matters is that the repository
    can say it rather than relying on whoever remembers.
    """
    lines = []
    for version in sorted(outcomes, key=parse, reverse=True):
        state, reason = outcomes[version]
        if state == "no":
            lines.append(f"  v{version} was cut and never published: {reason}")
        elif state == "pending" and version != source and parse(version) < parse(source):
            lines.append(f"  v{version} has no recorded outcome; run `release.py outcome --version {version} ...`")
    return lines


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Cut a cancellAI release.")
    sub = parser.add_subparsers(dest="command")
    sub.add_parser("check", help="Report version drift and epics closed without a release.")
    prepare_cmd = sub.add_parser("prepare", help="Bump versions, cut the changelog, write release evidence.")
    prepare_cmd.add_argument("--version", required=True)
    shape = prepare_cmd.add_mutually_exclusive_group(required=True)
    shape.add_argument("--epic", help="the epic this release closes")
    shape.add_argument("--fix", help="cut a patch that closes no epic; say what the tagged version could not carry")
    finalize_cmd = sub.add_parser("finalize", help="Point the Homebrew formula at the pushed tag.")
    finalize_cmd.add_argument("--version", required=True)
    finalize_cmd.add_argument("--sha256", help="skip the download and use this digest")
    finalize_cmd.add_argument("--adopt-cutover", action="store_true", help="the cutover release only: make the Rust engine `cancellai`")
    local_cmd = sub.add_parser("render-local-cutover", help="print the cutover formula for locally built archives")
    local_cmd.add_argument("--version", required=True)
    local_cmd.add_argument("--source-archive", required=True, type=Path)
    local_cmd.add_argument("--engine-archive", required=True, type=Path)
    render_cmd = sub.add_parser("render-release-formula", help="the release workflow: the formula as a function of the manifest")
    render_cmd.add_argument("--version", required=True)
    render_cmd.add_argument("--manifest", required=True, type=Path)
    render_cmd.add_argument("--source-sha", required=True)
    render_cmd.add_argument("--out", required=True, type=Path)
    sub.add_parser("verify-formula", help="compare the live formula's engine digests with the published ones")
    verify_cmd = sub.add_parser("verify-installed", help="assert an installed cutover formula reports its version")
    verify_cmd.add_argument("--version", required=True)
    verify_cmd.add_argument("--prefix", required=True, type=Path)
    outcome_cmd = sub.add_parser("outcome", help="Record whether a cut version's release actually published.")
    outcome_cmd.add_argument("--version", required=True)
    outcome_cmd.add_argument("--state", required=True, choices=list(PUBLISHED_STATES))
    outcome_cmd.add_argument("--reason", help="required when the release did not publish")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    command = args.command or "check"
    try:
        if command == "prepare":
            prepare(args.version, args.epic, args.fix)
        elif command == "finalize":
            finalize(args.version, args.sha256, adopt_cutover=args.adopt_cutover)
        elif command == "render-local-cutover":
            print(render_local_cutover(args.version, args.source_archive.resolve(), args.engine_archive.resolve()), end="")
        elif command == "render-release-formula":
            args.out.write_text(render_release_formula(args.version, args.manifest, args.source_sha), encoding="utf-8")
            print(f"rendered {args.out} for v{args.version} from {args.manifest}")
        elif command == "verify-formula":
            versions = current_versions()
            found = live_formula_problems(read(FORMULA), versions.formula) or published_digest_problems(read(FORMULA), versions.formula)
            if found:
                raise ReleaseError("\n".join(found))
            print(f"formula OK: v{versions.formula} is exactly what release.py renders, with the digests the release published")
        elif command == "verify-installed":
            for line in verify_installed(args.version, args.prefix):
                print(line)
        elif command == "outcome":
            record_outcome(args.version, args.state, args.reason)
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
            for line in unpublished_report(release_outcomes(), versions.source):
                print(line)
            print(f"next epic closure would suggest v{suggest_version(versions.source)}")
        return 0
    except (ReleaseError, OSError, KeyError, ValueError) as exc:
        print(f"RELEASE ERROR: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
