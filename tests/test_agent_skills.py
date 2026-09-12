"""Tests for the agent skill pack checker (E24-S01).

Mirrors the other governance checkers' approach: prove the real pack is consistent, then prove
the checker actually rejects each way a skill can drift from the contract it cites - a checker
with no failing case proves nothing about itself.

The drift cases are built as synthetic trees under a temporary directory, never by mutating the
committed pack.
"""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts import check_agent_skills as checker

GOOD_FRONTMATTER = """---
name: {name}
description: Does a specific thing. Use when the specific thing is needed.
---

# {name}

Read [the protocol](docs/development/AGENT_PROTOCOL.md), then run:

```sh
python3 scripts/project_os.py check
```
"""


PROBE = """---
name: probe
description: Does a specific thing. Use when the specific thing is needed.
---

See {citation} for details.
"""


def write_skill(root: Path, name: str, text: str) -> Path:
    skill = root / ".claude" / "skills" / name
    skill.mkdir(parents=True)
    (skill / "SKILL.md").write_text(text, encoding="utf-8")
    return skill


def fake_repository(root: Path) -> None:
    """The subset of the real repository a synthetic skill is allowed to cite."""
    (root / "docs" / "development").mkdir(parents=True)
    (root / "docs" / "development" / "AGENT_PROTOCOL.md").write_text("protocol\n", encoding="utf-8")
    (root / "scripts").mkdir(parents=True)
    (root / "scripts" / "project_os.py").write_text("# stub\n", encoding="utf-8")
    (root / "project" / "epics").mkdir(parents=True)
    # A pattern must match something, so the synthetic repository has to contain something for
    # the accepted patterns to match - an empty directory is exactly the case the checker now
    # rejects, and rightly.
    (root / "project" / "epics" / "E00.json").write_text("{}\n", encoding="utf-8")
    (root / "project" / "evidence" / "E00-S01").mkdir(parents=True)
    (root / "project" / "evidence" / "E00-S01" / "EVIDENCE.md").write_text("packet\n", encoding="utf-8")


class RealPackTests(unittest.TestCase):
    def test_the_committed_pack_is_consistent(self):
        count = checker.validate(checker.SKILLS_DIR, checker.ROOT)
        self.assertGreater(count, 0)

    def test_the_reported_count_matches_the_directories_present(self):
        expected = len(checker.skill_directories(checker.SKILLS_DIR))
        self.assertEqual(expected, checker.validate(checker.SKILLS_DIR, checker.ROOT))

    def test_the_command_line_entry_point_passes(self):
        self.assertEqual(0, checker.main(["check"]))


