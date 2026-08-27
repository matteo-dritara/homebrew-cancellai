import contextlib
import importlib.util
import io
import json
import os
import subprocess
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


def quiet_processes():
    return cleaner.ProcessObservation(pids={"codex": [], "claude": []}, complete=True)


def busy_processes(codex=(), claude=()):
    return cleaner.ProcessObservation(pids={"codex": list(codex), "claude": list(claude)}, complete=True)


def unknown_processes():
    """`ps` was unusable: no evidence either way."""
    return cleaner.ProcessObservation(pids={"codex": [], "claude": []}, complete=False)


def use_as_default_roots(test, codex: Path, claude: Path):
    """Make the temp roots stand in for the operator's real ~/.codex and ~/.claude.

    A non-default root needs --allow-custom-root (E00-S02). Tests that are not about the
    root boundary should exercise the ordinary default-root path, so they patch the
    default rather than silently acknowledging a custom one.
    """
    roots = {"codex": codex, "claude": claude}
    patcher = mock.patch.object(cleaner, "default_home", side_effect=lambda tool: roots[tool])
    patcher.start()
    test.addCleanup(patcher.stop)


def make_provider_roots(base: Path):
    """Create temp roots that a real provider fingerprint would accept.

    build_plan refuses destructive work on a directory that does not structurally look like
    the provider (E00-S02), so fixtures must carry the identifying markers a real
    ~/.codex and ~/.claude have.
    """
    codex = base / "codex-home"
    claude = base / "claude-home"
    codex.mkdir()
    claude.mkdir()
    write_file(codex / "auth.json", "{}")
    write_file(codex / "config.toml", 'model="x"')
    write_file(claude / "settings.json", "{}")
    write_file(claude / "keybindings.json", "{}")
    return codex, claude


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
        self.codex, self.claude = make_provider_roots(self.base)
        use_as_default_roots(self, self.codex, self.claude)

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

        with mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()):
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
        age_tree(self.claude / "logs", 100)
        cache = write_file(self.claude / "cache" / "changelog.md", "cache", 30)
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
            mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()),
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
        with mock.patch.object(cleaner, "active_processes", return_value=busy_processes(codex=[4321])):
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


