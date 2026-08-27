from __future__ import annotations

import unittest

from scripts import check_workflows


class WorkflowPolicyTests(unittest.TestCase):
    def test_active_workflows_follow_supply_chain_policy(self) -> None:
        check_workflows.validate_workflows()

    def test_declared_contexts_expand_the_matrix_and_exclude_triggers(self) -> None:
        declared = check_workflows.declared_check_names()
        # A matrix job never reports its bare name; requiring it blocks every pull request
        # forever while reporting nothing. This is the bug that was live in the repository.
        self.assertNotIn("test", declared)
        self.assertLessEqual({"test (3.10)", "test (3.14)"}, declared)
        # `on:` keys are not jobs.
        self.assertFalse({"push", "pull_request", "schedule"} & declared)

    def test_every_required_check_matches_a_real_job(self) -> None:
        required = check_workflows.required_check_names()
        self.assertTrue(required, "REPOSITORY_GOVERNANCE.md must list the required checks")
        declared = check_workflows.declared_check_names()
        for name in required:
            self.assertIn(name, declared, name)


if __name__ == "__main__":
    unittest.main()
