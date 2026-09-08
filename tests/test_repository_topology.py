"""Tests for the canonical-source-repository identification check (E17-S06, AC2).

Mirrors the other governance checkers' approach: prove the real repository state is
consistent, then prove the checker actually rejects a drift - a checker with no failing case
proves nothing about itself.
"""

from __future__ import annotations

import unittest

from scripts import check_repository_topology as topo

FORMULA_TEXT = 'homepage "https://github.com/matteo-dritara/homebrew-cancellai"\n'
RELEASE_PY_TEXT = 'REPO = "matteo-dritara/homebrew-cancellai"\n'
RELEASING_MD_TEXT = "The current remote is `matteo-dritara/homebrew-cancellai`.\n"


class RepositoryTopologyTests(unittest.TestCase):
    def test_the_real_repository_state_is_consistent(self):
        self.assertEqual([], topo.check())

    def test_matching_texts_produce_no_errors(self):
        errors = topo.check_texts(FORMULA_TEXT, RELEASE_PY_TEXT, RELEASING_MD_TEXT)
        self.assertEqual([], errors)

    def test_a_drifted_formula_homepage_is_flagged(self):
        drifted = 'homepage "https://github.com/matteo-dritara/cancellai"\n'
        errors = topo.check_texts(drifted, RELEASE_PY_TEXT, RELEASING_MD_TEXT)
        self.assertTrue(any("ambiguous" in e for e in errors), errors)

    def test_a_drifted_release_py_repo_constant_is_flagged(self):
        drifted = 'REPO = "matteo-dritara/cancellai"\n'
        errors = topo.check_texts(FORMULA_TEXT, drifted, RELEASING_MD_TEXT)
        self.assertTrue(any("ambiguous" in e for e in errors), errors)

    def test_a_drifted_releasing_md_claim_is_flagged(self):
        drifted = "The current remote is `matteo-dritara/cancellai`.\n"
        errors = topo.check_texts(FORMULA_TEXT, RELEASE_PY_TEXT, drifted)
        self.assertTrue(any("ambiguous" in e for e in errors), errors)

    def test_all_three_drifting_to_the_same_new_repository_is_not_flagged(self):
        # A real, deliberate migration (all three updated together) must not trip this check -
        # it is a drift detector, not a pin to today's specific repository name.
        formula = 'homepage "https://github.com/matteo-dritara/cancellai"\n'
        release_py = 'REPO = "matteo-dritara/cancellai"\n'
        releasing_md = "The current remote is `matteo-dritara/cancellai`.\n"
        errors = topo.check_texts(formula, release_py, releasing_md)
        self.assertEqual([], errors)

    def test_missing_homepage_line_is_a_clear_error_not_a_crash(self):
        with self.assertRaises(topo.TopologyError):
            topo.check_texts("no homepage here\n", RELEASE_PY_TEXT, RELEASING_MD_TEXT)

    def test_a_malformed_repository_value_is_rejected(self):
        malformed = 'homepage "https://github.com/not-a-repo-path"\n'
        with self.assertRaises(topo.TopologyError):
            topo.check_texts(malformed, RELEASE_PY_TEXT, RELEASING_MD_TEXT)

    def test_main_exits_zero_on_the_real_repository(self):
        self.assertEqual(0, topo.main([]))
        self.assertEqual(0, topo.main(["check"]))


if __name__ == "__main__":
    unittest.main(verbosity=2)
