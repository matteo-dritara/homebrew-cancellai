#!/usr/bin/env python3
"""cancellAI

Safe cleanup utility for local Codex and Claude Code session data.

Design goals:
- standard-library only;
- conservative defaults;
- official Codex session deletion when supported;
- preserve auth/config/skills/plugins/Claude auto-memory;
- dry-run and status modes;
- configurable retention and keep-latest safety rail;
- safe handling of custom CODEX_HOME / CLAUDE_CONFIG_DIR.

Supported platform: macOS. Other platforms are untested.
"""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import unicodedata
from collections.abc import Callable, Iterator, Sequence
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

VERSION = "1.8.0"
DEFAULT_DAYS = 7
DEFAULT_KEEP_LATEST = 2
UUID_RE = re.compile(r"(?P<uuid>[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})")
KNOWN_COMMANDS = {"clean", "status", "configure", "version"}

# Stable exit taxonomy. Automation must be able to distinguish "nothing ran because
# it was unsafe" from "everything ran" and from "you typed the command wrong".
EXIT_OK = 0
EXIT_CANCELLED = 1
EXIT_USAGE = 2
EXIT_FAILED = 3
EXIT_BLOCKED = 4

# Claude Code paths documented as session/application data. Auto-memory is deliberately excluded.
CLAUDE_RETENTION_PATHS = (
    "file-history",
    "plans",
    "debug",
    "paste-cache",
    "image-cache",
    "uploads",
    "session-env",
    "tasks",
    "shell-snapshots",
)
CLAUDE_LEGACY_PATHS = ("todos", "statsig", "logs")
CLAUDE_SAFE_CACHE_FILES = (
    "remote-settings.json",
    "policy-limits.json",
    os.path.join("cache", "changelog.md"),
)

# These are never deleted by this script in normal operation.
CLAUDE_PROTECTED_NAMES = {
    "settings.json",
    "keybindings.json",
    "plugins",
    "skills",
    "agents",
    "commands",
    "rules",
    "workflows",
    "output-styles",
    "agent-memory",
}
CODEX_PROTECTED_NAMES = {
    "auth.json",
    "config.toml",
    "skills",
    "rules",
    "memories",
    # Installed Codex plugin state (observed layout: plugins/, plugins/cache,
    # plugins/.plugin-appserver). No current code path sweeps this, but it
    # mirrors CLAUDE_PROTECTED_NAMES's "plugins" entry as a deliberate guard
    # against any future broader top-level scan treating it as disposable.
    "plugins",
}


# Budgets. cancellAI governs storage, so it never becomes an unbounded producer itself
# (C-11): recorded scan errors and pre-authority root probing are both capped.
MAX_RECORDED_SCAN_ERRORS = 50
MAX_ROOT_PROBE_ENTRIES = 2000

# Coverage vocabulary. There is deliberately no state meaning "deleted as it stands":
# no top-level provider entry is treated that way, so every state names either a container
# whose contents are selected, a conditional case, or something never touched.
COVERAGE_STATES = ("selective", "selective-aggressive", "aggressive-only", "trimmed", "protected", "reported", "unknown")
# `selective` is the honest label for a container: cancellAI selects entries *inside* it by
# age and policy and never deletes the container. Whether any particular entry is selected
# depends on its contents, age and the keep-latest rail, so the directory itself can never
# be called cleanable.
CODEX_SELECTIVE_NAMES = {"sessions", "archived_sessions", "log", "tmp"}
CODEX_REPORTED_PREFIXES = ("state_", "logs_", "goals_", "memories_", "queue_", "thread_history_")
CLAUDE_SELECTIVE_NAMES = {"projects", *CLAUDE_RETENTION_PATHS}
CLAUDE_SELECTIVE_AGGRESSIVE_NAMES = {"backups", *(Path(rel).parent.name for rel in CLAUDE_SAFE_CACHE_FILES if Path(rel).parent.name)}
CLAUDE_AGGRESSIVE_ONLY_NAMES = {*CLAUDE_LEGACY_PATHS, *(rel for rel in CLAUDE_SAFE_CACHE_FILES if Path(rel).parent.name == "")}
CLAUDE_TRIMMED_NAMES = {"history.jsonl"}


@dataclass(frozen=True)
class ProcessObservation:
    """What we learned about provider activity, and whether we learned anything at all.

    A dictionary of empty lists cannot distinguish "no provider is running" from "we could
    not enumerate processes". Conflating the two makes an unusable observation authorize
    deletion while a provider may be writing, so the two are carried separately.
    """

    pids: dict[str, list[int]]
    complete: bool = True

    def running(self, tool: str) -> list[int]:
        return self.pids.get(tool, [])

    @property
    def any_running(self) -> bool:
        return any(self.pids.values())


@dataclass
class Scan:
    """Completeness channel for one discovery scope.

    Filesystem helpers answer "how big" and "how recent" with numbers that cannot express
    "I could not look". This carries that separately, so absence of evidence never becomes
    evidence of absence, and an incomplete scope cannot hand out destructive authority.
    """

    scope: str
    errors: list[str] = field(default_factory=list)
    truncated: bool = False

    @property
    def complete(self) -> bool:
        return not self.errors and not self.truncated

    def record(self, path: Path, exc: OSError) -> None:
        # A path that vanished mid-scan is a race, not an unreadable scope: there is nothing
        # left to observe and nothing left to delete. Anything else means we are blind.
        if isinstance(exc, FileNotFoundError):
            return
        if len(self.errors) >= MAX_RECORDED_SCAN_ERRORS:
            self.truncated = True
            return
        self.errors.append(f"{path}: {exc.strerror or type(exc).__name__}")


@dataclass(frozen=True)
class RootAuthority:
    """Whether a configured provider root has earned the right to be mutated."""

    tool: str
    path: Path
    origin: str  # default | custom
    confidence: str  # default | high | low | unknown
    markers: tuple[str, ...]

    @property
    def structurally_credible(self) -> bool:
        """Whether the directory *looks* like the provider. Reported, never authoritative.

        Structural evidence is cheap to fabricate and therefore cannot be positive provider
        identity (SI-002). It is useful information for the operator; it is not permission.
        """
        return self.confidence in {"default", "high"}

    def destructive_allowed(self) -> bool:
        """Only the provider's own default directory may be mutated by the Python reference.

        Two weaker schemes were tried and rejected by independent review: filename markers,
        then validated structure plus an explicit operator flag. Neither establishes that a
        directory belongs to the provider - a lookalike satisfies both. Positive identity
        needs the provider capability contract, which the Rust core will have and this
        reference does not. Until then, a relocated root is inspectable and nothing more.
        See ADR-0013.
        """
        return self.origin == "default"

    def explain(self) -> str:
        if self.destructive_allowed():
            return f"{self.tool} root {self.path} ({self.origin}, confidence {self.confidence})"
        found = ", ".join(self.markers) if self.markers else "none"
        looks_right = " It does look like one" if self.structurally_credible else " It does not look like one"
        return (
            f"Refusing destructive work on {self.tool} root {self.path}: it is not the default {self.tool} directory."
            f"{looks_right} (confidence {self.confidence}; validated provider markers: {found}), but looking right is "
            "not proof of ownership, so this build will not remove anything there. Inspection with `status` works "
            "normally; "
            f"unset the {'CLAUDE_CONFIG_DIR' if self.tool == 'claude' else 'CODEX_HOME'} override to clean the default root."
        )


@dataclass(frozen=True)
class CoverageBucket:
    """How much of a provider root this build can actually reason about."""

    state: str
    entries: int
    bytes: int
    names: list[str]


@dataclass(frozen=True)
class Action:
    tool: str
    category: str
    path: Path
    size: int
    mtime: float
    session_id: str | None = None
    strategy: str = "filesystem"  # filesystem | codex-cli
    parent_session_id: str | None = None


@dataclass
class Plan:
    cutoff: float
    days: int
    keep_latest: int
    actions: list[Action] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)
    claude_history_session_ids: set[str] = field(default_factory=set)
    claude_history_lines: int = 0
    scans: list[Scan] = field(default_factory=list)
    root_authority: dict[str, RootAuthority] = field(default_factory=dict)
    withheld: list[str] = field(default_factory=list)
    for_mutation: bool = True

    @property
    def scan_complete(self) -> bool:
        return all(scan.complete for scan in self.scans)

    @property
    def scan_errors(self) -> list[str]:
        return [error for scan in self.scans for error in scan.errors]

    @property
    def incomplete_scopes(self) -> list[str]:
        return [scan.scope for scan in self.scans if not scan.complete]

    @property
    def estimated_bytes(self) -> int:
        return sum(a.size for a in self.actions)

    def actions_for(self, tool: str) -> list[Action]:
        return [a for a in self.actions if a.tool == tool]


@dataclass
class CleanResult:
    attempted: int = 0
    succeeded: int = 0
    failed: int = 0
    skipped: int = 0
    freed_bytes: int = 0
    errors: list[str] = field(default_factory=list)
    deleted_claude_session_ids: set[str] = field(default_factory=set)
    blocked_tools: set[str] = field(default_factory=set)
    deferred: list[str] = field(default_factory=list)

    @property
    def partial(self) -> bool:
        """True when safety deliberately prevented requested work from running."""
        return bool(self.blocked_tools or self.deferred)


class SafetyError(RuntimeError):
    pass


def now_ts() -> float:
    return time.time()