class TrustFloorTests(unittest.TestCase):
    """E00 regression coverage: the P0 defects must not be able to come back."""

    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.codex, self.claude = make_provider_roots(self.base)
        use_as_default_roots(self, self.codex, self.claude)
        self.quiet = mock.patch.object(cleaner, "active_processes", return_value=quiet_processes())

    def tearDown(self):
        self.td.cleanup()

    # --- E00-S01 protected-name barrier --------------------------------------
    def test_protected_action_injected_into_execution_is_refused(self):
        secret = write_file(self.codex / "auth.json", "SECRET", 100)
        plan = cleaner.Plan(cutoff=time.time() - 7 * DAY, days=7, keep_latest=0)
        plan.actions.append(cleaner.Action("codex", "old-temp", secret, 6, secret.stat().st_mtime))
        with self.quiet:
            result = cleaner.execute_plan(
                plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=True, trim_history=False, verbose=False
            )
        self.assertTrue(secret.exists())
        self.assertEqual(result.succeeded, 0)
        self.assertEqual(result.failed, 1)
        self.assertTrue(any("protected" in err for err in result.errors))

    def test_protected_barrier_covers_nested_paths_for_both_tools(self):
        nested_codex = write_file(self.codex / "plugins" / "marketplace" / "blob.bin", "keep", 100)
        nested_claude = write_file(self.claude / "agent-memory" / "deep" / "note.md", "keep", 100)
        with self.assertRaises(cleaner.SafetyError):
            cleaner.safe_remove(nested_codex, self.codex, cleaner.CODEX_PROTECTED_NAMES)
        with self.assertRaises(cleaner.SafetyError):
            cleaner.safe_remove(nested_claude, self.claude, cleaner.CLAUDE_PROTECTED_NAMES)
        self.assertTrue(nested_codex.exists())
        self.assertTrue(nested_claude.exists())

    def test_plan_drops_protected_candidates_emitted_by_a_scanner(self):
        plugins = write_file(self.codex / "plugins" / "cache" / "blob.bin", "keep", 100)
        age_tree(self.codex / "plugins", 100)
        rogue = cleaner.Action("codex", "old-temp", self.codex / "plugins", 4, time.time() - 100 * DAY)

        def rogue_aux(codex_home, cutoff, scan=None):
            return [rogue]

        with mock.patch.object(cleaner, "discover_codex_aux", side_effect=rogue_aux):
            plan = cleaner.build_plan(
                days=7,
                keep_latest=0,
                tools={"codex"},
                codex_home=self.codex,
                claude_home=self.claude,
                codex_backend="filesystem",
                aggressive=False,
            )
        self.assertEqual(plan.actions, [])
        self.assertTrue(any("protected names" in note for note in plan.notes))
        self.assertTrue(plugins.exists())

    def test_aggressive_mode_cannot_reach_protected_names(self):
        keep = write_file(self.claude / "agent-memory" / "MEMORY.md", "keep", 100)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"claude"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=True,
        )
        self.assertNotIn(keep, {a.path for a in plan.actions})
        self.assertTrue(keep.exists())

    # --- E00-S03 aggressive expands categories, not retention ----------------
    def test_aggressive_respects_cutoff_for_legacy_and_cache(self):
        fresh_legacy = write_file(self.claude / "todos" / "today.json", "fresh", 1)
        fresh_cache = write_file(self.claude / "cache" / "changelog.md", "fresh", 1)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"claude"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=True,
        )
        selected = {a.path for a in plan.actions}
        self.assertNotIn(self.claude / "todos", selected)
        self.assertNotIn(fresh_cache, selected)
        self.assertTrue(fresh_legacy.exists())

    def test_aggressive_cutoff_boundary(self):
        cache = self.claude / "remote-settings.json"
        for offset, expected in ((-1, True), (1, False)):
            write_file(cache, "{}")
            ts = time.time() - 7 * DAY + offset
            os.utime(cache, (ts, ts))
            plan = cleaner.build_plan(
                days=7,
                keep_latest=0,
                tools={"claude"},
                codex_home=self.codex,
                claude_home=self.claude,
                codex_backend="filesystem",
                aggressive=True,
            )
            self.assertEqual(cache in {a.path for a in plan.actions}, expected, f"offset={offset}")

    # --- E00-S04 destructive intent must be typed ----------------------------
    def test_flags_without_a_subcommand_never_normalize_to_clean(self):
        for argv in ([], ["--days", "14"], ["--aggressive"], ["--json"], ["-y"], ["--tool", "claude"]):
            self.assertEqual(cleaner.normalize_argv(argv)[0], "status", argv)
        self.assertEqual(cleaner.normalize_argv(["clean", "--days", "3"])[0], "clean")
        self.assertEqual(cleaner.normalize_argv(["--version"]), ["--version"])
        self.assertEqual(cleaner.normalize_argv(["clen"]), ["clen"])

    def test_unknown_verb_is_a_usage_error_not_a_cleanup(self):
        with mock.patch.object(sys, "stderr", new=io.StringIO()), self.assertRaises(SystemExit) as ctx:
            cleaner.main(["clen"])
        self.assertEqual(ctx.exception.code, cleaner.EXIT_USAGE)

    def test_exit_code_distinguishes_blocked_from_success_and_usage(self):
        sid = "12121212-1212-4212-8212-121212121212"
        codex_rollout(self.codex, sid, "2026-01-01", 30)
        env = {"CODEX_HOME": str(self.codex), "CLAUDE_CONFIG_DIR": str(self.claude)}
        with mock.patch.dict(os.environ, env), mock.patch.object(sys, "stdout", new=io.StringIO()):
            with mock.patch.object(cleaner, "active_processes", return_value=busy_processes(codex=[999])):
                blocked = cleaner.main(["clean", "-y", "--keep-latest", "0", "--tool", "codex", "--codex-backend", "filesystem"])
            with self.quiet:
                ok = cleaner.main(["clean", "-y", "--keep-latest", "0", "--tool", "codex", "--codex-backend", "filesystem"])
        self.assertEqual(blocked, cleaner.EXIT_BLOCKED)
        self.assertEqual(ok, cleaner.EXIT_OK)

    def test_blocked_json_run_reports_its_exit_code(self):
        sid = "13131313-1313-4313-8313-131313131313"
        codex_rollout(self.codex, sid, "2026-01-01", 30)
        env = {"CODEX_HOME": str(self.codex), "CLAUDE_CONFIG_DIR": str(self.claude)}
        buffer = io.StringIO()
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(sys, "stdout", new=buffer),
            mock.patch.object(cleaner, "active_processes", return_value=busy_processes(codex=[999])),
        ):
            code = cleaner.main(["clean", "-y", "--json", "--keep-latest", "0", "--tool", "codex", "--codex-backend", "filesystem"])
        payload = json.loads(buffer.getvalue())
        self.assertEqual(code, cleaner.EXIT_BLOCKED)
        self.assertEqual(payload["exit_code"], cleaner.EXIT_BLOCKED)
        self.assertEqual(payload["result"]["blocked_tools"], ["codex"])

    # --- E00-S06 shared provider metadata under concurrency ------------------
    def test_history_is_not_rewritten_while_claude_is_running(self):
        sid = "14141414-1414-4414-8414-141414141414"
        claude_session(self.claude, "-project", sid, 30)
        hist = self.claude / "history.jsonl"
        hist.write_text(json.dumps({"display": "old", "sessionId": sid}) + "\n", encoding="utf-8")
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"claude"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        with mock.patch.object(cleaner, "active_processes", return_value=busy_processes(claude=[4242])):
            result = cleaner.execute_plan(
                plan, codex_home=self.codex, claude_home=self.claude, dry_run=False, allow_running=True, trim_history=True, verbose=False
            )
        self.assertIn(sid, hist.read_text(encoding="utf-8"))
        self.assertTrue(result.partial)
        self.assertTrue(any("history trimming was skipped" in err for err in result.errors))

    def test_history_trim_abandons_rewrite_on_concurrent_write(self):
        sid = "15151515-1515-4515-8515-151515151515"
        hist = self.claude / "history.jsonl"
        original = json.dumps({"display": "old", "sessionId": sid}) + "\n"
        hist.write_text(original, encoding="utf-8")
        real_fsync = os.fsync

        def racing_fsync(fd):
            # Claude appends a new prompt after the copy loop but before the replace.
            with hist.open("a", encoding="utf-8") as fh:
                fh.write(json.dumps({"display": "concurrent", "sessionId": "other"}) + "\n")
            return real_fsync(fd)

        with mock.patch.object(cleaner.os, "fsync", racing_fsync):
            removed, _bytes, status = cleaner.trim_claude_history(hist, {sid})
        self.assertEqual(status, "concurrent-modification")
        self.assertEqual(removed, 0)
        text = hist.read_text(encoding="utf-8")
        self.assertIn(sid, text)
        self.assertIn("concurrent", text)
        self.assertEqual(list(hist.parent.glob(".history.*.tmp")), [])

    def test_history_trim_removes_only_deleted_sessions(self):
        keep = "16161616-1616-4616-8616-161616161616"
        drop = "17171717-1717-4717-8717-171717171717"
        hist = self.claude / "history.jsonl"
        hist.write_text(
            json.dumps({"sessionId": drop}) + "\n" + "broken\n" + json.dumps({"sessionId": keep}) + "\n",
            encoding="utf-8",
        )
        removed, _bytes, status = cleaner.trim_claude_history(hist, {drop})
        self.assertEqual((removed, status), (1, "trimmed"))
        text = hist.read_text(encoding="utf-8")
        self.assertNotIn(drop, text)
        self.assertIn(keep, text)
        self.assertIn("broken", text)

    # --- E00-S08 coverage reporting ------------------------------------------
    def test_unknown_provider_entries_are_reported_and_never_cleaned(self):
        unknown = write_file(self.codex / "computer-use" / "frame.png", "img", 100)
        age_tree(self.codex / "computer-use", 100)
        entries = cleaner.root_entry_sizes(self.codex)
        payload = cleaner.coverage_payload(entries, "codex")
        self.assertIn("computer-use", payload["unknown"]["names"])
        self.assertIn("unknown", {b.state for b in cleaner.coverage_report(entries, "codex")})

        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=True,
        )
        self.assertEqual(plan.actions, [])
        self.assertTrue(unknown.exists())

    def test_coverage_classifies_protected_cleanable_and_reported_state(self):
        write_file(self.codex / "auth.json", "SECRET")
        write_file(self.codex / "state_5.sqlite", "DB")
        write_file(self.codex / "sessions" / "keep.jsonl", "{}")
        write_file(self.codex / "dictation-history" / "a.wav", "snd")
        states = {p.name: cleaner.coverage_state(p.name, "codex") for p, _s in cleaner.root_entry_sizes(self.codex)}
        self.assertEqual(states["auth.json"], "protected")
        self.assertEqual(states["state_5.sqlite"], "reported")
        self.assertEqual(states["sessions"], "cleanable")
        self.assertEqual(states["dictation-history"], "unknown")
        self.assertEqual(cleaner.coverage_state("plugins", "claude"), "protected")
        self.assertEqual(cleaner.coverage_state("file-history", "claude"), "cleanable")
        self.assertEqual(cleaner.coverage_state("chrome", "claude"), "unknown")

    def test_status_coverage_output_lists_unknown_entries(self):
        write_file(self.claude / "daemon" / "state.json", "{}")
        env = {"CODEX_HOME": str(self.codex), "CLAUDE_CONFIG_DIR": str(self.claude)}
        buffer = io.StringIO()
        with mock.patch.dict(os.environ, env), mock.patch.object(sys, "stdout", new=buffer), self.quiet:
            code = cleaner.main(["status", "--coverage"])
        output = buffer.getvalue()
        self.assertEqual(code, cleaner.EXIT_OK)
        self.assertIn("claude coverage", output)
        self.assertIn("daemon", output)


