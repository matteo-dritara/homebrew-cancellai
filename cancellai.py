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
from collections.abc import Iterator, Sequence
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path

VERSION = "1.0.1"
DEFAULT_DAYS = 7
DEFAULT_KEEP_LATEST = 2
UUID_RE = re.compile(r"(?P<uuid>[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})")
KNOWN_COMMANDS = {"clean", "status", "configure", "version"}

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


def get_codex_home() -> Path:
    raw = os.environ.get("CODEX_HOME")
    return Path(raw).expanduser() if raw else Path.home() / ".codex"


def get_claude_home() -> Path:
    raw = os.environ.get("CLAUDE_CONFIG_DIR")
    return Path(raw).expanduser() if raw else Path.home() / ".claude"


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


def safe_lstat_size(path: Path) -> int:
    try:
        st = path.lstat()
    except (FileNotFoundError, PermissionError, OSError):
        return 0
    if stat.S_ISLNK(st.st_mode):
        return st.st_size
    if stat.S_ISREG(st.st_mode):
        return st.st_size
    if stat.S_ISDIR(st.st_mode):
        return directory_size(path)
    return 0


def directory_size(root: Path) -> int:
    if not root.exists() and not root.is_symlink():
        return 0
    try:
        st = root.lstat()
    except OSError:
        return 0
    if stat.S_ISLNK(st.st_mode):
        return st.st_size
    if stat.S_ISREG(st.st_mode):
        return st.st_size
    total = 0
    try:
        for base, dirs, files in os.walk(root, followlinks=False):
            base_p = Path(base)
            # Do not follow directory symlinks. os.walk places them in dirs.
            keep_dirs: list[str] = []
            for name in dirs:
                p = base_p / name
                try:
                    lst = p.lstat()
                    if stat.S_ISLNK(lst.st_mode):
                        total += lst.st_size
                    else:
                        keep_dirs.append(name)
                except OSError:
                    continue
            dirs[:] = keep_dirs
            for name in files:
                p = base_p / name
                with contextlib.suppress(OSError):
                    total += p.lstat().st_size
    except OSError:
        pass
    return total


def latest_mtime(path: Path) -> float:
    """Return latest mtime within a tree without following symlinks."""
    try:
        st = path.lstat()
    except OSError:
        return 0.0
    latest = st.st_mtime
    if stat.S_ISLNK(st.st_mode) or stat.S_ISREG(st.st_mode):
        return latest
    try:
        for base, dirs, files in os.walk(path, followlinks=False):
            base_p = Path(base)
            keep_dirs: list[str] = []
            for name in dirs:
                p = base_p / name
                try:
                    lst = p.lstat()
                except OSError:
                    continue
                latest = max(latest, lst.st_mtime)
                if not stat.S_ISLNK(lst.st_mode):
                    keep_dirs.append(name)
            dirs[:] = keep_dirs
            for name in files:
                with contextlib.suppress(OSError):
                    latest = max(latest, (base_p / name).lstat().st_mtime)
    except OSError:
        pass
    return latest


def iter_files(root: Path, suffix: str | None = None) -> Iterator[Path]:
    if not root.exists() or root.is_symlink():
        return
    for base, dirs, files in os.walk(root, followlinks=False):
        base_p = Path(base)
        dirs[:] = [d for d in dirs if not (base_p / d).is_symlink()]
        for name in files:
            p = base_p / name
            if suffix is None or name.endswith(suffix):
                yield p


def extract_uuid(text: str) -> str | None:
    matches = list(UUID_RE.finditer(text))
    return matches[-1].group("uuid").lower() if matches else None


def read_codex_parent_session_id(path: Path) -> str | None:
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
    except OSError:
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


def discover_codex_sessions(codex_home: Path, strategy: str) -> list[Action]:
    found: list[Action] = []
    for rel, category in (("sessions", "session"), ("archived_sessions", "archived-session")):
        root = codex_home / rel
        if not root.exists():
            continue
        for p in iter_files(root, suffix=".jsonl"):
            if not p.name.startswith("rollout-"):
                continue
            sid = extract_uuid(p.name)
            if not sid:
                continue
            try:
                st = p.lstat()
            except OSError:
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
                    parent_session_id=read_codex_parent_session_id(p),
                )
            )
    return found


