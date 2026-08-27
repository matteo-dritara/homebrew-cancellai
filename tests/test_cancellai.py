import contextlib
import importlib.util
import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).resolve().parent.parent / "cancellai.py"
spec = importlib.util.spec_from_file_location("cleaner", SCRIPT)
cleaner = importlib.util.module_from_spec(spec)
assert spec and spec.loader
sys.modules["cleaner"] = cleaner
spec.loader.exec_module(cleaner)

DAY = 86400


def write_file(path: Path, content="x", age_days=None):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if age_days is not None:
        ts = time.time() - age_days * DAY
        os.utime(path, (ts, ts))
    return path


def age_tree(path: Path, age_days: float):
    ts = time.time() - age_days * DAY
    if path.is_dir():
        for p in sorted(path.rglob("*"), reverse=True):
            with contextlib.suppress(OSError):
                os.utime(p, (ts, ts), follow_symlinks=False)
    os.utime(path, (ts, ts), follow_symlinks=False)


def codex_rollout(root: Path, sid: str, day: str, age_days: float, parent=None):
    year, month, dom = day.split("-")
    p = root / "sessions" / year / month / dom / f"rollout-{day}T10-00-00-{sid}.jsonl"
    meta = {"id": sid, "parent_thread_id": parent}
    write_file(p, json.dumps({"type": "session_meta", "payload": {"meta": meta}}) + "\n", age_days)
    return p


def claude_session(root: Path, project: str, sid: str, age_days: float, with_payload=True):
    p = root / "projects" / project / f"{sid}.jsonl"
    write_file(p, '{"type":"user"}\n', age_days)
    if with_payload:
        payload = root / "projects" / project / sid
        write_file(payload / "tool-results" / "large.txt", "z" * 100, age_days)
        age_tree(payload, age_days)
    return p