class RootAuthorityTests(unittest.TestCase):
    """E00-S02: a directory must prove it is a provider root before it can be mutated."""

    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.codex, self.claude = make_provider_roots(self.base)

    def tearDown(self):
        self.td.cleanup()

    def test_default_roots_are_accepted_even_when_empty(self):
        home = self.base / "home"
        (home / ".codex").mkdir(parents=True)
        (home / ".claude").mkdir(parents=True)
        with mock.patch.object(cleaner.Path, "home", return_value=home):
            for tool in ("codex", "claude"):
                authority = cleaner.fingerprint_root(cleaner.default_home(tool), tool)
                self.assertEqual((authority.origin, authority.confidence), ("default", "default"), tool)
                self.assertTrue(authority.destructive_allowed(acknowledged=False), tool)

    def test_ordinary_project_directory_with_tmp_and_log_is_refused(self):
        rogue = self.base / "my-project"
        write_file(rogue / "tmp" / "scratch.txt", "work", 30)
        write_file(rogue / "log" / "build.log", "log", 30)
        write_file(rogue / "src" / "main.py", "print()", 30)
        age_tree(rogue / "tmp", 30)
        age_tree(rogue / "log", 30)

        authority = cleaner.fingerprint_root(rogue, "codex")
        self.assertEqual(authority.confidence, "unknown")
        self.assertFalse(authority.destructive_allowed(acknowledged=True))

        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=rogue,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=True,
        )
        self.assertEqual(plan.actions, [])
        self.assertEqual(plan.withheld, ["codex"])
        self.assertTrue((rogue / "tmp" / "scratch.txt").exists())
        self.assertTrue((rogue / "log" / "build.log").exists())

    def test_low_confidence_custom_root_is_refused(self):
        # A single supporting marker is not an identity.
        weak = self.base / "weak"
        write_file(weak / "config.toml", 'model="x"')
        authority = cleaner.fingerprint_root(weak, "codex")
        self.assertEqual(authority.confidence, "low")
        self.assertFalse(authority.destructive_allowed(acknowledged=True))

    def test_credible_custom_root_needs_explicit_intent(self):
        custom = self.base / "custom-codex"
        write_file(custom / "auth.json", "{}")
        write_file(custom / "config.toml", 'model="x"')
        codex_rollout(custom, "51515151-5151-4151-8151-515151515151", "2026-01-01", 30)
        authority = cleaner.fingerprint_root(custom, "codex")
        self.assertEqual((authority.origin, authority.confidence), ("custom", "high"))
        # Structure alone is not permission: the operator must also mean it.
        self.assertFalse(authority.destructive_allowed(acknowledged=False))
        self.assertTrue(authority.destructive_allowed(acknowledged=True))
        self.assertIn("--allow-custom-root", authority.explain(acknowledged=False))

    def test_marker_filenames_without_provider_content_do_not_identify_a_root(self):
        # Filenames are not identity: the same names with unrelated content prove nothing.
        decoy = self.base / "decoy"
        write_file(decoy / "auth.json", "app credential placeholder")
        write_file(decoy / "settings.json", "not json at all")
        write_file(decoy / "history.jsonl", "plain log line\n")
        self.assertEqual(cleaner.fingerprint_root(decoy, "codex").confidence, "unknown")
        self.assertEqual(cleaner.fingerprint_root(decoy, "claude").confidence, "unknown")

    def test_acknowledged_custom_root_can_still_be_cleaned(self):
        custom = self.base / "custom-codex"
        write_file(custom / "auth.json", "{}")
        write_file(custom / "config.toml", 'model="x"')
        old = codex_rollout(custom, "52525252-5252-4252-8252-525252525252", "2026-01-01", 30)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=custom,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
            allow_custom_roots=True,
        )
        self.assertIn(old, {a.path for a in plan.actions})
        with mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()):
            cleaner.execute_plan(
                plan,
                codex_home=custom,
                claude_home=self.claude,
                dry_run=False,
                allow_running=True,
                trim_history=False,
                verbose=False,
            )
        self.assertFalse(old.exists())

    def test_inspection_still_works_on_an_unverified_root(self):
        rogue = self.base / "my-project"
        write_file(rogue / "tmp" / "scratch.txt", "work", 30)
        age_tree(rogue / "tmp", 30)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=rogue,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
            for_mutation=False,
        )
        self.assertTrue(plan.actions)
        self.assertEqual(plan.withheld, [])
        with self.assertRaises(cleaner.SafetyError):
            cleaner.execute_plan(
                plan,
                codex_home=rogue,
                claude_home=self.claude,
                dry_run=False,
                allow_running=True,
                trim_history=False,
                verbose=False,
            )
        self.assertTrue((rogue / "tmp" / "scratch.txt").exists())

    def test_execution_refuses_an_unverified_root_even_if_the_plan_says_otherwise(self):
        rogue = self.base / "my-project"
        target = write_file(rogue / "tmp" / "scratch.txt", "work", 30)
        plan = cleaner.Plan(cutoff=time.time() - 7 * DAY, days=7, keep_latest=0)
        plan.actions.append(cleaner.Action("codex", "old-temp", target, 4, target.stat().st_mtime))
        with self.assertRaises(cleaner.SafetyError):
            cleaner.execute_plan(
                plan,
                codex_home=rogue,
                claude_home=self.claude,
                dry_run=False,
                allow_running=True,
                trim_history=False,
                verbose=False,
            )
        self.assertTrue(target.exists())

    def test_unverified_root_makes_clean_exit_blocked(self):
        rogue = self.base / "my-project"
        write_file(rogue / "tmp" / "scratch.txt", "work", 30)
        age_tree(rogue / "tmp", 30)
        env = {"CODEX_HOME": str(rogue), "CLAUDE_CONFIG_DIR": str(self.claude)}
        buffer = io.StringIO()
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(sys, "stdout", new=buffer),
            mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()),
        ):
            code = cleaner.main(["clean", "-y", "--json", "--keep-latest", "0", "--tool", "codex"])
        payload = json.loads(buffer.getvalue())
        self.assertEqual(code, cleaner.EXIT_BLOCKED)
        self.assertEqual(payload["withheld_tools"], ["codex"])
        self.assertFalse(payload["roots"]["codex"]["destructive_allowed"])