def discover_claude_sessions(claude_home: Path) -> list[Action]:
    projects = claude_home / "projects"
    found: list[Action] = []
    if not projects.exists() or projects.is_symlink():
        return found

    # Top-level project transcript files only. memory/ and subagent JSONL are never treated as roots.
    try:
        project_dirs = [p for p in projects.iterdir() if p.is_dir() and not p.is_symlink()]
    except OSError:
        return found

    for project_dir in project_dirs:
        try:
            children = list(project_dir.iterdir())
        except OSError:
            continue
        for p in children:
            if not p.is_file() or p.is_symlink() or p.suffix != ".jsonl":
                continue
            sid = extract_uuid(p.stem)
            if not sid:
                continue
            try:
                st = p.stat()
            except OSError:
                continue
            companion = project_dir / p.stem
            size = st.st_size
            mt = st.st_mtime
            if companion.exists() and companion.is_dir() and not companion.is_symlink():
                size += directory_size(companion)
                mt = max(mt, latest_mtime(companion))
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
) -> list[Action]:
    if not root.exists() or root.is_symlink():
        return []
    protected_session_ids = protected_session_ids or set()
    actions: list[Action] = []
    try:
        entries = list(root.iterdir())
    except OSError:
        return actions
    for p in entries:
        sid = extract_uuid(p.name)
        if sid and sid in protected_session_ids:
            continue
        mt = latest_mtime(p)
        if mt <= 0 or mt >= cutoff:
            continue
        actions.append(
            Action(
                tool=tool,
                category=category,
                path=p,
                size=safe_lstat_size(p),
                mtime=mt,
                session_id=sid,
                strategy="filesystem",
            )
        )
    return actions


def discover_codex_aux(codex_home: Path, cutoff: float) -> list[Action]:
    actions: list[Action] = []
    for rel, category in (("log", "old-log"), ("tmp", "old-temp")):
        actions.extend(discover_aged_top_entries(codex_home / rel, "codex", category, cutoff))
    return actions


def discover_claude_aux(
    claude_home: Path,
    cutoff: float,
    aggressive: bool,
    protected_session_ids: set[str] | None = None,
) -> list[Action]:
    actions: list[Action] = []
    for rel in CLAUDE_RETENTION_PATHS:
        actions.extend(discover_aged_top_entries(claude_home / rel, "claude", rel, cutoff, protected_session_ids))

    if aggressive:
        actions.extend(discover_aged_top_entries(claude_home / "backups", "claude", "backups", cutoff, protected_session_ids))
        # Legacy directories are no longer written by current Claude Code.
        for rel in CLAUDE_LEGACY_PATHS:
            root = claude_home / rel
            if root.exists() and not root.is_symlink():
                actions.append(
                    Action(
                        tool="claude",
                        category=f"legacy-{rel}",
                        path=root,
                        size=directory_size(root),
                        mtime=latest_mtime(root),
                    )
                )
        for rel in CLAUDE_SAFE_CACHE_FILES:
            p = claude_home / rel
            if p.exists() and (p.is_file() or p.is_symlink()):
                try:
                    st = p.lstat()
                except OSError:
                    continue
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


def count_claude_history_matches(history_path: Path, session_ids: set[str]) -> int:
    if not session_ids or not history_path.exists():
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
    except OSError:
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
) -> Plan:
    if days < 1:
        raise ValueError("days must be >= 1")
    if keep_latest < 0:
        raise ValueError("keep_latest must be >= 0")

    cutoff = now_ts() - days * 86400
    plan = Plan(cutoff=cutoff, days=days, keep_latest=keep_latest)

    if "codex" in tools:
        if codex_backend in ("auto", "cli"):
            supported, _ = codex_delete_supported()
            strategy = "codex-cli" if supported else "unavailable"
        else:
            strategy = "filesystem"

        sessions = discover_codex_sessions(codex_home, strategy)
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
        plan.actions.extend(discover_codex_aux(codex_home, cutoff))

    if "claude" in tools:
        sessions = discover_claude_sessions(claude_home)
        selected = choose_old_sessions(sessions, cutoff, keep_latest)
        selected_ids = {a.session_id for a in selected if a.session_id}
        protected_ids = {a.session_id for a in sessions if a.session_id and a.session_id not in selected_ids}
        plan.actions.extend(selected)
        plan.actions.extend(discover_claude_aux(claude_home, cutoff, aggressive, protected_session_ids=protected_ids))
        plan.claude_history_session_ids = selected_ids
        plan.claude_history_lines = count_claude_history_matches(claude_home / "history.jsonl", plan.claude_history_session_ids)

    # De-duplicate exact filesystem paths while preserving the first action.
    deduped: list[Action] = []
    seen: set[tuple[str, str]] = set()
    for action in plan.actions:
        key = (action.strategy, str(action.path.resolve(strict=False)))
        if key in seen:
            continue
        seen.add(key)
        deduped.append(action)
    plan.actions = deduped
    return plan


def active_processes() -> dict[str, list[int]]:
    """Best-effort exact-process-name detection. False negatives are possible; never used as sole safety control."""
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
        return result
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
            continue
        base = Path(parts[1]).name
        for tool, names in targets.items():
            if base in names:
                result[tool].append(pid)
    return result


