from __future__ import annotations

import unittest

from scripts import check_workflows


class WorkflowPolicyTests(unittest.TestCase):
    def test_active_workflows_follow_supply_chain_policy(self) -> None:
        check_workflows.validate_workflows()


if __name__ == "__main__":
    unittest.main()