class ScanCompletenessTests(unittest.TestCase):
    """E00-S05: an incomplete observation must not become destructive permission."""

    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.codex, self.claude = make_provider_roots(self.base)
        use_as_default_roots(self, self.codex, self.claude)
        self.locked = []

    def tearDown(self):
        for path in self.locked:
            with contextlib.suppress(OSError):
                path.chmod(0o755)
        self.td.cleanup()

    def deny(self, path: Path):
        path.chmod(0o000)
        self.locked.append(path)

    def test_unreadable_directory_withholds_destructive_authority(self):
        sid = "21212121-2121-4121-8121-212121212121"
        old = codex_rollout(self.codex, sid, "2026-01-01", 30)
        blocked = self.codex / "tmp" / "locked"
        write_file(blocked / "inner.txt", "data", 30)
        age_tree(self.codex / "tmp", 30)
        self.deny(blocked)

        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        self.assertFalse(plan.scan_complete)
        self.assertEqual(plan.incomplete_scopes, ["codex"])
        self.assertEqual(plan.actions, [])
        self.assertEqual(plan.withheld, ["codex"])
        self.assertTrue(old.exists())

    def test_status_explains_unreadable_paths_without_crashing(self):
        blocked = self.claude / "file-history" / "locked"
        write_file(blocked / "inner.txt", "data", 30)
        age_tree(self.claude / "file-history", 30)
        self.deny(blocked)
        env = {"CODEX_HOME": str(self.codex), "CLAUDE_CONFIG_DIR": str(self.claude)}
        buffer = io.StringIO()
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(sys, "stdout", new=buffer),
            mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()),
        ):
            code = cleaner.main(["status", "--tool", "claude"])
        output = buffer.getvalue()
        self.assertEqual(code, cleaner.EXIT_OK)
        self.assertIn("SCAN INCOMPLETE", output)
        self.assertIn("locked", output)

    def test_execution_refuses_a_plan_built_from_an_incomplete_scan(self):
        plan = cleaner.Plan(cutoff=time.time() - 7 * DAY, days=7, keep_latest=0)
        incomplete = cleaner.Scan(scope="codex")
        incomplete.errors.append("/nowhere: Permission denied")
        plan.scans = [incomplete]
        with self.assertRaises(cleaner.SafetyError):
            cleaner.execute_plan(
                plan,
                codex_home=self.codex,
                claude_home=self.claude,
                dry_run=False,
                allow_running=True,
                trim_history=False,
                verbose=False,
            )

    def test_a_vanished_path_is_a_race_not_an_incomplete_scan(self):
        scan = cleaner.Scan(scope="codex")
        scan.record(Path("/nowhere/gone.txt"), FileNotFoundError(2, "No such file or directory"))
        self.assertTrue(scan.complete)
        scan.record(Path("/nowhere/locked"), PermissionError(13, "Permission denied"))
        self.assertFalse(scan.complete)

    def test_recorded_scan_errors_are_bounded(self):
        scan = cleaner.Scan(scope="codex")
        for index in range(cleaner.MAX_RECORDED_SCAN_ERRORS + 25):
            scan.record(Path(f"/nowhere/{index}"), PermissionError(13, "Permission denied"))
        self.assertEqual(len(scan.errors), cleaner.MAX_RECORDED_SCAN_ERRORS)
        self.assertTrue(scan.truncated)
        self.assertFalse(scan.complete)

    def test_complete_scan_still_cleans(self):
        sid = "22222222-2222-4222-8222-222222222223"
        old = codex_rollout(self.codex, sid, "2026-01-01", 30)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        self.assertTrue(plan.scan_complete)
        self.assertEqual(plan.withheld, [])
        with mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()):
            cleaner.execute_plan(
                plan,
                codex_home=self.codex,
                claude_home=self.claude,
                dry_run=False,
                allow_running=True,
                trim_history=False,
                verbose=False,
            )
        self.assertFalse(old.exists())