def format_bytes(num: int) -> str:
    value = float(max(num, 0))
    units = ["B", "KB", "MB", "GB", "TB"]
    for unit in units:
        if value < 1024.0 or unit == units[-1]:
            if unit == "B":
                return f"{int(value)} {unit}"
            return f"{value:.2f} {unit}"
        value /= 1024.0
    return f"{value:.2f} TB"


def format_age(mtime: float, reference: float | None = None) -> str:
    reference = reference or now_ts()
    days = max(0.0, (reference - mtime) / 86400.0)
    if days < 1:
        hours = days * 24
        return f"{hours:.1f}h"
    return f"{days:.1f}d"


def default_home(tool: str) -> Path:
    return Path.home() / (".claude" if tool == "claude" else ".codex")


def get_codex_home() -> Path:
    raw = os.environ.get("CODEX_HOME")
    return Path(raw).expanduser() if raw else default_home("codex")


def get_claude_home() -> Path:
    raw = os.environ.get("CLAUDE_CONFIG_DIR")
    return Path(raw).expanduser() if raw else default_home("claude")


# Marker validators below deliberately answer False on any error. They only ever *reduce*
# confidence, so an unreadable marker withholds authority rather than granting it.
def _is_json_object(path: Path) -> bool:
    try:
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 8 * 1024 * 1024:
            return False
        return isinstance(json.loads(path.read_text(encoding="utf-8", errors="strict")), dict)
    except (OSError, ValueError):
        return False


def _is_jsonl_of_objects(path: Path) -> bool:
    try:
        if path.is_symlink() or not path.is_file():
            return False
        with path.open("r", encoding="utf-8", errors="replace") as fh:
            for _ in range(20):
                line = fh.readline()
                if not line:
                    break
                if line.strip() and isinstance(json.loads(line), dict):
                    return True
    except (OSError, ValueError):
        return False
    return False


def _is_nonempty_file(path: Path) -> bool:
    try:
        return not path.is_symlink() and path.is_file() and path.stat().st_size > 0
    except OSError:
        return False


def _contains_uuid_named_jsonl(root: Path, prefix: str = "") -> bool:
    """Bounded probe for a provider-shaped transcript below `root`.

    Deliberately capped: fingerprinting runs before any authority is granted and must not
    turn into an unbounded walk of a directory we have not yet trusted.
    """
    seen = 0
    try:
        if root.is_symlink() or not root.is_dir():
            return False
        for base, dirs, files in os.walk(root, followlinks=False):
            base_p = Path(base)
            dirs[:] = [d for d in dirs if not (base_p / d).is_symlink()]
            for name in files:
                seen += 1
                if seen > MAX_ROOT_PROBE_ENTRIES:
                    return False
                if name.endswith(".jsonl") and name.startswith(prefix) and extract_uuid(name):
                    return True
    except OSError:
        return False
    return False


def _is_dir(path: Path) -> bool:
    try:
        return not path.is_symlink() and path.is_dir()
    except OSError:
        return False


# name -> (validator, is_identifying)
ROOT_MARKERS: dict[str, dict[str, tuple[Callable[[Path], bool], bool]]] = {
    "codex": {
        "auth.json": (_is_json_object, True),
        "session_index.jsonl": (_is_jsonl_of_objects, True),
        "installation_id": (_is_nonempty_file, True),
        "sessions": (lambda p: _contains_uuid_named_jsonl(p, "rollout-"), True),
        "config.toml": (_is_nonempty_file, False),
        "archived_sessions": (_is_dir, False),
        "history.jsonl": (_is_jsonl_of_objects, False),
        "skills": (_is_dir, False),
        "rules": (_is_dir, False),
        "memories": (_is_dir, False),
        "plugins": (_is_dir, False),
        "sqlite": (_is_dir, False),
    },
    "claude": {
        "settings.json": (_is_json_object, True),
        "keybindings.json": (_is_json_object, True),
        "projects": (_contains_uuid_named_jsonl, True),
        "history.jsonl": (_is_jsonl_of_objects, False),
        "file-history": (_is_dir, False),
        "shell-snapshots": (_is_dir, False),
        "plugins": (_is_dir, False),
        "agent-memory": (_is_dir, False),
        "session-env": (_is_dir, False),
        "tasks": (_is_dir, False),
        "statsig": (_is_dir, False),
        "todos": (_is_dir, False),
    },
}


def fingerprint_root(path: Path, tool: str) -> RootAuthority:
    """Decide how much a configured root has proved about its own identity.

    `validate_config_root()` only rejects catastrophically broad paths; an ordinary project
    directory that happens to contain `tmp/` or `log/` passes it. This adds the missing
    question - does this directory actually look like the provider we are about to delete
    from - and answers it from validated structure, never from the path string and never
    from a filename alone.
    """
    markers = ROOT_MARKERS[tool]
    resolved = path.resolve(strict=False)
    origin = "default" if resolved == default_home(tool).resolve(strict=False) else "custom"

    found: list[str] = []
    identifying = 0
    for name, (validator, is_identifying) in markers.items():
        if validator(path / name):
            found.append(name)
            identifying += int(is_identifying)

    if origin == "default":
        # The provider's own directory is authoritative by definition, including on a fresh
        # machine where it is empty or absent.
        confidence = "default"
    elif identifying >= 1 and len(found) >= 2:
        confidence = "high"
    elif found:
        confidence = "low"
    else:
        confidence = "unknown"
    return RootAuthority(tool=tool, path=resolved, origin=origin, confidence=confidence, markers=tuple(sorted(found)))


def canonical_name(name: str) -> str:
    """Unicode canonical caseless form, per UAX #15: NFD, casefold, NFD again.

    Case folding alone is not enough to compare filenames. APFS returns decomposed forms,
    so the same directory can arrive as `plu` + U+0308 + `gins` or as `plügins`, and folding
    compares them as different names. Folding can itself emit composed characters, hence the
    second normalization. Both sides of every protected-name comparison go through here.
    """
    return unicodedata.normalize("NFD", unicodedata.normalize("NFD", name).casefold())


def protected_names_for(tool: str) -> set[str]:
    return CLAUDE_PROTECTED_NAMES if tool == "claude" else CODEX_PROTECTED_NAMES


def protected_component(path: Path, root: Path, protected_names: set[str]) -> str | None:
    """Return the protected path component if `path` is, or lives under, a protected entry.

    This is a name-based barrier, deliberately independent of whichever scanner produced
    the candidate: a future discovery change cannot quietly invalidate the documented
    protection lists.

    The name is checked both lexically and after resolution. Resolving first would let a
    protected entry that is itself a symlink point outside the root, fall out of the
    relative-path computation, and lose its protection - which is exactly the entry an
    attacker or a misbehaving provider install would want unprotected.
    """
    if not protected_names:
        return None
    absolute = path.expanduser().absolute()
    root_absolute = root.expanduser().absolute()
    views: list[tuple[Path, Path]] = [
        (Path(os.path.normpath(absolute)), Path(os.path.normpath(root_absolute))),
    ]
    try:
        views.append((absolute.resolve(strict=False), root_absolute.resolve(strict=False)))
    except OSError:
        return "<unresolvable>"

    # macOS mounts APFS case-insensitively and stores decomposed Unicode, so `Plugins`,
    # `plugins` and a decomposed spelling can all name the same directory. Comparing raw
    # strings would make the barrier depend on which form a scanner happened to produce.
    # On a case-sensitive filesystem this is merely over-inclusive, which is the safe
    # direction for a barrier.
    folded = {canonical_name(name): name for name in protected_names}
    for candidate, base in views:
        try:
            relative = candidate.relative_to(base)
        except ValueError:
            # This view falls outside the approved root; containment checks own that case.
            continue
        for part in relative.parts:
            canonical = folded.get(canonical_name(part))
            if canonical is not None:
                return canonical
    return None


def validate_config_root(path: Path, label: str) -> Path:
    """Reject catastrophically broad roots before any destructive operation."""
    expanded = path.expanduser().absolute()
    # resolve(strict=False) follows existing symlinks without requiring the target to exist.
    resolved = expanded.resolve(strict=False)
    home = Path.home().resolve(strict=False)
    forbidden = {Path("/").resolve(), home}
    if resolved in forbidden:
        raise SafetyError(f"Refusing unsafe {label} root: {resolved}")
    if len(resolved.parts) < 3:
        raise SafetyError(f"Refusing suspiciously broad {label} root: {resolved}")
    return resolved


def is_within(path: Path, root: Path) -> bool:
    try:
        path.resolve(strict=False).relative_to(root.resolve(strict=False))
        return True
    except ValueError:
        return False


def observe(path: Path, scan: Scan | None = None) -> os.stat_result | None:
    """`lstat` that separates "not there" from "could not look".

    `Path.exists()` answers False for both, so using it as a discovery guard silently turns
    an unreadable directory into an empty one - the exact collapse this scope channel
    exists to prevent. Every guard that decides whether a scope was scanned goes through
    here instead.
    """
    try:
        return path.lstat()
    except FileNotFoundError:
        return None
    except OSError as exc:
        if scan is not None:
            scan.record(path, exc)
        return None


