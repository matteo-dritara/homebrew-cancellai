"""The checked inventory of CLI differences at the canonical switch (E06-S04, review round 6 F-04).

`project/cli_inventory.json` lists every long flag of the four commands the frozen Python CLI and
the Rust engine share. This checks that the list is complete against the Python parser, that every
flag it calls "same" really is in the Rust engine's committed help, and that every flag it calls
removed or changed is disclosed in the Unreleased release notes.
"""

from __future__ import annotations

import importlib.util
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
INVENTORY = json.loads((ROOT / "project" / "cli_inventory.json").read_text(encoding="utf-8"))
GOLDEN = ROOT / "rust" / "crates" / "cancellai-cli" / "tests" / "golden"


def python_flags() -> dict[str, set[str]]:
    spec = importlib.util.spec_from_file_location("cancellai_inventory_ref", ROOT / "cancellai.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["cancellai_inventory_ref"] = module
    spec.loader.exec_module(module)
    parser = module.build_parser()
    commands = parser._subparsers._group_actions[0].choices
    return {
        name: {option for action in sub._actions for option in action.option_strings if option.startswith("--") and option != "--help"}
        for name, sub in commands.items()
    }


def unreleased_notes() -> str:
    text = (ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    start = text.index("## [Unreleased]")
    end = text.index("\n## [", start + 1)
    return text[start:end]


class CliInventoryTests(unittest.TestCase):
    def test_every_python_flag_is_inventoried(self) -> None:
        for command, flags in python_flags().items():
            self.assertIn(command, INVENTORY["commands"], command)
            self.assertEqual(set(INVENTORY["commands"][command]), flags, command)

    def test_every_same_flag_is_in_the_rust_help(self) -> None:
        for command, flags in INVENTORY["commands"].items():
            help_text = (GOLDEN / f"{command}_help.txt").read_text(encoding="utf-8")
            for flag, disposition in flags.items():
                if disposition == "same":
                    self.assertIn(flag, help_text, f"{command} {flag}")

    def test_every_removed_or_changed_flag_is_disclosed_in_the_release_notes(self) -> None:
        notes = unreleased_notes()
        for command, flags in INVENTORY["commands"].items():
            for flag, disposition in flags.items():
                if disposition != "same":
                    self.assertIn(flag, notes, f"{command} {flag} ({disposition}) is not in the Unreleased notes")
        for topic in INVENTORY["presentation"]:
            self.assertIn(topic, notes.lower(), topic)


if __name__ == "__main__":
    unittest.main()