class IndependentVerifierAdversarialTests(unittest.TestCase):
    """Counterexamples added by the independent E00 verifier, not executor evidence."""

    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.codex, self.claude = make_provider_roots(self.base)

    def tearDown(self):
        self.td.cleanup()

    def test_protected_symlink_name_cannot_be_unlinked(self):
        target = self.base / "external-plugin-state"
        target.mkdir()
        protected_link = self.codex / "plugins"
        protected_link.symlink_to(target, target_is_directory=True)

        with self.assertRaises(cleaner.SafetyError):
            cleaner.safe_remove(protected_link, self.codex, cleaner.CODEX_PROTECTED_NAMES)
        self.assertTrue(protected_link.is_symlink())

    def test_generic_custom_root_can_falsely_earn_high_authority(self):
        project = self.base / "ordinary-project"
        write_file(project / "auth.json", "app credential placeholder")
        rollout = codex_rollout(project, "31313131-3131-4131-8131-313131313131", "2026-01-01", 30)

        authority = cleaner.fingerprint_root(project, "codex")
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=project,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        self.assertFalse(authority.destructive_allowed(acknowledged=True), authority.explain(acknowledged=True))
        self.assertNotIn(rollout, {action.path for action in plan.actions})

    def test_history_trim_preserves_retained_bytes_including_crlf(self):
        remove_id = "41414141-4141-4141-8141-414141414141"
        keep_id = "42424242-4242-4242-8242-424242424242"
        history = self.claude / "history.jsonl"
        original = (
            (json.dumps({"sessionId": remove_id}) + "\r\n").encode()
            + b"malformed retained line\r\n"
            + json.dumps({"sessionId": keep_id}).encode()
        )
        history.write_bytes(original)

        removed, _bytes, status = cleaner.trim_claude_history(history, {remove_id})
        self.assertEqual((removed, status), (1, "trimmed"))
        self.assertEqual(history.read_bytes(), b"malformed retained line\r\n" + json.dumps({"sessionId": keep_id}).encode())