def safe_remove(path: Path, approved_root: Path) -> int:
    """Delete one path without following symlinks. Returns pre-delete size."""
    root_resolved = approved_root.resolve(strict=False)
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


def trim_claude_history(history_path: Path, deleted_session_ids: set[str], dry_run: bool = False) -> tuple[int, int]:
    """Remove history lines tied to successfully deleted session ids. Malformed lines are preserved."""
    if not deleted_session_ids or not history_path.exists():
        return 0, 0
    deleted_session_ids = {s.lower() for s in deleted_session_ids}
    try:
        original_stat = history_path.stat()
        lines = history_path.read_text(encoding="utf-8", errors="replace").splitlines(keepends=True)
    except OSError:
        return 0, 0

    kept: list[str] = []
    removed = 0
    removed_bytes = 0
    for line in lines:
        should_remove = False
        try:
            obj = json.loads(line)
            sid = str(obj.get("sessionId", "")).lower()
            should_remove = sid in deleted_session_ids
        except json.JSONDecodeError:
            should_remove = False
        if should_remove:
            removed += 1
            removed_bytes += len(line.encode("utf-8", errors="replace"))
        else:
            kept.append(line)

    if removed == 0 or dry_run:
        return removed, removed_bytes

    history_path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp_name = tempfile.mkstemp(prefix=".history.", suffix=".tmp", dir=str(history_path.parent))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.writelines(kept)
            fh.flush()
            os.fsync(fh.fileno())
        os.chmod(tmp_name, stat.S_IMODE(original_stat.st_mode))
        os.replace(tmp_name, history_path)
    finally:
        with contextlib.suppress(FileNotFoundError):
            os.unlink(tmp_name)
    return removed, removed_bytes


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

    running = active_processes()
    target_tools = {action.tool for action in plan.actions}
    blocked_tools = {tool for tool, pids in running.items() if tool in target_tools and pids and not allow_running}
    codex_ok, codex_bin = codex_delete_supported()

    before_size = 0
    if "codex" in target_tools and codex_home.exists():
        before_size += directory_size(codex_home)
    if "claude" in target_tools and claude_home.exists():
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
                safe_remove(action.path, codex_home)
            else:
                # For a Claude session action, delete transcript + sibling session payload directory.
                if action.category == "session" and action.session_id:
                    companion = action.path.parent / action.path.stem
                    safe_remove(action.path, claude_home)
                    if companion.exists() or companion.is_symlink():
                        safe_remove(companion, claude_home)
                    result.deleted_claude_session_ids.add(action.session_id)
                else:
                    safe_remove(action.path, claude_home)
            result.succeeded += 1
            if verbose:
                print(f"  deleted [{action.tool}/{action.category}] {action.path}")
        except Exception as exc:  # deliberate isolation: continue cleaning other independent items
            result.failed += 1
            result.errors.append(f"{action.tool}: {action.path}: {exc}")
        if verbose and idx % 50 == 0:
            print(f"  progress: {idx}/{len(plan.actions)} actions")

    if trim_history and result.deleted_claude_session_ids and "claude" not in blocked_tools:
        removed_lines, _removed_bytes = trim_claude_history(claude_home / "history.jsonl", result.deleted_claude_session_ids)
        if verbose and removed_lines:
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
    if "codex" in target_tools and codex_home.exists():
        after_size += directory_size(codex_home)
    if "claude" in target_tools and claude_home.exists():
        after_size += directory_size(claude_home)
    result.freed_bytes = max(0, before_size - after_size)

    for tool in sorted(blocked_tools):
        pids = ", ".join(str(p) for p in running[tool])
        result.errors.append(
            f"Skipped {tool} cleanup because a {tool} process appears to be running (PID: {pids}). "
            "Close it or pass --allow-running if you accept the risk."
        )
    return result


def largest_entries(root: Path, limit: int = 8) -> list[tuple[Path, int]]:
    if not root.exists() or root.is_symlink():
        return []
    entries: list[tuple[Path, int]] = []
    try:
        children = list(root.iterdir())
    except OSError:
        return []
    for p in children:
        entries.append((p, safe_lstat_size(p)))
    entries.sort(key=lambda item: item[1], reverse=True)
    return entries[:limit]


def protected_codex_db_entries(codex_home: Path) -> list[tuple[Path, int]]:
    if not codex_home.exists():
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
                entries.append((p, safe_lstat_size(p)))
    except OSError:
        pass
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
    status.add_argument("--top", type=int, default=8, help="Number of top-level disk consumers to show")

    configure = sub.add_parser("configure", help="Configure Claude Code's built-in retention")
    configure.add_argument("--claude-retention", type=int, required=True, metavar="DAYS")

    sub.add_parser("version", help="Print version")
    return parser