def safe_lstat_size(path: Path, scan: Scan | None = None) -> int:
    try:
        st = path.lstat()
    except OSError as exc:
        if scan is not None:
            scan.record(path, exc)
        return 0
    if stat.S_ISLNK(st.st_mode):
        # A symlink is never followed for size accounting (E00-S02 / ADR-0013): its own
        # lstat().st_size is not disk footprint, it is the byte length of the stored target
        # path string, which is platform- and location-dependent (an identical fixture reports
        # a different "size" depending on the absolute length of wherever it happens to sit -
        # this is exactly how the characterization corpus caught it: the same symlink fixture
        # produced a different committed-vs-fresh byte count on Linux CI than on macOS, because
        # the temp-directory prefix length differs). Reporting it as size would be reporting a
        # path length as if it were storage, so it contributes nothing here.
        return 0
    if stat.S_ISREG(st.st_mode):
        return st.st_size
    if stat.S_ISDIR(st.st_mode):
        return directory_size(path, scan)
    return 0


def directory_size(root: Path, scan: Scan | None = None) -> int:
    st = observe(root, scan)
    if st is None:
        return 0
    if stat.S_ISLNK(st.st_mode):
        # See safe_lstat_size: a symlink's own lstat().st_size is a target-path length, not
        # disk footprint, and must never be accounted as size.
        return 0
    if stat.S_ISREG(st.st_mode):
        return st.st_size
    total = 0

    def on_walk_error(exc: OSError) -> None:
        if scan is not None:
            scan.record(Path(getattr(exc, "filename", None) or root), exc)

    try:
        for base, dirs, files in os.walk(root, followlinks=False, onerror=on_walk_error):
            base_p = Path(base)
            # Do not follow directory symlinks. os.walk places them in dirs. A symlink here
            # contributes no bytes (see safe_lstat_size) - only its non-symlink siblings are
            # kept for further descent.
            keep_dirs: list[str] = []
            for name in dirs:
                p = base_p / name
                try:
                    lst = p.lstat()
                    if not stat.S_ISLNK(lst.st_mode):
                        keep_dirs.append(name)
                except OSError as exc:
                    if scan is not None:
                        scan.record(p, exc)
                    continue
            dirs[:] = keep_dirs
            for name in files:
                p = base_p / name
                try:
                    lst = p.lstat()
                    # A symlink can appear in `files` too (os.walk classifies by the *target's*
                    # type, and a symlink to a regular file is not a directory) - it must not
                    # contribute its target-path-length "size" either.
                    if not stat.S_ISLNK(lst.st_mode):
                        total += lst.st_size
                except OSError as exc:
                    if scan is not None:
                        scan.record(p, exc)
    except OSError as exc:
        if scan is not None:
            scan.record(root, exc)
    return total


def latest_mtime(path: Path, scan: Scan | None = None) -> float:
    """Return latest mtime within a tree without following symlinks."""
    try:
        st = path.lstat()
    except OSError as exc:
        if scan is not None:
            scan.record(path, exc)
        return 0.0
    latest = st.st_mtime
    if stat.S_ISLNK(st.st_mode) or stat.S_ISREG(st.st_mode):
        return latest

    def on_walk_error(exc: OSError) -> None:
        if scan is not None:
            scan.record(Path(getattr(exc, "filename", None) or path), exc)

    try:
        for base, dirs, files in os.walk(path, followlinks=False, onerror=on_walk_error):
            base_p = Path(base)
            keep_dirs: list[str] = []
            for name in dirs:
                p = base_p / name
                try:
                    lst = p.lstat()
                except OSError as exc:
                    if scan is not None:
                        scan.record(p, exc)
                    continue
                latest = max(latest, lst.st_mtime)
                if not stat.S_ISLNK(lst.st_mode):
                    keep_dirs.append(name)
            dirs[:] = keep_dirs
            for name in files:
                p = base_p / name
                try:
                    latest = max(latest, p.lstat().st_mtime)
                except OSError as exc:
                    if scan is not None:
                        scan.record(p, exc)
    except OSError as exc:
        if scan is not None:
            scan.record(path, exc)
    return latest


def iter_files(root: Path, suffix: str | None = None, scan: Scan | None = None) -> Iterator[Path]:
    st = observe(root, scan)
    if st is None or stat.S_ISLNK(st.st_mode):
        return

    def on_walk_error(exc: OSError) -> None:
        if scan is not None:
            scan.record(Path(getattr(exc, "filename", None) or root), exc)

    for base, dirs, files in os.walk(root, followlinks=False, onerror=on_walk_error):
        base_p = Path(base)
        dirs[:] = [d for d in dirs if not (base_p / d).is_symlink()]
        for name in files:
            p = base_p / name
            if suffix is None or name.endswith(suffix):
                yield p


def extract_uuid(text: str) -> str | None:
    matches = list(UUID_RE.finditer(text))
    return matches[-1].group("uuid").lower() if matches else None


def read_codex_parent_session_id(path: Path, scan: Scan | None = None) -> str | None:
    """Read parent_thread_id from Codex session_meta without scanning the full rollout.

    Current Codex rollouts put session metadata near the head of the JSONL file.
    We intentionally cap both records and bytes so discovery stays cheap even when a
    transcript is very large. Unknown/legacy formats simply return None.
    """
    max_records = 10
    max_bytes = 512 * 1024
    consumed = 0
    try:
        with path.open("r", encoding="utf-8", errors="replace") as fh:
            for _ in range(max_records):
                line = fh.readline()
                if not line:
                    break
                consumed += len(line.encode("utf-8", errors="replace"))
                if consumed > max_bytes:
                    break
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if obj.get("type") != "session_meta":
                    continue
                payload = obj.get("payload")
                if not isinstance(payload, dict):
                    return None
                meta = payload.get("meta")
                if not isinstance(meta, dict):
                    meta = payload
                raw_parent = meta.get("parent_thread_id")
                if raw_parent is None:
                    return None
                return extract_uuid(str(raw_parent))
    except OSError as exc:
        # Unreadable lineage is not "no parent": it changes which sessions are treated as
        # independent safety units, so the scope must lose destructive authority.
        if scan is not None:
            scan.record(path, exc)
        return None
    return None


def choose_codex_old_sessions(
    actions: list[Action],
    cutoff: float,
    keep_latest: int,
    *,
    collapse_subagents: bool,
) -> list[Action]:
    """Choose old Codex session trees conservatively.

    `keep_latest` counts root sessions, not subagents. A root tree is considered recent
    when *any* discovered descendant is recent. This prevents deleting an old-looking
    parent whose subagent was updated recently. For the official Codex CLI backend,
    one delete action per root is emitted because Codex cascades deletion to subagents.
    Filesystem fallback emits every rollout path because raw unlinking does not cascade.
    """
    if not actions:
        return []

    # One representative per thread id for graph decisions. Keep the freshest copy if a
    # transient duplicate exists across active/archived storage. Entries without ids are
    # conservatively handled by the generic selector below.
    by_id: dict[str, Action] = {}
    no_id: list[Action] = []
    for action in actions:
        if not action.session_id:
            no_id.append(action)
            continue
        current = by_id.get(action.session_id)
        if current is None or action.mtime > current.mtime:
            by_id[action.session_id] = action

    def root_id_for(sid: str) -> str:
        current = sid
        seen: set[str] = set()
        while current not in seen:
            seen.add(current)
            action = by_id.get(current)
            if action is None or not action.parent_session_id:
                return current
            parent = action.parent_session_id
            if parent not in by_id:
                # Parent is not locally discoverable, so this subtree is an independent
                # safety unit rather than assuming deletion semantics we cannot inspect.
                return current
            current = parent
        # Malformed/cyclic metadata: isolate the original thread rather than over-delete.
        return sid

    groups: dict[str, list[Action]] = {}
    for sid, action in by_id.items():
        groups.setdefault(root_id_for(sid), []).append(action)

    group_rows: list[tuple[str, float, int, list[Action]]] = []
    for root_id, members in groups.items():
        effective_mtime = max(a.mtime for a in members)
        total_size = sum(a.size for a in members)
        group_rows.append((root_id, effective_mtime, total_size, members))
    group_rows.sort(key=lambda row: row[1], reverse=True)

    protected_roots = {row[0] for row in group_rows[: max(keep_latest, 0)]}
    selected: list[Action] = []
    for root_id, effective_mtime, total_size, members in group_rows:
        if root_id in protected_roots or effective_mtime >= cutoff:
            continue
        if collapse_subagents:
            root_action = by_id.get(root_id)
            if root_action is None:
                # Defensive fallback. root_id normally always has a representative.
                root_action = max(members, key=lambda a: a.mtime)
            selected.append(
                Action(
                    tool=root_action.tool,
                    category=root_action.category,
                    path=root_action.path,
                    size=total_size,
                    mtime=effective_mtime,
                    session_id=root_action.session_id,
                    strategy=root_action.strategy,
                    parent_session_id=root_action.parent_session_id,
                )
            )
        else:
            # Raw filesystem removal must remove each rollout independently. Include all
            # copies belonging to selected thread ids, not just graph representatives.
            member_ids = {a.session_id for a in members if a.session_id}
            selected.extend(a for a in actions if a.session_id in member_ids and a.mtime < cutoff)

    selected.extend(choose_old_sessions(no_id, cutoff, keep_latest=0))

    # Exact-path de-duplication is useful for pathological duplicate discovery.
    deduped: list[Action] = []
    seen_paths: set[str] = set()
    for action in sorted(selected, key=lambda a: a.mtime, reverse=True):
        key = str(action.path.resolve(strict=False))
        if key in seen_paths:
            continue
        seen_paths.add(key)
        deduped.append(action)
    return deduped