class ReviewResponseTests(unittest.TestCase):
    """Regressions for every defect the independent E00 review found."""

    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.codex, self.claude = make_provider_roots(self.base)
        use_as_default_roots(self, self.codex, self.claude)
        self.locked = []

    def tearDown(self):
        # Restore permissions before the temporary tree is removed.
        for path, mode in reversed(self.locked):
            with contextlib.suppress(OSError):
                path.chmod(mode)
        self.td.cleanup()

    def deny(self, path: Path, restore_mode: int):
        self.locked.append((path, restore_mode))
        path.chmod(0o000)

    # --- E00-S01: the name barrier must not be defeated by resolution --------
    def test_protected_symlink_pointing_outside_the_root_is_refused(self):
        for tool, root, names in (
            ("codex", self.codex, cleaner.CODEX_PROTECTED_NAMES),
            ("claude", self.claude, cleaner.CLAUDE_PROTECTED_NAMES),
        ):
            outside = self.base / f"external-{tool}"
            outside.mkdir()
            link = root / "plugins"
            link.symlink_to(outside, target_is_directory=True)
            with self.assertRaises(cleaner.SafetyError, msg=tool):
                cleaner.safe_remove(link, root, names)
            self.assertTrue(link.is_symlink(), tool)
            self.assertIsNotNone(cleaner.protected_component(link, root, names), tool)
            link.unlink()

    def test_protected_name_reached_through_a_dot_dot_path_is_refused(self):
        deep = self.codex / "tmp" / "nested"
        deep.mkdir(parents=True)
        sneaky = deep / ".." / ".." / "plugins" / "state.bin"
        write_file(self.codex / "plugins" / "state.bin", "keep")
        self.assertEqual(cleaner.protected_component(sneaky, self.codex, cleaner.CODEX_PROTECTED_NAMES), "plugins")

    # --- E00-S04: a boundary that fires at execution is a block, not a crash --
    def test_execution_time_root_refusal_becomes_exit_blocked(self):
        sid = "61616161-6161-4161-8161-616161616161"
        codex_rollout(self.codex, sid, "2026-01-01", 30)
        env = {"CODEX_HOME": str(self.codex), "CLAUDE_CONFIG_DIR": str(self.claude)}
        real = cleaner.fingerprint_root
        calls = {"n": 0}

        def flip(path, tool):
            authority = real(path, tool)
            calls["n"] += 1
            if calls["n"] <= 2:
                return authority
            # The root stops looking like a provider between planning and execution.
            return cleaner.RootAuthority(tool=tool, path=authority.path, origin="custom", confidence="unknown", markers=())

        buffer = io.StringIO()
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(sys, "stdout", new=buffer),
            mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()),
            mock.patch.object(cleaner, "fingerprint_root", side_effect=flip),
        ):
            code = cleaner.main(["clean", "-y", "--json", "--keep-latest", "0", "--tool", "codex", "--codex-backend", "filesystem"])
        payload = json.loads(buffer.getvalue())
        self.assertEqual(code, cleaner.EXIT_BLOCKED)
        self.assertEqual(payload["exit_code"], cleaner.EXIT_BLOCKED)
        self.assertTrue(payload["result"]["deferred"])

    # --- E00-S05: every observation error reaches its scope ------------------
    def test_unreadable_codex_lineage_marks_the_scan_partial(self):
        sid = "62626262-6262-4262-8262-626262626262"
        rollout = codex_rollout(self.codex, sid, "2026-01-01", 30)
        self.deny(rollout, 0o644)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        self.assertFalse(plan.scan_complete)
        self.assertEqual(plan.withheld, ["codex"])
        self.assertTrue(rollout.exists())

    def test_unreadable_history_marks_the_scan_partial(self):
        sid = "63636363-6363-4363-8363-636363636363"
        claude_session(self.claude, "-project", sid, 30)
        history = self.claude / "history.jsonl"
        history.write_text(json.dumps({"sessionId": sid}) + "\n", encoding="utf-8")
        self.deny(history, 0o644)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"claude"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        self.assertFalse(plan.scan_complete)
        self.assertEqual(plan.withheld, ["claude"])

    def test_status_reports_root_totals_as_lower_bounds_when_partial(self):
        blocked = self.claude / "file-history" / "locked"
        write_file(blocked / "inner.txt", "data", 30)
        self.deny(blocked, 0o755)
        env = {"CODEX_HOME": str(self.codex), "CLAUDE_CONFIG_DIR": str(self.claude)}
        buffer = io.StringIO()
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(sys, "stdout", new=buffer),
            mock.patch.object(cleaner, "active_processes", return_value=quiet_processes()),
        ):
            cleaner.main(["status", "--tool", "claude"])
        self.assertIn("at least; scan incomplete", buffer.getvalue())

    # --- E00-S06: retained bytes are copied verbatim -------------------------
    def test_history_trim_preserves_crlf_and_missing_trailing_newline(self):
        drop = "64646464-6464-4464-8464-646464646464"
        keep = "65656565-6565-4565-8565-656565656565"
        history = self.claude / "history.jsonl"
        expected = b"raw \xff bytes retained\r\n" + json.dumps({"sessionId": keep}).encode()
        history.write_bytes((json.dumps({"sessionId": drop}) + "\r\n").encode() + expected)
        removed, _bytes, status = cleaner.trim_claude_history(history, {drop})
        self.assertEqual((removed, status), (1, "trimmed"))
        self.assertEqual(history.read_bytes(), expected)

    # --- E00-S08: `cleanable` may not overclaim ------------------------------
    def test_coverage_states_match_what_cleanup_actually_reaches(self):
        self.assertEqual(cleaner.coverage_state("history.jsonl", "claude"), "trimmed")
        self.assertEqual(cleaner.coverage_state("backups", "claude"), "aggressive-only")
        self.assertEqual(cleaner.coverage_state("todos", "claude"), "aggressive-only")
        self.assertEqual(cleaner.coverage_state("cache", "claude"), "aggressive-only")
        self.assertEqual(cleaner.coverage_state("file-history", "claude"), "cleanable")
        self.assertEqual(cleaner.coverage_state("history.jsonl", "codex"), "unknown")
        self.assertEqual(set(cleaner.COVERAGE_LEGEND), set(cleaner.COVERAGE_STATES))

    def test_no_standalone_history_file_is_ever_selected_for_deletion(self):
        history = self.claude / "history.jsonl"
        history.write_text(json.dumps({"sessionId": "x"}) + "\n", encoding="utf-8")
        os.utime(history, (time.time() - 400 * DAY, time.time() - 400 * DAY))
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"claude"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=True,
        )
        self.assertNotIn(history, {a.path for a in plan.actions})

    # --- E00-S09: unknown activity is not absence of activity ----------------
    def test_unknown_process_activity_blocks_cleanup(self):
        sid = "66666666-6666-4666-8666-666666666667"
        old = codex_rollout(self.codex, sid, "2026-01-01", 30)
        plan = cleaner.build_plan(
            days=7,
            keep_latest=0,
            tools={"codex"},
            codex_home=self.codex,
            claude_home=self.claude,
            codex_backend="filesystem",
            aggressive=False,
        )
        with mock.patch.object(cleaner, "active_processes", return_value=unknown_processes()):
            result = cleaner.execute_plan(
                plan,
                codex_home=self.codex,
                claude_home=self.claude,
                dry_run=False,
                allow_running=False,
                trim_history=True,
                verbose=False,
            )
        self.assertTrue(old.exists())
        self.assertTrue(result.partial)
        self.assertIn("codex", result.blocked_tools)
        self.assertTrue(any("could not determine" in e.lower() for e in result.errors))

    def test_unusable_ps_output_is_reported_as_incomplete(self):
        for stdout, returncode in (("", 0), ("garbage without pids", 0), ("1 codex", 1)):
            completed = subprocess.CompletedProcess(args=["ps"], returncode=returncode, stdout=stdout)
            with mock.patch.object(cleaner.subprocess, "run", return_value=completed):
                self.assertFalse(cleaner.active_processes().complete, (stdout, returncode))

    def test_successful_ps_output_is_reported_as_complete(self):
        completed = subprocess.CompletedProcess(args=["ps"], returncode=0, stdout=f"{os.getpid()} python3\n424242 codex\n")
        with mock.patch.object(cleaner.subprocess, "run", return_value=completed):
            observation = cleaner.active_processes()
        self.assertTrue(observation.complete)
        self.assertEqual(observation.running("codex"), [424242])
        self.assertEqual(observation.running("claude"), [])


if __name__ == "__main__":
    unittest.main(verbosity=2)