class DriftTests(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        fake_repository(self.root)
        self.skills = self.root / ".claude" / "skills"

    def tearDown(self):
        self._tmp.cleanup()

    def validate(self) -> int:
        return checker.validate(self.skills, self.root)

    def assert_rejected(self, fragment: str) -> None:
        with self.assertRaises(checker.AgentSkillsError) as caught:
            self.validate()
        self.assertIn(fragment, str(caught.exception))

    def test_a_well_formed_skill_passes(self):
        write_skill(self.root, "orient", GOOD_FRONTMATTER.format(name="orient"))
        self.assertEqual(1, self.validate())

    def test_a_name_that_disagrees_with_its_directory_is_rejected(self):
        write_skill(self.root, "orient", GOOD_FRONTMATTER.format(name="orientation"))
        self.assert_rejected("does not match its directory")

    def test_a_missing_frontmatter_block_is_rejected(self):
        write_skill(self.root, "orient", "# orient\n\nno frontmatter here\n")
        self.assert_rejected("no YAML frontmatter")

    def test_an_unterminated_frontmatter_block_is_rejected(self):
        write_skill(self.root, "orient", "---\nname: orient\ndescription: Use when needed.\n")
        self.assert_rejected("not terminated by a closing")

    def test_a_missing_description_is_rejected(self):
        write_skill(self.root, "orient", "---\nname: orient\n---\n\nbody\n")
        self.assert_rejected("has no `description`")

    def test_a_description_that_never_says_when_to_apply_is_rejected(self):
        text = "---\nname: orient\ndescription: Does a specific thing.\n---\n\nbody\n"
        write_skill(self.root, "orient", text)
        self.assert_rejected("does not say when the skill applies")

    def test_an_oversized_description_is_rejected(self):
        filler = "x" * (checker.DESCRIPTION_MAX + 1)
        text = f"---\nname: orient\ndescription: Use when needed. {filler}\n---\n\nbody\n"
        write_skill(self.root, "orient", text)
        self.assert_rejected("characters (max")

    def test_a_cited_path_that_does_not_exist_is_rejected(self):
        text = GOOD_FRONTMATTER.format(name="orient").replace("docs/development/AGENT_PROTOCOL.md", "docs/development/DELETED_DOCUMENT.md")
        write_skill(self.root, "orient", text)
        self.assert_rejected("cites a path that does not exist")

    def test_a_named_script_that_does_not_exist_is_rejected(self):
        # Asserted on the script check's own message, not on the substring "does not exist",
        # which the path check also produces for the same token - an independent review showed
        # that a SCRIPT_COMMAND regex mutated to match nothing left the looser assertion green.
        text = GOOD_FRONTMATTER.format(name="orient").replace("scripts/project_os.py check", "scripts/gone.py check")
        write_skill(self.root, "orient", text)
        self.assert_rejected("names a script that does not exist")

    def test_a_skill_directory_without_a_manifest_is_rejected(self):
        (self.skills / "orient").mkdir(parents=True)
        self.assert_rejected("no SKILL.md")

    def test_a_pattern_whose_directory_exists_is_accepted(self):
        # `project/epics/*.json` and `project/evidence/<STORY-ID>/` stand for a class of file,
        # not one file; requiring the named instance to exist would make them uncitable.
        text = GOOD_FRONTMATTER.format(name="orient").replace(
            "docs/development/AGENT_PROTOCOL.md", "project/epics/*.json and project/evidence/<STORY-ID>/EVIDENCE.md"
        )
        write_skill(self.root, "orient", text)
        self.assertEqual(1, self.validate())

    def test_a_pattern_that_matches_nothing_is_rejected(self):
        # The named instance cannot be required to exist, but the pattern must still expand to
        # something - otherwise a broken pointer passes simply by wearing a placeholder.
        text = GOOD_FRONTMATTER.format(name="orient").replace("docs/development/AGENT_PROTOCOL.md", "project/moved-away/*.json")
        write_skill(self.root, "orient", text)
        self.assert_rejected("cites a pattern that matches nothing")

    def test_a_support_file_directory_is_not_counted_as_a_skill(self):
        write_skill(self.root, "orient", GOOD_FRONTMATTER.format(name="orient"))
        (self.skills / "_shared").mkdir()
        self.assertEqual(1, self.validate())

    def test_a_deleted_pack_is_reported_rather_than_silently_passing(self):
        self.assert_rejected("no agent skill pack at")

    def test_an_empty_pack_is_reported_rather_than_silently_passing(self):
        self.skills.mkdir(parents=True)
        self.assert_rejected("contains no skills")

    def test_every_problem_is_reported_at_once_rather_than_the_first(self):
        write_skill(self.root, "orient", "---\nname: wrong\ndescription: Does a thing.\n---\n\nbody\n")
        with self.assertRaises(checker.AgentSkillsError) as caught:
            self.validate()
        message = str(caught.exception)
        self.assertIn("does not match its directory", message)
        self.assertIn("does not say when the skill applies", message)


class CitationRegressionTests(unittest.TestCase):
    """Every case below is a reproduction from the independent review of E24-S01.

    The first implementation skipped any citation containing a placeholder, which validated
    `docs/*totally/fake/path.md` as `docs`; it also never matched a dot-relative citation, and
    never matched a root-level file at all - including `AGENTS.md`, the pack's most-cited
    target. These run against the REAL repository, so they also fail if one of the accepted
    shapes stops resolving.
    """

    REJECTED = (
        "docs/*totally/fake/path.md",
        "project/<X>/completely/invented.json",
        "scripts/$NOPE/not_a_script.py",
        "tests/*junk/nowhere/at/all.rs",
        "../../scripts/gone.py",
        "./docs/GONE.md",
        "docs/development/DELETED_DOCUMENT.md",
    )
    ACCEPTED = (
        "AGENTS.md",
        "CHANGELOG.md",
        "cancellai.py",
        "project/epics/*.json",
        "project/evidence/<STORY-ID>/*.md",
        "docs/<AREA>/*.md",
        "rust/crates/*/Cargo.toml",
        "docs/metadata",
        "docs/development/AGENT_PROTOCOL.md",
    )

    def problems_for(self, citation: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            skill = write_skill(Path(tmp), "probe", PROBE.format(citation=citation)).parent.parent.parent
            return checker.check_skill(skill / ".claude" / "skills" / "probe", checker.ROOT)

    def test_a_broken_pointer_is_rejected_however_it_is_spelled(self):
        for citation in self.REJECTED:
            with self.subTest(citation=citation):
                self.assertTrue(self.problems_for(citation), f"{citation} was accepted")

    def test_a_resolvable_pointer_is_accepted(self):
        for citation in self.ACCEPTED:
            with self.subTest(citation=citation):
                self.assertEqual([], self.problems_for(citation))

    def test_a_pattern_is_required_to_match_something(self):
        # The distinction the glob expansion buys: both are patterns, only one names files.
        self.assertEqual([], self.problems_for("project/epics/*.json"))
        self.assertTrue(self.problems_for("project/epics/*.nonexistent-extension"))


class FrontmatterShapeTests(unittest.TestCase):
    """YAML shapes a skill author will actually write must not be misdiagnosed.

    The pack is portable by design, so a SKILL.md that any YAML-conformant harness loads must
    not be rejected here - and a malformed one must be reported for what it is.
    """

    def problems_for(self, text: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            write_skill(root, "probe", text)
            return checker.check_skill(root / ".claude" / "skills" / "probe", checker.ROOT)

    def test_a_quoted_scalar_is_understood(self):
        self.assertEqual([], self.problems_for('---\nname: "probe"\ndescription: "Does. Use when needed."\n---\n\nbody\n'))

    def test_a_folded_block_scalar_is_understood(self):
        text = "---\nname: probe\ndescription: >\n  Does a thing.\n  Use when the thing is needed.\n---\n\nbody\n"
        self.assertEqual([], self.problems_for(text))

    def test_a_byte_order_mark_does_not_hide_the_frontmatter(self):
        self.assertEqual([], self.problems_for("\ufeff---\nname: probe\ndescription: Does. Use when needed.\n---\n\nbody\n"))

    def test_an_empty_frontmatter_block_is_reported_as_empty_not_as_unterminated(self):
        self.assertEqual(["probe: YAML frontmatter is empty"], self.problems_for("---\n---\n\nbody\n"))

    def test_an_unterminated_block_is_reported_as_unterminated(self):
        problems = self.problems_for("---\nname: probe\ndescription: Use when needed.\n")
        self.assertEqual(["probe: YAML frontmatter is not terminated by a closing `---`"], problems)

    def test_a_file_with_no_frontmatter_at_all_says_so(self):
        self.assertEqual(["probe: no YAML frontmatter"], self.problems_for("# probe\n\nbody\n"))


if __name__ == "__main__":
    unittest.main()