def normalize_argv(argv: Sequence[str]) -> list[str]:
    args = list(argv)
    if not args:
        # No subcommand and no flags: default to the non-destructive status view.
        return ["status"]
    # Global --version remains global.
    if args[0] in {"--version", "-h", "--help"}:
        return args
    if args[0] not in KNOWN_COMMANDS:
        return ["clean", *args]
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
        )
    except (SafetyError, ValueError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2

    if args.json:
        payload = plan_summary_dict(plan)
        payload["roots"] = {
            "codex": {"path": str(codex_home), "bytes": directory_size(codex_home)},
            "claude": {"path": str(claude_home), "bytes": directory_size(claude_home)},
        }
        payload["running"] = active_processes()
        print(json.dumps(payload, indent=2))
        return 0

    print("cancellAI status")
    print(f"Codex:  {codex_home}  {format_bytes(directory_size(codex_home))}")
    print(f"Claude: {claude_home}  {format_bytes(directory_size(claude_home))}")
    running = active_processes()
    if running["codex"] or running["claude"]:
        print(f"Running processes: codex={running['codex'] or '-'} claude={running['claude'] or '-'}")
    print()
    print_plan(plan, show_paths=args.paths)

    print("\nLargest top-level Codex entries:")
    for p, size in largest_entries(codex_home, args.top):
        print(f"  {format_bytes(size):>10}  {p.name}")
    print("Largest top-level Claude entries:")
    for p, size in largest_entries(claude_home, args.top):
        print(f"  {format_bytes(size):>10}  {p.name}")

    protected = protected_codex_db_entries(codex_home)
    big = [(p, s) for p, s in protected if s >= 100 * 1024 * 1024]
    if big:
        print("\nProtected Codex SQLite state (reported, never deleted automatically):")
        for p, size in big:
            print(f"  {format_bytes(size):>10}  {p.name}")
    return 0


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
        return 2

    if args.json and not args.dry_run and not args.yes:
        print("ERROR: --json with destructive clean requires --yes or --dry-run", file=sys.stderr)
        return 2

    if args.json and args.dry_run:
        print(json.dumps({"dry_run": True, **plan_summary_dict(plan)}, indent=2))
        return 0

    if not args.json:
        print_plan(plan, show_paths=args.dry_run or args.verbose)

    if not plan.actions:
        if args.json:
            print(json.dumps({"dry_run": bool(args.dry_run), **plan_summary_dict(plan), "result": "nothing-to-do"}, indent=2))
        else:
            print("Nothing to clean.")
        return 0

    if args.dry_run:
        if not args.json:
            print("\nDry-run only. No files were changed.")
        return 0

    if not args.yes:
        warning = f"Delete {len(plan.actions)} old item(s), approximately {format_bytes(plan.estimated_bytes)}? This cannot be undone."
        if not confirm(warning):
            print("Cancelled.")
            return 1

    result = execute_plan(
        plan,
        codex_home=codex_home,
        claude_home=claude_home,
        dry_run=False,
        allow_running=args.allow_running,
        trim_history=not args.keep_claude_history,
        verbose=args.verbose and not args.json,
    )

    if args.json:
        payload = {
            "dry_run": False,
            **plan_summary_dict(plan),
            "result": {
                "attempted": result.attempted,
                "succeeded": result.succeeded,
                "failed": result.failed,
                "skipped": result.skipped,
                "freed_bytes": result.freed_bytes,
                "errors": result.errors,
            },
        }
        print(json.dumps(payload, indent=2))
    else:
        print("\nCleanup complete")
        print(f"  succeeded: {result.succeeded}")
        print(f"  failed:    {result.failed}")
        print(f"  skipped:   {result.skipped}")
        print(f"  reclaimed: {format_bytes(result.freed_bytes)}")
        for err in result.errors:
            print(f"  WARNING: {err}")
    return 2 if result.failed else 0


def cmd_configure(args: argparse.Namespace) -> int:
    try:
        claude_home = validate_config_root(get_claude_home(), "Claude")
        settings = configure_claude_retention(claude_home, args.claude_retention)
    except (SafetyError, ValueError, OSError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 2
    print(f"Set Claude Code cleanupPeriodDays={args.claude_retention} in {settings}")
    return 0


def main(argv: Sequence[str] | None = None) -> int:
    argv = normalize_argv(sys.argv[1:] if argv is None else argv)
    parser = build_parser()
    args = parser.parse_args(argv)
    command = args.command or "status"
    if command == "status":
        return cmd_status(args)
    if command == "configure":
        return cmd_configure(args)
    if command == "version":
        print(VERSION)
        return 0
    return cmd_clean(args)


if __name__ == "__main__":
    raise SystemExit(main())
