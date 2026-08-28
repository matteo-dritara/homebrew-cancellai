"""Tests for the synthetic provider-layout fixture corpus (E01-S02).

Two things are exercised here, deliberately kept apart:

- that every fixture is a *credible* provider layout, verified against the real reference
  implementation (fingerprint_root, discover_*, build_plan) rather than merely "did it build";
- that scripts/check_fixtures.py actually catches a broken corpus, not only passes on the
  good one - a checker with no failing case proves nothing about itself.
"""

from __future__ import annotations

import contextlib
import copy
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import check_fixtures

cancellai = check_fixtures.load_recipes().cancellai
recipes = check_fixtures.load_recipes()


class FixtureCorpusTests(unittest.TestCase):
    def setUp(self):
        self.td = tempfile.TemporaryDirectory()
        self.base = Path(self.td.name)
        self.locked: list[Path] = []

    def tearDown(self):
        for path in self.locked:
            with contextlib.suppress(OSError):
                path.chmod(0o755)
        self.td.cleanup()

    def build(self, fixture_id: str) -> Path:
        root = self.base / fixture_id
        root.mkdir()
        recipes.build(fixture_id, root)
        return root

    def manifest_entries(self) -> list[dict]:
        return check_fixtures.load_manifest()["fixtures"]

    # --- corpus-level checks ------------------------------------------------

    def test_manifest_is_internally_valid(self):
        self.assertEqual([], check_fixtures.validate())

    def test_every_fixture_is_recognized_by_the_reference_fingerprint(self):
        for entry in self.manifest_entries():
            with self.subTest(fixture=entry["id"]):
                root = self.build(entry["id"])
                authority = cancellai.fingerprint_root(root, entry["tool"])
                self.assertEqual(authority.origin, "custom")
                self.assertNotEqual(
                    authority.confidence,
                    "unknown",
                    f"{entry['id']}: reference fingerprinting does not recognize this as a {entry['tool']} root at all",
                )

    # --- category-specific falsification checks ------------------------------

    def test_subagent_tree_children_link_to_the_root(self):
        root = self.build("codex-subagent-tree")
        rollouts = sorted(root.rglob("rollout-*.jsonl"))
        self.assertEqual(3, len(rollouts))
        root_id = "33333333-3333-4333-8333-333333333333"
        parents = {path.name: cancellai.read_codex_parent_session_id(path) for path in rollouts}
        children = [name for name, parent in parents.items() if parent == root_id]
        self.assertEqual(2, len(children), parents)

    def test_protected_state_is_never_selected_even_aggressive_and_far_past_cutoff(self):
        empty_other = self.base / "unused-home"
        for tool, fixture_id, protected_names in (
            ("claude", "claude-protected-state", cancellai.CLAUDE_PROTECTED_NAMES),
            ("codex", "codex-protected-state", cancellai.CODEX_PROTECTED_NAMES),
        ):
            with self.subTest(tool=tool):
                root = self.build(fixture_id)
                homes = {"codex": empty_other, "claude": empty_other}
                homes[tool] = root
                plan = cancellai.build_plan(
                    days=1,
                    keep_latest=0,
                    tools={tool},
                    codex_home=homes["codex"],
                    claude_home=homes["claude"],
                    codex_backend="filesystem",
                    aggressive=True,
                    for_mutation=False,
                )
                for action in plan.actions:
                    relative = action.path.resolve(strict=False).relative_to(root.resolve(strict=False))
                    self.assertNotIn(
                        relative.parts[0] if relative.parts else "",
                        protected_names,
                        f"{fixture_id}: {action.path} is under a protected name but was selected anyway",
                    )

    def test_symlink_escape_fixture_actually_resolves_outside_its_root(self):
        root = self.build("codex-symlink-escape")
        link = root / "sessions" / "escape.jsonl"
        self.assertTrue(link.is_symlink())
        self.assertFalse(cancellai.is_within(link, root))

    def test_symlink_protected_name_fixture_actually_resolves_outside_its_root(self):
        root = self.build("claude-symlink-protected-name")
        link = root / "Plugins"
        self.assertTrue(link.is_symlink())
        self.assertFalse(cancellai.is_within(link, root))

    def test_partial_tree_fixture_has_exactly_one_unlistable_subtree(self):
        root = self.build("claude-partial-tree")
        project_dir = root / "projects" / "synthetic-project-c"
        session_files = sorted(project_dir.glob("*.jsonl"))
        self.assertEqual(2, len(session_files))
        locked_dir = project_dir / "locked-subagent"
        self.assertTrue(locked_dir.is_dir())
        self.locked.append(locked_dir)

        scan = cancellai.Scan(scope="test")
        cancellai.directory_size(project_dir, scan)
        self.assertFalse(scan.complete)
        self.assertTrue(any("locked-subagent" in error for error in scan.errors), scan.errors)

    # --- the checker must actually be able to fail ---------------------------

    def test_checker_flags_a_missing_category(self):
        data = check_fixtures.load_manifest()
        data = copy.deepcopy(data)
        data["fixtures"] = [f for f in data["fixtures"] if f["category"] != "symlink"]
        with mock.patch.object(check_fixtures, "load_manifest", return_value=data):
            errors = check_fixtures.validate()
        self.assertTrue(any("symlink" in e for e in errors), errors)

    def test_checker_flags_an_unknown_recipe_reference(self):
        data = copy.deepcopy(check_fixtures.load_manifest())
        data["fixtures"].append(
            {
                "id": "does-not-exist",
                "tool": "claude",
                "category": "normal_session",
                "layout": "default",
                "description": "no matching recipe on purpose",
            }
        )
        with mock.patch.object(check_fixtures, "load_manifest", return_value=data):
            errors = check_fixtures.validate()
        self.assertTrue(any("does-not-exist" in e and "no matching recipe" in e for e in errors), errors)

    def test_checker_flags_forbidden_content(self):
        data = {
            "schema_version": 1,
            "fixtures": [
                {
                    "id": "claude-normal-session",
                    "tool": "claude",
                    "category": "normal_session",
                    "layout": "default",
                    "description": "deliberately leaks an email address",
                }
            ],
        }

        def leaky_recipe(root: Path) -> None:
            (root / "leak.txt").write_text("contact operator@example.com for help", encoding="utf-8")

        fake_recipes = mock.Mock()
        fake_recipes.FIXTURES = {"claude-normal-session": leaky_recipe}
        fake_recipes.build = lambda fixture_id, root: leaky_recipe(root)

        with (
            mock.patch.object(check_fixtures, "load_manifest", return_value=data),
            mock.patch.object(check_fixtures, "load_recipes", return_value=fake_recipes),
        ):
            errors = check_fixtures.validate()
        self.assertTrue(any("forbidden pattern" in e for e in errors), errors)

    def test_checker_flags_an_orphaned_recipe(self):
        data = copy.deepcopy(check_fixtures.load_manifest())
        data["fixtures"] = [f for f in data["fixtures"] if f["id"] != "claude-normal-session"]
        with mock.patch.object(check_fixtures, "load_manifest", return_value=data):
            errors = check_fixtures.validate()
        self.assertTrue(any("claude-normal-session" in e and "no manifest entry" in e for e in errors), errors)


if __name__ == "__main__":
    unittest.main()