class CleanerTests(unittest.TestCase):
    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.codex = self.base / "codex-home"
        self.claude = self.base / "claude-home"
        self.codex.mkdir()
        self.claude.mkdir()

    def tearDown(self):
        self.td.cleanup()

    def test_codex_filesystem_cleanup_preserves_protected_and_recent(self):
        old_id = "11111111-1111-4111-8111-111111111111"
        new_id = "22222222-2222-4222-8222-222222222222"
        old = codex_rollout(self.codex, old_id, "2026-01-01", 30)
        recent = codex_rollout(self.codex, new_id, "2026-01-02", 1)
        write_file(self.codex / "auth.json", "SECRET")
        write_file(self.codex / "config.toml", 'model="x"')
        write_file(self.codex / "state_5.sqlite", "DB")

        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        paths = {a.path for a in plan.actions}
        self.assertIn(old, paths)
        self.assertNotIn(recent, paths)

        result = cleaner.execute_plan(
            plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=True, trim_history=True, verbose=False
        )
        self.assertEqual(result.failed, 0)
        self.assertFalse(old.exists())
        self.assertTrue(recent.exists())
        self.assertTrue((self.codex / "auth.json").exists())
        self.assertTrue((self.codex / "config.toml").exists())
        self.assertTrue((self.codex / "state_5.sqlite").exists())

    def test_claude_cleanup_preserves_memory_settings_plugins_and_trims_history(self):
        old_id = "33333333-3333-4333-8333-333333333333"
        new_id = "44444444-4444-4444-8444-444444444444"
        old = claude_session(self.claude, "-tmp-project", old_id, 30)
        recent = claude_session(self.claude, "-tmp-project", new_id, 1)
        memory = write_file(self.claude / "projects" / "-tmp-project" / "memory" / "MEMORY.md", "remember", 100)
        settings = write_file(self.claude / "settings.json", '{"model":"opus"}')
        plugin = write_file(self.claude / "plugins" / "x" / "plugin.txt", "keep")
        old_hist = {"display": "old", "timestamp": 1, "project": "/tmp/project", "sessionId": old_id}
        new_hist = {"display": "new", "timestamp": 2, "project": "/tmp/project", "sessionId": new_id}
        hist = self.claude / "history.jsonl"
        hist.write_text(json.dumps(old_hist) + "\n" + "malformed line\n" + json.dumps(new_hist) + "\n", encoding="utf-8")

        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"claude"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        self.assertIn(old_id, plan.claude_history_session_ids)
        self.assertEqual(plan.claude_history_lines, 1)

        result = cleaner.execute_plan(
            plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=True, trim_history=True, verbose=False
        )
        self.assertEqual(result.failed, 0)
        self.assertFalse(old.exists())
        self.assertFalse((old.parent / old_id).exists())
        self.assertTrue(recent.exists())
        self.assertTrue(memory.exists())
        self.assertTrue(settings.exists())
        self.assertTrue(plugin.exists())
        text = hist.read_text(encoding="utf-8")
        self.assertNotIn(old_id, text)
        self.assertIn(new_id, text)
        self.assertIn("malformed line", text)

    def test_keep_latest_protects_newest_even_if_all_are_old(self):
        ids = [
            "55555555-5555-4555-8555-555555555555",
            "66666666-6666-4666-8666-666666666666",
            "77777777-7777-4777-8777-777777777777",
        ]
        files = [
            codex_rollout(self.codex, ids[0], "2026-01-01", 40),
            codex_rollout(self.codex, ids[1], "2026-01-02", 30),
            codex_rollout(self.codex, ids[2], "2026-01-03", 20),
        ]
        plan = cleaner.build_plan(
            days=7, keep_latest=2, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        session_actions = [a for a in plan.actions if a.category == "session"]
        self.assertEqual(len(session_actions), 1)
        self.assertEqual(session_actions[0].path, files[0])

    def test_codex_cli_groups_subagents_under_root_and_sums_size(self):
        root_id = "10101010-1010-4010-8010-101010101010"
        child_id = "20202020-2020-4020-8020-202020202020"
        root = codex_rollout(self.codex, root_id, "2026-01-01", 30)
        child = codex_rollout(self.codex, child_id, "2026-01-01", 29, parent=root_id)
        with mock.patch.object(cleaner, "codex_delete_supported", return_value=(True, "/fake/codex")):
            plan = cleaner.build_plan(
                days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="auto", aggressive=False
            )
        session_actions = [a for a in plan.actions if a.category == "session"]
        self.assertEqual(len(session_actions), 1)
        self.assertEqual(session_actions[0].session_id, root_id)
        self.assertEqual(session_actions[0].size, root.stat().st_size + child.stat().st_size)

    def test_codex_recent_subagent_protects_old_root_tree(self):
        root_id = "30303030-3030-4030-8030-303030303030"
        child_id = "40404040-4040-4040-8040-404040404040"
        codex_rollout(self.codex, root_id, "2026-01-01", 30)
        codex_rollout(self.codex, child_id, "2026-01-02", 1, parent=root_id)
        with mock.patch.object(cleaner, "codex_delete_supported", return_value=(True, "/fake/codex")):
            plan = cleaner.build_plan(
                days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="auto", aggressive=False
            )
        self.assertEqual([a for a in plan.actions if a.category == "session"], [])

    def test_codex_keep_latest_counts_root_trees_not_subagents(self):
        old_root = "50505050-5050-4050-8050-505050505050"
        old_child = "60606060-6060-4060-8060-606060606060"
        new_root = "70707070-7070-4070-8070-707070707070"
        new_child = "80808080-8080-4080-8080-808080808080"
        codex_rollout(self.codex, old_root, "2026-01-01", 40)
        codex_rollout(self.codex, old_child, "2026-01-01", 39, parent=old_root)
        codex_rollout(self.codex, new_root, "2026-01-02", 30)
        codex_rollout(self.codex, new_child, "2026-01-02", 29, parent=new_root)
        with mock.patch.object(cleaner, "codex_delete_supported", return_value=(True, "/fake/codex")):
            plan = cleaner.build_plan(
                days=7, keep_latest=1, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="auto", aggressive=False
            )
        session_actions = [a for a in plan.actions if a.category == "session"]
        self.assertEqual(len(session_actions), 1)
        self.assertEqual(session_actions[0].session_id, old_root)

    def test_codex_filesystem_fallback_removes_each_subagent_rollout(self):
        root_id = "90909090-9090-4090-8090-909090909090"
        child_id = "a0a0a0a0-a0a0-40a0-80a0-a0a0a0a0a0a0"
        root = codex_rollout(self.codex, root_id, "2026-01-01", 30)
        child = codex_rollout(self.codex, child_id, "2026-01-01", 29, parent=root_id)
        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        paths = {a.path for a in plan.actions if a.category == "session"}
        self.assertEqual(paths, {root, child})

    def test_dry_run_changes_nothing(self):
        sid = "88888888-8888-4888-8888-888888888888"
        old = claude_session(self.claude, "-project", sid, 30)
        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"claude"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        result = cleaner.execute_plan(
            plan, codex_home=self.codex, claude_home=self.claude, dry_run=True, allow_running=True, trim_history=True, verbose=False
        )
        self.assertTrue(old.exists())
        self.assertEqual(result.attempted, 0)
        self.assertEqual(result.skipped, len(plan.actions))

    def test_aggressive_still_does_not_touch_claude_memory(self):
        memory = write_file(self.claude / "projects" / "-p" / "memory" / "MEMORY.md", "keep", 100)
        legacy = write_file(self.claude / "logs" / "old.log", "remove", 100)
        cache = write_file(self.claude / "cache" / "changelog.md", "cache", 1)
        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"claude"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=True
        )
        cleaner.execute_plan(
            plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=True, trim_history=True, verbose=False
        )
        self.assertTrue(memory.exists())
        self.assertFalse(legacy.exists())
        self.assertFalse(cache.exists())

    def test_claude_retention_configuration_preserves_existing_keys(self):
        settings = self.claude / "settings.json"
        settings.write_text(json.dumps({"model": "opus", "permissions": {"allow": ["Bash(ls)"]}}), encoding="utf-8")
        cleaner.configure_claude_retention(self.claude, 3)
        data = json.loads(settings.read_text(encoding="utf-8"))
        self.assertEqual(data["cleanupPeriodDays"], 3)
        self.assertEqual(data["model"], "opus")
        self.assertIn("permissions", data)

    def test_invalid_claude_settings_are_not_overwritten(self):
        settings = self.claude / "settings.json"
        settings.write_text("{bad json", encoding="utf-8")
        with self.assertRaises(ValueError):
            cleaner.configure_claude_retention(self.claude, 3)
        self.assertEqual(settings.read_text(encoding="utf-8"), "{bad json")

    def test_codex_protected_names_include_plugins_for_parity_with_claude(self):
        # Found via real-world dogfooding: ~/.codex/plugins is genuine installed
        # plugin state (plugins/cache, plugins/.plugin-appserver), not disposable
        # cache, mirroring Claude's own "plugins" protection.
        self.assertIn("plugins", cleaner.CODEX_PROTECTED_NAMES)

    def test_validate_root_rejects_home_and_root(self):
        with self.assertRaises(cleaner.SafetyError):
            cleaner.validate_config_root(Path("/"), "test")
        with mock.patch.object(cleaner.Path, "home", return_value=self.base), self.assertRaises(cleaner.SafetyError):
            cleaner.validate_config_root(self.base, "test")

    def test_codex_cli_backend_invokes_force_delete(self):
        sid = "99999999-9999-4999-8999-999999999999"
        old = codex_rollout(self.codex, sid, "2026-01-01", 30)
        plan = cleaner.Plan(cutoff=time.time() - 7 * DAY, days=7, keep_latest=0)
        action = cleaner.Action("codex", "session", old, old.stat().st_size, old.stat().st_mtime, sid, "codex-cli")
        plan.actions = [action]

        calls = []

        def fake_delete(action, codex_bin):
            calls.append((action.session_id, codex_bin))
            action.path.unlink()
            return True, "deleted"

        with (
            mock.patch.object(cleaner, "codex_delete_supported", return_value=(True, "/fake/codex")),
            mock.patch.object(cleaner, "delete_codex_via_cli", side_effect=fake_delete),
            mock.patch.object(cleaner, "active_processes", return_value={"codex": [], "claude": []}),
        ):
            result = cleaner.execute_plan(
                plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=False, trim_history=True, verbose=False
            )
        self.assertEqual(result.failed, 0)
        self.assertEqual(calls, [(sid, "/fake/codex")])
        self.assertFalse(old.exists())

    def test_recent_claude_session_protects_old_session_scoped_aux_data(self):
        sid = "abababab-abab-4bab-8bab-abababababab"
        claude_session(self.claude, "-project", sid, 1)
        aux = self.claude / "file-history" / sid
        write_file(aux / "snapshot.txt", "checkpoint", 30)
        age_tree(aux, 30)
        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"claude"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        self.assertNotIn(aux, {a.path for a in plan.actions})

    def test_running_codex_process_blocks_destructive_cleanup_by_default(self):
        sid = "cdcdcdcd-cdcd-4dcd-8dcd-cdcdcdcdcdcd"
        old = codex_rollout(self.codex, sid, "2026-01-01", 30)
        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        with mock.patch.object(cleaner, "active_processes", return_value={"codex": [4321], "claude": []}):
            result = cleaner.execute_plan(
                plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=False, trim_history=True, verbose=False
            )
        self.assertTrue(old.exists())
        self.assertGreaterEqual(result.skipped, 1)
        self.assertTrue(any("appears to be running" in e for e in result.errors))

    def test_codex_auto_never_silently_falls_back_to_raw_file_deletion(self):
        sid = "efefefef-efef-4fef-8fef-efefefefefef"
        old = codex_rollout(self.codex, sid, "2026-01-01", 30)
        with mock.patch.object(cleaner, "codex_delete_supported", return_value=(False, None)):
            plan = cleaner.build_plan(
                days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="auto", aggressive=False
            )
        self.assertNotIn(old, {a.path for a in plan.actions})
        self.assertTrue(any("does not expose" in note for note in plan.notes))

    def test_symlink_outside_root_is_never_followed(self):
        outside = self.base / "outside.txt"
        outside.write_text("do not delete", encoding="utf-8")
        link = self.codex / "tmp" / "old-link"
        link.parent.mkdir(parents=True)
        link.symlink_to(outside)
        ts = time.time() - 30 * DAY
        os.utime(link, (ts, ts), follow_symlinks=False)
        plan = cleaner.build_plan(
            days=7, keep_latest=0, tools={"codex"}, codex_home=self.codex, claude_home=self.claude, codex_backend="filesystem", aggressive=False
        )
        cleaner.execute_plan(
            plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=True, trim_history=True, verbose=False
        )
        self.assertTrue(outside.exists())
        self.assertFalse(link.exists())


if __name__ == "__main__":
    unittest.main(verbosity=2)
