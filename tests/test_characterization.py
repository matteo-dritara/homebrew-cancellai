"""Tests for the Python behavior characterization suite (E01-S04).

Same shape as tests/test_fixtures.py and tests/test_schemas.py: prove the committed corpus
is reproducible, then prove the checker actually notices when it stops being reproducible.
"""

from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import characterize


class CharacterizationTests(unittest.TestCase):
    def test_committed_characterization_matches_a_fresh_run(self):
        self.assertEqual([], characterize.check())

    def test_committed_characterization_is_reproducible_across_runs(self):
        first = characterize.characterize_all()
        second = characterize.characterize_all()
        self.assertEqual(first, second)

    def test_every_manifest_fixture_has_a_reviewed_classification(self):
        from scripts import check_fixtures

        manifest_ids = {entry["id"] for entry in check_fixtures.load_manifest()["fixtures"]}
        self.assertEqual(manifest_ids, set(characterize.CLASSIFICATIONS))
        for fixture_id, (classification, rationale) in characterize.CLASSIFICATIONS.items():
            with self.subTest(fixture=fixture_id):
                self.assertIn(classification, characterize.VALID_CLASSIFICATIONS)
                self.assertTrue(rationale.strip())

    def test_redact_paths_strips_the_volatile_prefix_wherever_it_appears(self):
        root = "/tmp/some-random-dir/provider-home"
        payload = {
            "notes": [f"Refusing destructive work: {root}/projects/x: Permission denied"],
            "nested": {"unreadable": [f"{root}/a", f"{root}/b: Permission denied"]},
            "unaffected": "value",
        }
        redacted = characterize._redact_dict(payload, root)
        self.assertNotIn(root, json.dumps(redacted))
        self.assertEqual("value", redacted["unaffected"])

    def test_characterize_one_rejects_a_fixture_with_no_classification(self):
        with self.assertRaises(characterize.CharacterizationError):
            characterize.characterize_one("does-not-exist", "claude")

    def test_characterize_one_rejects_an_invalid_classification_value(self):
        override = {"claude-normal-session": ("MAYBE", "not a real value")}
        with mock.patch.dict(characterize.CLASSIFICATIONS, override), self.assertRaises(characterize.CharacterizationError):
            characterize.characterize_one("claude-normal-session", "claude")

    def test_check_flags_a_committed_file_that_no_longer_matches_a_fresh_run(self):
        with tempfile.TemporaryDirectory() as tmp:
            tampered_dir = Path(tmp) / "characterization"
            shutil.copytree(characterize.CHARACTERIZATION_DIR, tampered_dir)
            target = tampered_dir / "claude-normal-session.characterization.json"
            data = json.loads(target.read_text(encoding="utf-8"))
            data["classification"] = "KNOWN_DEFECT"
            target.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")

            with mock.patch.object(characterize, "CHARACTERIZATION_DIR", tampered_dir):
                errors = characterize.check()
        self.assertTrue(any("does not match a fresh run" in e for e in errors), errors)

    def test_check_flags_a_missing_committed_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            sparse_dir = Path(tmp) / "characterization"
            shutil.copytree(characterize.CHARACTERIZATION_DIR, sparse_dir)
            (sparse_dir / "claude-normal-session.characterization.json").unlink()

            with mock.patch.object(characterize, "CHARACTERIZATION_DIR", sparse_dir):
                errors = characterize.check()
        self.assertTrue(any("do not match the fixture corpus" in e for e in errors), errors)


if __name__ == "__main__":
    unittest.main()