def choose_old_sessions(actions: list[Action], cutoff: float, keep_latest: int) -> list[Action]:
    """Apply age cutoff plus a keep-latest rail, counting unique sessions rather than files."""
    ordered = sorted(actions, key=lambda a: a.mtime, reverse=True)
    protected_ids: set[str] = set()
    protected_paths: set[Path] = set()
    protected_count = 0
    for action in ordered:
        if protected_count >= max(keep_latest, 0):
            break
        if action.session_id:
            if action.session_id in protected_ids:
                continue
            protected_ids.add(action.session_id)
        else:
            if action.path in protected_paths:
                continue
            protected_paths.add(action.path)
        protected_count += 1

    selected: list[Action] = []
    seen_ids: set[str] = set()
    seen_paths: set[Path] = set()
    for action in ordered:
        if action.mtime >= cutoff:
            continue
        if action.session_id and action.session_id in protected_ids:
            continue
        if action.path in protected_paths:
            continue
        if action.session_id:
            if action.session_id in seen_ids:
                continue
            seen_ids.add(action.session_id)
        else:
            if action.path in seen_paths:
                continue
            seen_paths.add(action.path)
        selected.append(action)
    return selected


def codex_delete_supported(codex_bin: str | None = None) -> tuple[bool, str | None]:
    codex_bin = codex_bin or shutil.which("codex")
    if not codex_bin:
        return False, None
    try:
        # codex_bin is a PATH-resolved absolute path, not shell-interpreted; no untrusted input.
        proc = subprocess.run(  # noqa: S603
            [codex_bin, "delete", "--help"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=8,
            check=False,
        )
        text = proc.stdout or ""
        return proc.returncode == 0 and "--force" in text, codex_bin
    except (OSError, subprocess.SubprocessError):
        return False, codex_bin


def discover_codex_sessions(codex_home: Path, strategy: str, scan: Scan | None = None) -> list[Action]:
    found: list[Action] = []
    for rel, category in (("sessions", "session"), ("archived_sessions", "archived-session")):
        root = codex_home / rel
        root_stat = observe(root, scan)
        if root_stat is None or stat.S_ISLNK(root_stat.st_mode):
            continue
        for p in iter_files(root, suffix=".jsonl", scan=scan):
            if not p.name.startswith("rollout-"):
                continue
            sid = extract_uuid(p.name)
            if not sid:
                continue
            try:
                st = p.lstat()
            except OSError as exc:
                if scan is not None:
                    scan.record(p, exc)
                continue
            found.append(
                Action(
                    tool="codex",
                    category=category,
                    path=p,
                    size=st.st_size,
                    mtime=st.st_mtime,
                    session_id=sid,
                    strategy=strategy,
                    parent_session_id=read_codex_parent_session_id(p, scan),
                )
            )
    return found


def discover_claude_sessions(claude_home: Path, scan: Scan | None = None) -> list[Action]:
    projects = claude_home / "projects"
    found: list[Action] = []
    projects_stat = observe(projects, scan)
    if projects_stat is None or stat.S_ISLNK(projects_stat.st_mode):
        return found

    # Top-level project transcript files only. memory/ and subagent JSONL are never treated as roots.
    try:
        project_dirs = [p for p in projects.iterdir() if p.is_dir() and not p.is_symlink()]
    except OSError as exc:
        if scan is not None:
            scan.record(projects, exc)
        return found

    for project_dir in project_dirs:
        try:
            children = list(project_dir.iterdir())
        except OSError as exc:
            if scan is not None:
                scan.record(project_dir, exc)
            continue
        for p in children:
            if not p.is_file() or p.is_symlink() or p.suffix != ".jsonl":
                continue
            sid = extract_uuid(p.stem)
            if not sid:
                continue
            try:
                st = p.stat()
            except OSError as exc:
                if scan is not None:
                    scan.record(p, exc)
                continue
            companion = project_dir / p.stem
            size = st.st_size
            mt = st.st_mtime
            if companion.exists() and companion.is_dir() and not companion.is_symlink():
                size += directory_size(companion, scan)
                mt = max(mt, latest_mtime(companion, scan))
            found.append(
                Action(
                    tool="claude",
                    category="session",
                    path=p,
                    size=size,
                    mtime=mt,
                    session_id=sid,
                    strategy="filesystem",
                )
            )
    return found


def discover_aged_top_entries(
    root: Path,
    tool: str,
    category: str,
    cutoff: float,
    protected_session_ids: set[str] | None = None,
    scan: Scan | None = None,
) -> list[Action]:
    st = observe(root, scan)
    if st is None or stat.S_ISLNK(st.st_mode):
        return []
    protected_session_ids = protected_session_ids or set()
    actions: list[Action] = []
    try:
        entries = list(root.iterdir())
    except OSError as exc:
        if scan is not None:
            scan.record(root, exc)
        return actions
    for p in entries:
        sid = extract_uuid(p.name)
        if sid and sid in protected_session_ids:
            continue
        mt = latest_mtime(p, scan)
        if mt <= 0 or mt >= cutoff:
            continue
        actions.append(
            Action(
                tool=tool,
                category=category,
                path=p,
                size=safe_lstat_size(p, scan),
                mtime=mt,
                session_id=sid,
                strategy="filesystem",
            )
        )
    return actions


def discover_codex_aux(codex_home: Path, cutoff: float, scan: Scan | None = None) -> list[Action]:
    actions: list[Action] = []
    for rel, category in (("log", "old-log"), ("tmp", "old-temp")):
        actions.extend(discover_aged_top_entries(codex_home / rel, "codex", category, cutoff, scan=scan))
    return actions


def discover_claude_aux(
    claude_home: Path,
    cutoff: float,
    aggressive: bool,
    protected_session_ids: set[str] | None = None,
    scan: Scan | None = None,
) -> list[Action]:
    actions: list[Action] = []
    for rel in CLAUDE_RETENTION_PATHS:
        actions.extend(discover_aged_top_entries(claude_home / rel, "claude", rel, cutoff, protected_session_ids, scan))

    if aggressive:
        actions.extend(discover_aged_top_entries(claude_home / "backups", "claude", "backups", cutoff, protected_session_ids, scan))
        # Legacy directories are no longer written by current Claude Code. --aggressive
        # widens which categories are eligible; it never bypasses the age cutoff.
        for rel in CLAUDE_LEGACY_PATHS:
            root = claude_home / rel
            legacy_stat = observe(root, scan)
            if legacy_stat is not None and not stat.S_ISLNK(legacy_stat.st_mode):
                mt = latest_mtime(root, scan)
                if mt <= 0 or mt >= cutoff:
                    continue
                actions.append(
                    Action(
                        tool="claude",
                        category=f"legacy-{rel}",
                        path=root,
                        size=directory_size(root, scan),
                        mtime=mt,
                    )
                )
        for rel in CLAUDE_SAFE_CACHE_FILES:
            p = claude_home / rel
            st = observe(p, scan)
            if st is None or not (stat.S_ISREG(st.st_mode) or stat.S_ISLNK(st.st_mode)):
                continue
            if st.st_mtime < cutoff:
                actions.append(
                    Action(
                        tool="claude",
                        category="rebuildable-cache",
                        path=p,
                        size=st.st_size,
                        mtime=st.st_mtime,
                    )
                )
    return actions


def count_claude_history_matches(history_path: Path, session_ids: set[str], scan: Scan | None = None) -> int:
    if not session_ids or observe(history_path, scan) is None:
        return 0
    matches = 0
    try:
        with history_path.open("r", encoding="utf-8", errors="replace") as fh:
            for line in fh:
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                sid = str(obj.get("sessionId", "")).lower()
                if sid in session_ids:
                    matches += 1
    except OSError as exc:
        if scan is not None:
            scan.record(history_path, exc)
        return 0
    return matches


def build_plan(
    *,
    days: int,
    keep_latest: int,
    tools: set[str],
    codex_home: Path,
    claude_home: Path,
    codex_backend: str,
    aggressive: bool,
    for_mutation: bool = True,
) -> Plan:
    """Assemble the set of actions.

    `for_mutation=False` is the inspection path used by `status`: discovery still runs and
    still reports what it saw, but the plan is not permitted to be executed. The destructive
    path (`clean`, including `--dry-run`) keeps `for_mutation=True` so that a preview and a
    real run always select the same set.
    """
    if days < 1:
        raise ValueError("days must be >= 1")
    if keep_latest < 0:
        raise ValueError("keep_latest must be >= 0")

    cutoff = now_ts() - days * 86400
    plan = Plan(cutoff=cutoff, days=days, keep_latest=keep_latest, for_mutation=for_mutation)
    plan.root_authority = {
        "codex": fingerprint_root(codex_home, "codex"),
        "claude": fingerprint_root(claude_home, "claude"),
    }
    codex_scan = Scan(scope="codex")
    claude_scan = Scan(scope="claude")
    plan.scans = [scan for tool, scan in (("codex", codex_scan), ("claude", claude_scan)) if tool in tools]

    if "codex" in tools:
        if codex_backend in ("auto", "cli"):
            supported, _ = codex_delete_supported()
            strategy = "codex-cli" if supported else "unavailable"
        else:
            strategy = "filesystem"

        sessions = discover_codex_sessions(codex_home, strategy, codex_scan)
        selected = choose_codex_old_sessions(
            sessions,
            cutoff,
            keep_latest,
            collapse_subagents=(strategy != "filesystem"),
        )
        if strategy == "unavailable" and selected:
            plan.notes.append(
                "Codex has old sessions, but this installed CLI does not expose 'codex delete --force'. "
                "They are skipped unless --codex-backend filesystem is explicitly requested."
            )
        else:
            plan.actions.extend(selected)
        if strategy == "filesystem" and selected:
            plan.notes.append(
                "Codex filesystem fallback is enabled explicitly. It removes rollout JSONL files directly and "
                "may leave stale session metadata in Codex SQLite indexes; prefer the default auto backend on current Codex versions."
            )
        plan.actions.extend(discover_codex_aux(codex_home, cutoff, codex_scan))

    if "claude" in tools:
        sessions = discover_claude_sessions(claude_home, claude_scan)
        selected = choose_old_sessions(sessions, cutoff, keep_latest)
        selected_ids = {a.session_id for a in selected if a.session_id}
        protected_ids = {a.session_id for a in sessions if a.session_id and a.session_id not in selected_ids}
        plan.actions.extend(selected)
        plan.actions.extend(discover_claude_aux(claude_home, cutoff, aggressive, protected_ids, claude_scan))
        plan.claude_history_session_ids = selected_ids
        plan.claude_history_lines = count_claude_history_matches(claude_home / "history.jsonl", plan.claude_history_session_ids, claude_scan)

    # De-duplicate exact filesystem paths while preserving the first action.
    deduped: list[Action] = []
    seen: set[tuple[str, str]] = set()
    for action in plan.actions:
        key = (action.strategy, str(action.path.resolve(strict=False)))
        if key in seen:
            continue
        seen.add(key)
        deduped.append(action)

    # Protected-name barrier. Independent of the scanners above by design: it holds even
    # if a future discovery change starts emitting protected paths as candidates.
    roots = {"codex": codex_home, "claude": claude_home}
    kept: list[Action] = []
    blocked_names: set[str] = set()
    for action in deduped:
        if action.strategy == "codex-cli":
            # Deletion is delegated to Codex by session id, not by path.
            kept.append(action)
            continue
        hit = protected_component(action.path, roots[action.tool], protected_names_for(action.tool))
        if hit is None:
            kept.append(action)
        else:
            blocked_names.add(f"{action.tool}:{hit}")
    if blocked_names:
        plan.notes.append(
            "Refused "
            + str(len(deduped) - len(kept))
            + " candidate(s) covered by protected names: "
            + ", ".join(sorted(blocked_names))
            + ". This is a safety barrier, not a filter you can disable."
        )

    if for_mutation:
        # Two independent reasons to withhold destructive authority for a whole tool:
        # the root has not proved it is that provider, or we could not finish looking at it.
        withheld: set[str] = set()
        for tool in sorted(tools):
            authority = plan.root_authority[tool]
            if not authority.destructive_allowed():
                withheld.add(tool)
                plan.notes.append(authority.explain())
        for scan in plan.scans:
            if not scan.complete:
                withheld.add(scan.scope)
                detail = "; ".join(scan.errors[:3]) or "scan truncated"
                plan.notes.append(
                    f"Refusing destructive work on {scan.scope}: the scan was incomplete, so absence of evidence "
                    f"cannot mean absence of data ({len(scan.errors)} unreadable path(s), e.g. {detail}). "
                    "Run `status` to see the full list."
                )
        if withheld:
            before = len(kept)
            kept = [action for action in kept if action.tool not in withheld]
            plan.withheld = sorted(withheld)
            plan.notes.append(f"Withheld {before - len(kept)} candidate(s) for: {', '.join(plan.withheld)}.")

    plan.actions = kept
    return plan


def active_processes() -> ProcessObservation:
    """Best-effort exact-process-name detection.

    False negatives remain possible even on success, so this is never the sole safety
    control. What it must not do is report success when it failed: if `ps` is missing,
    fails, times out, or returns nothing parsable, the observation is marked incomplete and
    the caller treats provider activity as unknown rather than as absent.
    """
    targets = {"codex": {"codex", "Codex"}, "claude": {"claude"}}
    result: dict[str, list[int]] = {"codex": [], "claude": []}
    ps_bin = shutil.which("ps") or "/bin/ps"
    try:
        # Fixed argument list, no untrusted input; ps_bin is PATH-resolved above.
        proc = subprocess.run(  # noqa: S603
            [ps_bin, "-axo", "pid=,comm="],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            timeout=5,
            check=False,
        )
    except (OSError, subprocess.SubprocessError):
        return ProcessObservation(pids=result, complete=False)
    if proc.returncode != 0:
        return ProcessObservation(pids=result, complete=False)
    saw_self = False
    self_pid = os.getpid()
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            continue
        try:
            pid = int(parts[0])
        except ValueError:
            continue
        if pid == self_pid:
            saw_self = True
            continue
        base = Path(parts[1]).name
        for tool, names in targets.items():
            if base in names:
                result[tool].append(pid)
    # A full process listing necessarily contains this process. If it does not, we are not
    # looking at a full listing - filtered, truncated, sandboxed or stubbed output must not
    # be read as "nothing is running".
    return ProcessObservation(pids=result, complete=saw_self)


def safe_remove(path: Path, approved_root: Path, protected_names: set[str]) -> int:
    """Delete one path without following symlinks. Returns pre-delete size.

    `protected_names` is re-checked here, not only at plan time, so the barrier holds for
    every caller and at the last possible moment before the filesystem is touched.
    """
    root_resolved = approved_root.resolve(strict=False)
    hit = protected_component(path, approved_root, protected_names)
    if hit is not None:
        raise SafetyError(f"Refusing to delete protected {hit!r} state: {path}")
    try:
        st = path.lstat()
    except FileNotFoundError:
        return 0

    # A symlink may resolve outside the approved root. That is fine only for unlinking
    # the link itself, provided its real parent directory is inside the approved root.
    if stat.S_ISLNK(st.st_mode):
        parent = path.parent.resolve(strict=False)
        try:
            parent.relative_to(root_resolved)
        except ValueError as exc:
            raise SafetyError(f"Refusing to unlink symlink outside approved root: {path}") from exc
        if parent == root_resolved.parent and path.name == root_resolved.name:
            raise SafetyError(f"Refusing to delete config root itself: {path}")
        size = st.st_size
        path.unlink(missing_ok=True)
        return size

    if not is_within(path, approved_root):
        raise SafetyError(f"Refusing to delete path outside approved root: {path}")
    if path.resolve(strict=False) == root_resolved:
        raise SafetyError(f"Refusing to delete config root itself: {path}")

    size = safe_lstat_size(path)
    if stat.S_ISREG(st.st_mode):
        path.unlink(missing_ok=True)
    elif stat.S_ISDIR(st.st_mode):
        shutil.rmtree(path)
    else:
        path.unlink(missing_ok=True)
    return size


def prune_empty_dirs(root: Path) -> None:
    """Cosmetic post-mutation tidy-up. rmdir failing simply means the directory is in use."""
    if not root.exists() or root.is_symlink():
        return
    for base, dirs, _files in os.walk(root, topdown=False, followlinks=False):
        base_p = Path(base)
        for name in dirs:
            p = base_p / name
            if p.is_symlink():
                continue
            with contextlib.suppress(OSError):
                p.rmdir()


def delete_codex_via_cli(action: Action, codex_bin: str) -> tuple[bool, str]:
    if not action.session_id:
        raise ValueError("delete_codex_via_cli requires an action with a session_id")
    try:
        # session_id was extracted via UUID_RE upstream and codex_bin is PATH-resolved;
        # neither is untrusted/shell-interpreted input.
        proc = subprocess.run(  # noqa: S603
            [codex_bin, "delete", action.session_id, "--force"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=45,
            check=False,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        return False, str(exc)
    if proc.returncode == 0:
        return True, (proc.stdout or "").strip()
    return False, (proc.stdout or f"exit {proc.returncode}").strip()


def _history_identity(st: os.stat_result) -> tuple[int, int, int, int]:
    return (st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns)


def trim_claude_history(
    history_path: Path,
    deleted_session_ids: set[str],
    dry_run: bool = False,
) -> tuple[int, int, str]:
    """Remove history lines tied to successfully deleted session ids.

    Malformed lines are preserved. The rewrite streams instead of loading the file, and
    the source is re-identified immediately before the atomic replace: if a provider wrote
    to `history.jsonl` while we were copying it, the replace is abandoned rather than
    silently discarding the concurrent writer's lines.

    Returns (removed_lines, removed_bytes, status) where status is one of
    "noop", "trimmed", "dry-run", "unreadable" or "concurrent-modification".
    """
    if not deleted_session_ids or not history_path.exists():
        return 0, 0, "noop"
    if history_path.is_symlink():
        # os.replace() would swap the link for a regular file, silently detaching whatever
        # the operator pointed it at. Shared provider metadata is not rewritten through an
        # indirection we did not create.
        return 0, 0, "unreadable"
    deleted_session_ids = {s.lower() for s in deleted_session_ids}
    try:
        original_stat = history_path.stat()
    except OSError:
        return 0, 0, "unreadable"

    if dry_run:
        return count_claude_history_matches(history_path, deleted_session_ids), 0, "dry-run"

    history_path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=".history.", suffix=".tmp", dir=str(history_path.parent))
    removed = 0
    removed_bytes = 0
    try:
        try:
            with history_path.open("rb") as src, os.fdopen(fd, "wb") as out:
                for line in src:
                    try:
                        obj = json.loads(line.decode("utf-8", errors="replace"))
                        should_remove = str(obj.get("sessionId", "")).lower() in deleted_session_ids
                    except (json.JSONDecodeError, UnicodeDecodeError):
                        should_remove = False
                    if not isinstance(should_remove, bool):
                        should_remove = False
                    if should_remove:
                        removed += 1
                        removed_bytes += len(line)
                    else:
                        # Retained lines are copied verbatim: no newline translation, no
                        # re-encoding, no trailing-newline insertion.
                        out.write(line)
                out.flush()
                os.fsync(out.fileno())
        except OSError:
            return 0, 0, "unreadable"

        if removed == 0:
            return 0, 0, "noop"
        try:
            current_stat = history_path.stat()
        except OSError:
            return 0, 0, "concurrent-modification"
        if _history_identity(current_stat) != _history_identity(original_stat):
            return 0, 0, "concurrent-modification"
        os.chmod(tmp_name, stat.S_IMODE(original_stat.st_mode))
        os.replace(tmp_name, history_path)
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(tmp_name)
    return removed, removed_bytes, "trimmed"


def execute_plan(
    plan: Plan,
    *,
    codex_home: Path,
    claude_home: Path,
    dry_run: bool,
    allow_running: bool,
    trim_history: bool,
    verbose: bool,
) -> CleanResult:
    result = CleanResult()
    if dry_run:
        result.skipped = len(plan.actions)
        return result

    if not plan.for_mutation:
        raise SafetyError("Refusing to execute an inspection-only plan")

    # Defense in depth: build_plan already withheld these, so reaching here means a caller
    # assembled a plan another way. Re-derive both gates rather than trusting the plan.
    roots = {"codex": codex_home, "claude": claude_home}
    for tool in sorted({action.tool for action in plan.actions}):
        authority = fingerprint_root(roots[tool], tool)
        if not authority.destructive_allowed():
            raise SafetyError(authority.explain())
    for scan in plan.scans:
        if not scan.complete:
            raise SafetyError(f"Refusing to execute a plan built from an incomplete {scan.scope} scan")

    running = active_processes()
    target_tools = {action.tool for action in plan.actions}
    if running.complete:
        blocked_tools = {tool for tool in target_tools if running.running(tool) and not allow_running}
    else:
        # Unknown activity is not absence of activity.
        blocked_tools = set() if allow_running else set(target_tools)
        result.errors.append(
            "Could not determine whether Codex/Claude are running, so cleanup was skipped. Pass --allow-running if you accept the risk."
        )
    result.blocked_tools = set(blocked_tools)
    codex_ok, codex_bin = codex_delete_supported()

    before_size = 0
    if "codex" in target_tools:
        before_size += directory_size(codex_home)
    if "claude" in target_tools:
        before_size += directory_size(claude_home)

    for idx, action in enumerate(plan.actions, 1):
        if action.tool in blocked_tools:
            result.skipped += 1
            continue
        result.attempted += 1
        try:
            if action.tool == "codex" and action.strategy == "codex-cli":
                if not codex_ok or not codex_bin:
                    raise RuntimeError("Codex official delete backend became unavailable")
                ok, msg = delete_codex_via_cli(action, codex_bin)
                if not ok:
                    raise RuntimeError(msg or "codex delete failed")
            elif action.tool == "codex":
                safe_remove(action.path, codex_home, CODEX_PROTECTED_NAMES)
            else:
                # For a Claude session action, delete transcript + sibling session payload directory.
                if action.category == "session" and action.session_id:
                    companion = action.path.parent / action.path.stem
                    safe_remove(action.path, claude_home, CLAUDE_PROTECTED_NAMES)
                    if companion.exists() or companion.is_symlink():
                        safe_remove(companion, claude_home, CLAUDE_PROTECTED_NAMES)
                    result.deleted_claude_session_ids.add(action.session_id)
                else:
                    safe_remove(action.path, claude_home, CLAUDE_PROTECTED_NAMES)
            result.succeeded += 1
            if verbose:
                print(f"  deleted [{action.tool}/{action.category}] {action.path}")
        except Exception as exc:  # deliberate isolation: continue cleaning other independent items
            result.failed += 1
            result.errors.append(f"{action.tool}: {action.path}: {exc}")
        if verbose and idx % 50 == 0:
            print(f"  progress: {idx}/{len(plan.actions)} actions")

    if trim_history and result.deleted_claude_session_ids and "claude" not in blocked_tools:
        if not running.complete:
            result.deferred.append("Claude history trimming was skipped because provider activity could not be determined.")
        elif running.running("claude"):
            # history.jsonl is shared mutable provider metadata. --allow-running may permit
            # removing independent artifacts, but it never authorizes rewriting a file a
            # live provider is appending to.
            result.deferred.append(
                "Claude history trimming was skipped because a Claude process is running "
                f"(PID: {', '.join(str(pid) for pid in running.running('claude'))}). "
                "Deleted sessions may still be listed in history.jsonl."
            )
        else:
            removed_lines, _removed_bytes, trim_status = trim_claude_history(claude_home / "history.jsonl", result.deleted_claude_session_ids)
            if trim_status == "concurrent-modification":
                result.deferred.append(
                    "Claude history.jsonl changed while it was being rewritten; the trim was abandoned to avoid discarding concurrent writes."
                )
            elif trim_status == "unreadable":
                result.deferred.append("Claude history.jsonl could not be read or rewritten; deleted sessions may still be listed in it.")
            elif verbose and removed_lines:
                print(f"  trimmed {removed_lines} Claude history line(s) linked to deleted sessions")

    # Empty date/project/session directories are harmless but add visual clutter.
    if "codex" not in blocked_tools:
        prune_empty_dirs(codex_home / "sessions")
        prune_empty_dirs(codex_home / "archived_sessions")
    if "claude" not in blocked_tools:
        # Never remove project roots or memory; prune only known transient roots.
        for rel in CLAUDE_RETENTION_PATHS:
            prune_empty_dirs(claude_home / rel)

    after_size = 0
    if "codex" in target_tools:
        after_size += directory_size(codex_home)
    if "claude" in target_tools:
        after_size += directory_size(claude_home)
    result.freed_bytes = max(0, before_size - after_size)

    for tool in sorted(blocked_tools):
        pids = ", ".join(str(p) for p in running.running(tool))
        if not pids:
            continue
        result.errors.append(
            f"Skipped {tool} cleanup because a {tool} process appears to be running (PID: {pids}). "
            "Close it or pass --allow-running if you accept the risk."
        )
    result.errors.extend(result.deferred)
    return result


def root_entry_sizes(root: Path, scan: Scan | None = None) -> list[tuple[Path, int]]:
    """Size every top-level entry of a provider root in a single pass."""
    st = observe(root, scan)
    if st is None or stat.S_ISLNK(st.st_mode):
        return []
    try:
        children = list(root.iterdir())
    except OSError as exc:
        if scan is not None:
            scan.record(root, exc)
        return []
    return [(p, safe_lstat_size(p, scan)) for p in children]


def largest_entries(entries: Sequence[tuple[Path, int]], limit: int = 8) -> list[tuple[Path, int]]:
    return sorted(entries, key=lambda item: item[1], reverse=True)[:limit]


def coverage_state(name: str, tool: str) -> str:
    """Classify one top-level provider entry against what this build actually knows.

    The vocabulary is deliberately narrow so the report cannot overclaim:

    - `selective` - entries inside are selected by age and policy; this entry is never deleted;
    - `selective-aggressive` - the same, but only under `--aggressive`;
    - `aggressive-only` - this entry is deleted whole, only under `--aggressive`;
    - `trimmed` - never deleted; individual lines are rewritten when their session goes;
    - `protected` - an unconditional barrier covers it;
    - `reported` - shown in status, never touched;
    - `unknown` - this build does not classify it at all.

    There is deliberately no state meaning "this entry gets deleted as it stands", because
    no top-level provider entry is treated that way outside `--aggressive`.

    Reporting `unknown` is the point: a provider that adds a directory must show up as
    unclassified rather than disappearing from the picture. No discovery path reads this
    function, so no state here can create a cleanup candidate.
    """
    if name in protected_names_for(tool):
        return "protected"
    if tool == "codex":
        if name in CODEX_SELECTIVE_NAMES:
            return "selective"
        if name.startswith(CODEX_REPORTED_PREFIXES):
            return "reported"
        return "unknown"
    if name in CLAUDE_TRIMMED_NAMES:
        return "trimmed"
    if name in CLAUDE_SELECTIVE_NAMES:
        return "selective"
    if name in CLAUDE_SELECTIVE_AGGRESSIVE_NAMES:
        return "selective-aggressive"
    if name in CLAUDE_AGGRESSIVE_ONLY_NAMES:
        return "aggressive-only"
    return "unknown"


def coverage_report(entries: Sequence[tuple[Path, int]], tool: str) -> list[CoverageBucket]:
    grouped: dict[str, list[tuple[str, int]]] = {}
    for path, size in entries:
        grouped.setdefault(coverage_state(path.name, tool), []).append((path.name, size))
    return [
        CoverageBucket(
            state=state,
            entries=len(items),
            bytes=sum(size for _name, size in items),
            names=sorted(name for name, _size in items),
        )
        for state in COVERAGE_STATES
        if (items := grouped.get(state))
    ]


def coverage_payload(entries: Sequence[tuple[Path, int]], tool: str) -> dict[str, dict[str, object]]:
    return {bucket.state: {"entries": bucket.entries, "bytes": bucket.bytes, "names": bucket.names} for bucket in coverage_report(entries, tool)}


COVERAGE_LEGEND = {
    "selective": "entries inside are selected by age and policy; this entry is never deleted",
    "selective-aggressive": "the same, but only under --aggressive",
    "aggressive-only": "deleted whole, only under --aggressive",
    "trimmed": "never deleted; lines tied to deleted sessions are removed",
    "protected": "unconditional barrier, never deleted",
    "reported": "shown here, never touched",
    "unknown": "not classified by this build; never a cleanup candidate",
}


def print_coverage(tool: str, root: Path, entries: Sequence[tuple[Path, int]]) -> None:
    print(f"\n{tool} coverage  {root}")
    for bucket in coverage_report(entries, tool):
        print(f"  {bucket.state:15} {format_bytes(bucket.bytes):>10}  {bucket.entries:3} entry(ies)  - {COVERAGE_LEGEND[bucket.state]}")
        if bucket.state == "unknown":
            print(f"    {', '.join(bucket.names)}")
    print("  Unknown entries are reported so provider layout drift stays visible.")


def protected_codex_db_entries(codex_home: Path, scan: Scan | None = None) -> list[tuple[Path, int]]:
    if observe(codex_home, scan) is None:
        return []
    entries: list[tuple[Path, int]] = []
    try:
        for p in codex_home.iterdir():
            if p.is_file() and (
                p.name.startswith("state_")
                or p.name.startswith("logs_")
                or p.name.startswith("goals_")
                or p.name.startswith("memories_")
                or p.name.startswith("queue_")
                or p.name.startswith("thread_history_")
            ):
                entries.append((p, safe_lstat_size(p, scan)))
    except OSError as exc:
        if scan is not None:
            scan.record(codex_home, exc)
    return sorted(entries, key=lambda item: item[1], reverse=True)


def parse_tools(value: str) -> set[str]:
    if value == "all":
        return {"codex", "claude"}
    return {value}


def plan_summary_dict(plan: Plan) -> dict[str, object]:
    by_tool: dict[str, dict[str, int]] = {}
    by_category: dict[str, dict[str, int]] = {}
    for action in plan.actions:
        t = by_tool.setdefault(action.tool, {"actions": 0, "bytes": 0})
        t["actions"] += 1
        t["bytes"] += action.size
        key = f"{action.tool}:{action.category}"
        c = by_category.setdefault(key, {"actions": 0, "bytes": 0})
        c["actions"] += 1
        c["bytes"] += action.size
    return {
        "days": plan.days,
        "keep_latest": plan.keep_latest,
        "cutoff": datetime.fromtimestamp(plan.cutoff, tz=timezone.utc).isoformat(),
        "estimated_bytes": plan.estimated_bytes,
        "actions": len(plan.actions),
        "by_tool": by_tool,
        "by_category": by_category,
        "claude_history_lines": plan.claude_history_lines,
        "notes": plan.notes,
        "withheld_tools": plan.withheld,
        "scan": {
            "complete": plan.scan_complete,
            "incomplete_scopes": plan.incomplete_scopes,
            "unreadable": plan.scan_errors,
        },
        "roots": {
            tool: {
                "path": str(authority.path),
                "origin": authority.origin,
                "confidence": authority.confidence,
                "markers": list(authority.markers),
                "destructive_allowed": authority.destructive_allowed(),
            }
            for tool, authority in sorted(plan.root_authority.items())
        },
    }


def print_plan(plan: Plan, *, show_paths: bool, max_paths: int = 20) -> None:
    print(f"Retention: {plan.days} day(s) | keep latest: {plan.keep_latest} session(s) per tool")
    print(f"Candidates: {len(plan.actions)} action(s) | estimated payload: {format_bytes(plan.estimated_bytes)}")
    grouped: dict[tuple[str, str], list[Action]] = {}
    for action in plan.actions:
        grouped.setdefault((action.tool, action.category), []).append(action)
    for (tool, category), actions in sorted(grouped.items()):
        total = sum(a.size for a in actions)
        print(f"  {tool:6}  {category:20} {len(actions):5}  {format_bytes(total):>10}")
    if plan.claude_history_lines:
        print(f"  claude  history.jsonl        {plan.claude_history_lines:5}  linked prompt line(s) to trim")
    if not plan.scan_complete:
        print(f"SCAN INCOMPLETE: {len(plan.scan_errors)} unreadable path(s) in {', '.join(plan.incomplete_scopes)}")
        for error in plan.scan_errors[:10]:
            print(f"  unreadable: {error}")
        if len(plan.scan_errors) > 10:
            print(f"  ... and {len(plan.scan_errors) - 10} more")
    for note in plan.notes:
        print(f"NOTE: {note}")
    if show_paths and plan.actions:
        print("\nLargest candidates:")
        for action in sorted(plan.actions, key=lambda a: a.size, reverse=True)[:max_paths]:
            print(f"  {format_bytes(action.size):>10}  {format_age(action.mtime):>7}  [{action.tool}/{action.category}] {action.path}")


def configure_claude_retention(claude_home: Path, days: int) -> Path:
    if days < 1:
        raise ValueError("Claude cleanupPeriodDays must be >= 1")
    claude_home.mkdir(parents=True, exist_ok=True)
    settings = claude_home / "settings.json"
    data: dict[str, object] = {}
    mode = 0o600
    if settings.exists():
        try:
            data = json.loads(settings.read_text(encoding="utf-8"))
            if not isinstance(data, dict):
                raise ValueError("settings.json root must be a JSON object")
            mode = stat.S_IMODE(settings.stat().st_mode)
        except json.JSONDecodeError as exc:
            raise ValueError(f"Refusing to modify invalid JSON in {settings}: {exc}") from exc
    data["cleanupPeriodDays"] = days
    atomic_write_json(settings, data, mode)
    return settings


def atomic_write_json(path: Path, data: dict[str, object], mode: int = 0o600) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=str(path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2, ensure_ascii=False)
            fh.write("\n")
            fh.flush()
            os.fsync(fh.fileno())
        os.chmod(tmp_name, mode)
        os.replace(tmp_name, path)
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(tmp_name)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="cancellai",
        description="Safely reclaim disk space from old Codex and Claude Code sessions.",
    )
    parser.add_argument("--version", action="version", version=f"%(prog)s {VERSION}")
    sub = parser.add_subparsers(dest="command")

    def add_common(p: argparse.ArgumentParser) -> None:
        p.add_argument("--days", type=int, default=DEFAULT_DAYS, help=f"Delete data older than N days (default: {DEFAULT_DAYS})")
        p.add_argument(
            "--keep-latest",
            type=int,
            default=DEFAULT_KEEP_LATEST,
            help=f"Always keep at least N newest sessions per tool (default: {DEFAULT_KEEP_LATEST})",
        )
        p.add_argument("--tool", choices=["all", "codex", "claude"], default="all")
        p.add_argument(
            "--codex-backend",
            choices=["auto", "cli", "filesystem"],
            default="auto",
            help="auto/cli prefer official 'codex delete --force'; filesystem is an explicit unsafe-compatibility fallback",
        )
        p.add_argument("--aggressive", action="store_true", help="Also remove Claude legacy/rebuildable caches")
        p.add_argument("--json", action="store_true", help="Machine-readable summary")

    clean = sub.add_parser("clean", help="Clean old session data (default when other args are given without a subcommand)")
    add_common(clean)
    clean.add_argument("--dry-run", action="store_true", help="Preview only; delete nothing")
    clean.add_argument("-y", "--yes", action="store_true", help="Skip confirmation")
    clean.add_argument("--allow-running", action="store_true", help="Allow cleanup even if Codex/Claude processes appear active")
    clean.add_argument("--keep-claude-history", action="store_true", help="Do not trim prompt-history lines tied to deleted Claude sessions")
    clean.add_argument("--verbose", action="store_true")

    status = sub.add_parser("status", help="Show disk usage and cleanup candidates (default command)")
    add_common(status)
    status.add_argument("--paths", action="store_true", help="Show largest candidate paths")
    status.add_argument(
        "--coverage",
        action="store_true",
        help="Report which top-level provider entries this build classifies, including unknown ones",
    )
    status.add_argument("--top", type=int, default=8, help="Number of top-level disk consumers to show")

    configure = sub.add_parser("configure", help="Configure Claude Code's built-in retention")
    configure.add_argument("--claude-retention", type=int, required=True, metavar="DAYS")

    sub.add_parser("version", help="Print version")
    return parser


def normalize_argv(argv: Sequence[str]) -> list[str]:
    """Resolve a missing subcommand toward observation, never toward deletion.

    Earlier releases turned `cancellai --days 14` into `clean --days 14`. Destructive
    intent must be typed, so a leading flag now selects the read-only `status` view and an
    unrecognized verb is left to argparse as a usage error.
    """
    args = list(argv)
    if not args:
        return ["status"]
    # Global --version/--help remain global.
    if args[0] in {"--version", "-h", "--help"}:
        return args
    if args[0] in KNOWN_COMMANDS:
        return args
    if args[0].startswith("-"):
        return ["status", *args]
    return args


def confirm(prompt: str) -> bool:
    try:
        answer = input(f"{prompt} [y/N] ").strip().lower()
    except (EOFError, KeyboardInterrupt):
        print()
        return False
    return answer in {"y", "yes"}


def cmd_status(args: argparse.Namespace) -> int:
    try:
        codex_home = validate_config_root(get_codex_home(), "Codex")
        claude_home = validate_config_root(get_claude_home(), "Claude")
        plan = build_plan(
            days=args.days,
            keep_latest=args.keep_latest,
            tools=parse_tools(args.tool),
            codex_home=codex_home,
            claude_home=claude_home,
            codex_backend=args.codex_backend,
            aggressive=args.aggressive,
            for_mutation=False,
        )
    except (SafetyError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return EXIT_USAGE

    root_scans = {"codex": Scan(scope="codex root"), "claude": Scan(scope="claude root")}
    codex_entries = root_entry_sizes(codex_home, root_scans["codex"])
    claude_entries = root_entry_sizes(claude_home, root_scans["claude"])
    codex_bytes = sum(size for _p, size in codex_entries)
    claude_bytes = sum(size for _p, size in claude_entries)

    roots: dict[str, dict[str, object]] = {}
    for tool, entries, total in (("codex", codex_entries, codex_bytes), ("claude", claude_entries, claude_bytes)):
        authority = plan.root_authority[tool]
        roots[tool] = {
            "path": str(authority.path),
            "origin": authority.origin,
            "confidence": authority.confidence,
            "markers": list(authority.markers),
            "destructive_allowed": authority.destructive_allowed(),
            "bytes": total,
            "bytes_complete": root_scans[tool].complete,
            "unreadable": root_scans[tool].errors,
            "coverage": coverage_payload(entries, tool),
        }

    if args.json:
        payload = plan_summary_dict(plan)
        payload["roots"] = roots
        observation = active_processes()
        payload["running"] = {"pids": observation.pids, "observed": observation.complete}
        print(json.dumps(payload, indent=2))
        return EXIT_OK

    def total_label(tool: str, total: int) -> str:
        # An incomplete traversal produces a lower bound, and must not be printed as a fact.
        return f"{format_bytes(total)}{'' if root_scans[tool].complete else ' (at least; scan incomplete)'}"

    print("cancellAI status")
    print(f"Codex:  {codex_home}  {total_label('codex', codex_bytes)}")
    print(f"Claude: {claude_home}  {total_label('claude', claude_bytes)}")
    for tool, scan in root_scans.items():
        if not scan.complete:
            print(f"WARNING: {len(scan.errors)} unreadable path(s) under the {tool} root; reported sizes are lower bounds.")
            for error in scan.errors[:5]:
                print(f"  unreadable: {error}")
    for tool in ("codex", "claude"):
        authority = plan.root_authority[tool]
        if not authority.destructive_allowed():
            print(f"WARNING: {authority.explain()}")
    running = active_processes()
    if not running.complete:
        print("Running processes: unknown (process enumeration failed; cleanup will refuse to run)")
    elif running.any_running:
        print(f"Running processes: codex={running.running('codex') or '-'} claude={running.running('claude') or '-'}")
    print()
    print_plan(plan, show_paths=args.paths)

    print("\nLargest top-level Codex entries:")
    for p, size in largest_entries(codex_entries, args.top):
        print(f"  {format_bytes(size):>10}  {p.name}")
    print("Largest top-level Claude entries:")
    for p, size in largest_entries(claude_entries, args.top):
        print(f"  {format_bytes(size):>10}  {p.name}")

    protected = protected_codex_db_entries(codex_home, root_scans["codex"])
    big = [(p, s) for p, s in protected if s >= 100 * 1024 * 1024]
    if big:
        print("\nProtected Codex SQLite state (reported, never deleted automatically):")
        for p, size in big:
            print(f"  {format_bytes(size):>10}  {p.name}")

    if args.coverage:
        print_coverage("codex", codex_home, codex_entries)
        print_coverage("claude", claude_home, claude_entries)
    return EXIT_OK


def cmd_clean(args: argparse.Namespace) -> int:
    try:
        codex_home = validate_config_root(get_codex_home(), "Codex")
        claude_home = validate_config_root(get_claude_home(), "Claude")
        plan = build_plan(
            days=args.days,
            keep_latest=args.keep_latest,
            tools=parse_tools(args.tool),
            codex_home=codex_home,
            claude_home=claude_home,
            codex_backend=args.codex_backend,
            aggressive=args.aggressive,
        )
    except (SafetyError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return EXIT_USAGE

    if args.json and not args.dry_run and not args.yes:
        print("ERROR: --json with destructive clean requires --yes or --dry-run", file=sys.stderr)
        return EXIT_USAGE

    if args.json and args.dry_run:
        dry_code = EXIT_BLOCKED if plan.withheld else EXIT_OK
        print(json.dumps({"dry_run": True, "exit_code": dry_code, **plan_summary_dict(plan)}, indent=2))
        return dry_code

    if not args.json:
        print_plan(plan, show_paths=args.dry_run or args.verbose)

    if not plan.actions:
        empty_code = EXIT_BLOCKED if plan.withheld else EXIT_OK
        if args.json:
            print(
                json.dumps(
                    {
                        "dry_run": bool(args.dry_run),
                        "exit_code": empty_code,
                        **plan_summary_dict(plan),
                        "result": "safety-withheld" if plan.withheld else "nothing-to-do",
                    },
                    indent=2,
                )
            )
        else:
            print("Nothing to clean." if not plan.withheld else "Nothing was cleaned: safety withheld the requested work.")
        return empty_code

    if args.dry_run:
        if not args.json:
            print("\nDry-run only. No files were changed.")
        return EXIT_BLOCKED if plan.withheld else EXIT_OK

    if not args.yes:
        warning = f"Delete {len(plan.actions)} old item(s), approximately {format_bytes(plan.estimated_bytes)}? This cannot be undone."
        if not confirm(warning):
            print("Cancelled.")
            return EXIT_CANCELLED

    try:
        result = execute_plan(
            plan,
            codex_home=codex_home,
            claude_home=claude_home,
            dry_run=False,
            allow_running=args.allow_running,
            trim_history=not args.keep_claude_history,
            verbose=args.verbose and not args.json,
        )
    except SafetyError as exc:
        # A boundary that re-fired between planning and execution is a safety block, not a
        # crash: it must reach automation through the documented exit code and JSON shape.
        result = CleanResult()
        result.deferred.append(str(exc))
        result.errors.append(str(exc))
    except OSError as exc:
        # An I/O failure that escapes the per-action isolation is a mutation failure, and
        # automation must see exit 3 rather than a traceback and Python's own exit 1 -
        # which would be indistinguishable from the user declining the prompt.
        result = CleanResult()
        result.failed = 1
        result.errors.append(f"Cleanup aborted: {exc}")
    if plan.withheld:
        withheld_note = f"Safety withheld all work for: {', '.join(plan.withheld)}. See the notes above."
        result.deferred.append(withheld_note)
        result.errors.append(withheld_note)

    # A run that safety refused to perform is not a success. Automation must be able to
    # tell "cleaned" from "deliberately did not clean" without parsing warning text.
    if result.failed:
        exit_code = EXIT_FAILED
    elif result.partial:
        exit_code = EXIT_BLOCKED
    else:
        exit_code = EXIT_OK

    if args.json:
        payload = {
            "dry_run": False,
            "exit_code": exit_code,
            **plan_summary_dict(plan),
            "result": {
                "attempted": result.attempted,
                "succeeded": result.succeeded,
                "failed": result.failed,
                "skipped": result.skipped,
                "blocked_tools": sorted(result.blocked_tools),
                "deferred": result.deferred,
                "freed_bytes": result.freed_bytes,
                "errors": result.errors,
            },
        }
        print(json.dumps(payload, indent=2))
    else:
        print("\nCleanup complete" if exit_code == EXIT_OK else "\nCleanup incomplete")
        print(f"  succeeded: {result.succeeded}")
        print(f"  failed:    {result.failed}")
        print(f"  skipped:   {result.skipped}")
        print(f"  reclaimed: {format_bytes(result.freed_bytes)}")
        for err in result.errors:
            print(f"  WARNING: {err}")
        if exit_code != EXIT_OK:
            print(f"  exit code: {exit_code}")
    return exit_code


def cmd_configure(args: argparse.Namespace) -> int:
    try:
        claude_home = validate_config_root(get_claude_home(), "Claude")
        # Writing provider configuration is a mutation, so it uses the same root boundary.
        authority = fingerprint_root(claude_home, "claude")
        if not authority.destructive_allowed():
            raise SafetyError(authority.explain())
        settings = configure_claude_retention(claude_home, args.claude_retention)
    except (SafetyError, ValueError, OSError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return EXIT_USAGE
    print(f"Set Claude Code cleanupPeriodDays={args.claude_retention} in {settings}")
    return EXIT_OK


def main(argv: Sequence[str] | None = None) -> int:
    argv = normalize_argv(sys.argv[1:] if argv is None else argv)
    parser = build_parser()
    args = parser.parse_args(argv)
    command = args.command or "status"
    try:
        if command == "status":
            return cmd_status(args)
        if command == "configure":
            return cmd_configure(args)
        if command == "version":
            print(VERSION)
            return EXIT_OK
        return cmd_clean(args)
    except KeyboardInterrupt:
        print("\nInterrupted.", file=sys.stderr)
        return EXIT_CANCELLED
    except Exception as exc:  # the exit taxonomy must cover every path, including a bug in this program
        # An uncaught exception would leave Python's own exit code 1, which automation
        # cannot distinguish from the user declining the confirmation prompt.
        print(f"ERROR: unexpected failure: {type(exc).__name__}: {exc}", file=sys.stderr)
        return EXIT_FAILED


if __name__ == "__main__":
    raise SystemExit(main())
