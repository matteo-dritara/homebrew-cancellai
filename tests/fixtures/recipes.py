"""Deterministic generators for the synthetic provider-layout fixture corpus (E01-S02).

Fixtures are built from small recipes rather than committed as filesystem data, per
docs/development/VERIFICATION_STRATEGY.md ("Synthetic fixture generators are preferred")
and this directory's README. Every recipe writes only synthetic content: no real session
transcripts, prompts, source code, credentials, or captured home paths.

cancellai.py is loaded by file location, mirroring tests/test_cancellai.py, so this module
works the same way whether it is imported by pytest or by scripts/check_fixtures.py.
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import time
from collections.abc import Callable
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
_SPEC = importlib.util.spec_from_file_location("cancellai_reference", ROOT / "cancellai.py")
if _SPEC is None or _SPEC.loader is None:
    raise ImportError(f"cannot load cancellai.py from {ROOT}")
cancellai = importlib.util.module_from_spec(_SPEC)
# dataclasses resolves annotations via sys.modules[cls.__module__]; the module must be
# registered there before exec_module runs, or cancellai.py's frozen dataclasses fail
# to load. Mirrors tests/test_cancellai.py's identical workaround.
sys.modules[_SPEC.name] = cancellai
_SPEC.loader.exec_module(cancellai)

DAY = 86400


def _write(path: Path, content: str = "x", age_days: float | None = None) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if age_days is not None:
        ts = time.time() - age_days * DAY
        os.utime(path, (ts, ts))
    return path


def _age(path: Path, age_days: float) -> None:
    ts = time.time() - age_days * DAY
    os.utime(path, (ts, ts), follow_symlinks=False)


def _codex_markers(root: Path) -> None:
    _write(root / "auth.json", "{}")
    _write(root / "config.toml", 'model = "synthetic"\n')


def _claude_markers(root: Path) -> None:
    _write(root / "settings.json", "{}")
    _write(root / "keybindings.json", "{}")


def _codex_rollout(root: Path, session_id: str, day: str, age_days: float, parent: str | None = None) -> Path:
    year, month, dom = day.split("-")
    path = root / "sessions" / year / month / dom / f"rollout-{day}T09-00-00-{session_id}.jsonl"
    meta = {"type": "session_meta", "payload": {"meta": {"id": session_id, "parent_thread_id": parent}}}
    return _write(path, json.dumps(meta) + "\n", age_days)


def _claude_session(root: Path, project: str, session_id: str, age_days: float) -> Path:
    path = root / "projects" / project / f"{session_id}.jsonl"
    return _write(path, json.dumps({"type": "user"}) + "\n", age_days)


def _claude_session_with_payload(root: Path, project: str, session_id: str, age_days: float) -> tuple[Path, Path]:
    """A session plus its companion payload directory (named exactly after the session id).

    discover_claude_sessions only recurses into a subdirectory that is a session's own
    companion payload dir - an unrelated subdirectory is never walked at all. This is the
    real mechanism a partial-tree fixture must use to produce a genuinely incomplete scan.
    """
    session_path = _claude_session(root, project, session_id, age_days)
    payload_dir = root / "projects" / project / session_id
    _write(payload_dir / "tool-results" / "large.txt", "synthetic payload", age_days)
    _age(payload_dir, age_days)
    return session_path, payload_dir


def build_claude_normal_session(root: Path) -> None:
    """A single, well-formed session inside the retention window - nothing eligible."""
    _claude_markers(root)
    _claude_session(root, "synthetic-project-a", "11111111-1111-4111-8111-111111111111", age_days=2)


def build_codex_normal_session(root: Path) -> None:
    """A single, well-formed rollout inside the retention window."""
    _codex_markers(root)
    _codex_rollout(root, "22222222-2222-4222-8222-222222222222", "2026-08-20", age_days=8)


def build_codex_subagent_tree(root: Path) -> None:
    """A root rollout with two subagent children, all old enough to be selected together.

    Exercises choose_codex_old_sessions: --keep-latest must count the root tree, not
    individual rollout files, and every child must be reachable from the root id.
    """
    _codex_markers(root)
    root_id = "33333333-3333-4333-8333-333333333333"
    _codex_rollout(root, root_id, "2026-05-01", age_days=120)
    _codex_rollout(root, "33333333-3333-4333-8333-333333333334", "2026-05-01", age_days=120, parent=root_id)
    _codex_rollout(root, "33333333-3333-4333-8333-333333333335", "2026-05-01", age_days=120, parent=root_id)


def build_claude_active_data(root: Path) -> None:
    """A session touched moments ago, standing in for a provider that is mid-write.

    Filesystem freshness alone cannot prove a process is running; discover_* layers this
    with ProcessObservation. This fixture supplies the freshness half of that signal.
    """
    _claude_markers(root)
    _claude_session(root, "synthetic-project-b", "44444444-4444-4444-8444-444444444444", age_days=0)


def _write_protected_entry(root: Path, name: str) -> None:
    """Create one protected name as whichever shape it really is: a file or a directory.

    A handful of protected names (settings.json, auth.json, ...) are files; the rest are
    directories. Getting this wrong is exactly the kind of gap the E00-S01 barrier exists
    to close, so the fixture must match the real shape rather than assume one.
    """
    path = root / name
    if path.suffix in {".json", ".toml"}:
        _write(path, "{}" if path.suffix == ".json" else 'key = "synthetic"\n', age_days=400)
    else:
        _write(path / ".synthetic-keep", "synthetic", age_days=400)
        _age(path, 400)


def build_claude_protected_state(root: Path) -> None:
    """Every CLAUDE_PROTECTED_NAMES entry, aged far past any cutoff, still off-limits."""
    _claude_markers(root)
    for name in sorted(cancellai.CLAUDE_PROTECTED_NAMES):
        path = root / name
        if path.exists():
            _age(path, 400)
            continue
        _write_protected_entry(root, name)


def build_codex_protected_state(root: Path) -> None:
    """Every CODEX_PROTECTED_NAMES entry, aged far past any cutoff, still off-limits."""
    _codex_markers(root)
    for name in sorted(cancellai.CODEX_PROTECTED_NAMES):
        path = root / name
        if path.exists():
            _age(path, 400)
            continue
        _write_protected_entry(root, name)


def build_claude_partial_tree(root: Path) -> None:
    """Two ordinary sessions, plus a third whose companion payload dir cannot be listed.

    Locking a *directory* is what actually produces an unreadable scope: lstat on a single
    0o000 file still succeeds (stat needs no read permission on its target), but os.walk
    cannot list a 0o000 directory's contents. discover_claude_sessions only ever recurses
    into a subdirectory that is a session's own companion payload dir (root/projects/<p>/<sid>/)
    - an unrelated subdirectory is never walked at all - so the locked directory must be a
    real companion, not merely a sibling, to reproduce a genuinely incomplete scan on the
    path build_plan actually takes.

    The caller must restore permissions (chmod 0o755) on every path under `root` before
    removing the tree - chmod(0o000) only denies a non-root reader, matching the existing
    convention in tests/test_cancellai.py, but is still real enough to break naive cleanup.
    """
    _claude_markers(root)
    project = "synthetic-project-c"
    _claude_session(root, project, "55555555-5555-4555-8555-555555555551", age_days=40)
    _claude_session(root, project, "55555555-5555-4555-8555-555555555552", age_days=40)
    _, locked_payload = _claude_session_with_payload(root, project, "55555555-5555-4555-8555-555555555553", age_days=40)
    locked_payload.chmod(0o000)


def build_codex_partial_tree(root: Path) -> None:
    """Two readable rollouts, plus a third inside a session directory that cannot be listed.

    The Codex mirror of build_claude_partial_tree, and the fixture the corpus never had:
    `docs/audits/2026-09-03-CODE_REVIEW.md` (CR-TE-01) reproduced the Rust engine *deleting*
    the readable rollout here while cancellai.py withholds the whole tool. The gate could not
    see that, because no Codex partial-scan fixture existed to run through it (CR-TE-03).

    Locking a date directory under sessions/ - not a single file - is what actually produces
    an unreadable scope: lstat on a 0o000 file still succeeds, but os.walk cannot list a 0o000
    directory, so iter_files' onerror hook fires and Scan records the path. That is the exact
    branch discover_codex_sessions takes, so the incompleteness is real rather than staged.

    The caller must restore permissions (chmod 0o755) under `root` before removing the tree,
    per build_claude_partial_tree's identical note.
    """
    _codex_markers(root)
    _codex_rollout(root, "88888888-8888-4888-8888-888888888881", "2026-05-01", age_days=120)
    _codex_rollout(root, "88888888-8888-4888-8888-888888888882", "2026-05-01", age_days=120)
    _codex_rollout(root, "88888888-8888-4888-8888-888888888883", "2026-05-02", age_days=120)
    locked_day = root / "sessions" / "2026" / "05" / "02"
    _age(locked_day, 120)
    locked_day.chmod(0o000)


def build_claude_partial_project(root: Path) -> None:
    """Two readable sessions in one project, plus a second project directory that cannot be listed.

    Distinct from build_claude_partial_tree, which locks a session's *companion payload*
    directory. E06-S02 repaired only that branch; discover_claude_sessions reaches a project
    directory through a separate `project_dir.iterdir()` call, and an unreadable one there was
    silently skipped by the Rust adapter with nothing recorded - a case no document disclosed
    before CR-TE-01. Both branches must be in the corpus, or repairing one leaves the other
    provably untested.

    The caller must restore permissions (chmod 0o755) under `root` before removing the tree.
    """
    _claude_markers(root)
    readable = "synthetic-project-d"
    _claude_session(root, readable, "99999999-9999-4999-8999-999999999991", age_days=40)
    _claude_session(root, readable, "99999999-9999-4999-8999-999999999992", age_days=40)
    locked_project = root / "projects" / "synthetic-project-e"
    _claude_session(root, "synthetic-project-e", "99999999-9999-4999-8999-999999999993", age_days=40)
    _age(locked_project, 40)
    locked_project.chmod(0o000)


def build_codex_symlink_escape(root: Path) -> None:
    """A symlink inside sessions/ pointing outside the approved root.

    Must never be followed for deletion or size accounting (E00-S02 / ADR-0013).
    """
    _codex_markers(root)
    _codex_rollout(root, "66666666-6666-4666-8666-666666666666", "2026-01-01", age_days=200)
    outside = root.parent / "outside-codex-root" / "not-a-rollout.jsonl"
    _write(outside, "synthetic content outside the approved root", age_days=200)
    link = root / "sessions" / "escape.jsonl"
    link.parent.mkdir(parents=True, exist_ok=True)
    link.symlink_to(outside)


def build_claude_symlink_protected_name(root: Path) -> None:
    """A case-variant symlink of a protected name, pointing outside the root.

    Protection is checked lexically and after resolution, in canonical caseless NFD form
    (docs/architecture/AS_IS.md, safety-critical core item 5); a fixture that only varies
    case is the falsification case that rule exists for.
    """
    _claude_markers(root)
    outside = root.parent / "outside-claude-root" / "payload.txt"
    _write(outside, "synthetic content outside the approved root", age_days=200)
    variant_name = "Plugins"  # case variant of the protected "plugins"
    (root / variant_name).symlink_to(outside)


def build_codex_layout_drift(root: Path) -> None:
    """An unrecognized top-level entry alongside the known layout.

    No current coverage/discovery rule understands `plugin_cache_v2/`; it must be reported
    as unknown (E00-S08 coverage vocabulary), never treated as cleanable.
    """
    _codex_markers(root)
    _codex_rollout(root, "77777777-7777-4777-8777-777777777777", "2026-07-01", age_days=30)
    _write(root / "plugin_cache_v2" / "index.bin", "synthetic-unknown-layout", age_days=30)


FIXTURES: dict[str, Callable[[Path], None]] = {
    "claude-normal-session": build_claude_normal_session,
    "codex-normal-session": build_codex_normal_session,
    "codex-subagent-tree": build_codex_subagent_tree,
    "claude-active-data": build_claude_active_data,
    "claude-protected-state": build_claude_protected_state,
    "codex-protected-state": build_codex_protected_state,
    "claude-partial-tree": build_claude_partial_tree,
    "claude-partial-project": build_claude_partial_project,
    "codex-partial-tree": build_codex_partial_tree,
    "codex-symlink-escape": build_codex_symlink_escape,
    "claude-symlink-protected-name": build_claude_symlink_protected_name,
    "codex-layout-drift": build_codex_layout_drift,
}


def build(fixture_id: str, root: Path) -> None:
    """Materialize one fixture's provider-root tree at `root` (must already exist)."""
    try:
        recipe = FIXTURES[fixture_id]
    except KeyError as exc:
        raise KeyError(f"unknown fixture id: {fixture_id}") from exc
    recipe(root)
