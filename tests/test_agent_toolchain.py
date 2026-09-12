"""Tests for agent-toolchain governance (E26-S01).

The manifest governs third-party code and third-party prompt content entering the agent that
writes this repository. The cases below are the ways such a control silently stops controlling:
an unmanaged component that nobody notices, a privileged capability admitted on trust nobody
granted, a decision that never expires, and a context budget that is only advisory.
"""

from __future__ import annotations

import datetime as dt
import unittest

from scripts import check_agent_toolchain as toolchain

TODAY = dt.date(2026, 9, 12)


def component(**overrides):
    base = {
        "id": "example",
        "kind": "plugin",
        "scope": "user",
        "source": "github:owner/name",
        "version": "1.0.0",
        "trust": "Vendor",
        "capabilities": ["prompt"],
        "always_on_tokens": 100,
        "purpose": "does a thing this project needs",
        "decision": {"status": "approved", "by": "owner", "date": "2026-09-01", "rationale": "because"},
    }
    base.update(overrides)
    return base


def manifest(components, **overrides):
    base = {"schema_version": 1, "review_cadence_days": 90, "context_budget_tokens": 1000, "components": components, "rejected": []}
    base.update(overrides)
    return base


class ComponentValidationTests(unittest.TestCase):
    def test_a_well_formed_component_passes(self):
        self.assertEqual([], toolchain.validate_component(component()))

    def test_a_missing_field_is_named(self):
        entry = component()
        del entry["trust"]
        self.assertTrue(any("missing required field 'trust'" in e for e in toolchain.validate_component(entry)))

    def test_an_unknown_kind_is_refused(self):
        self.assertTrue(any("unknown kind" in e for e in toolchain.validate_component(component(kind="wizardry"))))

    def test_a_decision_without_a_rationale_is_refused(self):
        entry = component(decision={"status": "approved", "by": "owner", "date": "2026-09-01", "rationale": "   "})
        self.assertTrue(any("no rationale" in e for e in toolchain.validate_component(entry)))

    def test_a_component_with_no_purpose_is_refused(self):
        self.assertTrue(any("no purpose" in e for e in toolchain.validate_component(component(purpose=""))))


class PrivilegeTests(unittest.TestCase):
    """A component that runs code is software, not text, and the trust bar follows."""

    def test_community_trust_may_not_execute_code(self):
        entry = component(trust="Community", capabilities=["prompt", "executes-code"])
        self.assertTrue(any("must be FirstParty or Vendor" in e for e in toolchain.validate_component(entry)))

    def test_unknown_trust_may_not_hold_credentials(self):
        entry = component(trust="Unknown", capabilities=["credentials"])
        self.assertTrue(any("credentials" in e for e in toolchain.validate_component(entry)))

    def test_community_trust_is_fine_for_prompt_only(self):
        self.assertEqual([], toolchain.validate_component(component(trust="Community", capabilities=["prompt"])))

    def test_vendor_trust_may_execute_code(self):
        self.assertEqual([], toolchain.validate_component(component(trust="Vendor", capabilities=["executes-code"])))


class ReconciliationTests(unittest.TestCase):
    def test_an_installed_component_absent_from_the_manifest_fails(self):
        # The whole supply-chain control: an addition nobody decided to carry becomes visible.
        errors, _ = toolchain.evaluate(manifest([]), {"mystery-hook": ".claude/hooks/mystery.sh"}, TODAY)
        self.assertTrue(any("not in the manifest" in e for e in errors))

    def test_a_project_scoped_component_in_the_manifest_but_not_installed_fails(self):
        errors, _ = toolchain.evaluate(manifest([component(id="gone", scope="project")]), {}, TODAY)
        self.assertTrue(any("not installed" in e for e in errors))

    def test_a_user_scoped_component_is_not_required_to_be_present(self):
        # CI cannot see a user-scoped plugin. Claiming to verify what it cannot see would be
        # worse than admitting the boundary.
        errors, _ = toolchain.evaluate(manifest([component(scope="user")]), {}, TODAY)
        self.assertEqual([], errors)

    def test_a_retired_component_is_not_required_to_be_present(self):
        entry = component(id="old", scope="project", decision={"status": "retired", "by": "owner", "date": "2026-01-01", "rationale": "unused"})
        errors, _ = toolchain.evaluate(manifest([entry]), {}, TODAY)
        self.assertEqual([], errors)

    def test_duplicate_ids_are_refused(self):
        errors, _ = toolchain.evaluate(manifest([component(), component()]), {}, TODAY)
        self.assertTrue(any("duplicate component ids" in e for e in errors))


class ExpiryTests(unittest.TestCase):
    def test_a_decision_past_the_cadence_is_reported(self):
        entry = component(decision={"status": "approved", "by": "owner", "date": "2026-01-01", "rationale": "because"})
        _, warnings = toolchain.evaluate(manifest([entry], review_cadence_days=90), {}, TODAY)
        self.assertTrue(any("past the 90-day review cadence" in w for w in warnings))

    def test_a_recent_decision_is_not_reported(self):
        _, warnings = toolchain.evaluate(manifest([component()]), {}, TODAY)
        self.assertEqual([], warnings)

    def test_a_retired_component_does_not_expire(self):
        entry = component(decision={"status": "retired", "by": "owner", "date": "2020-01-01", "rationale": "gone"})
        self.assertEqual([], toolchain.stale_decisions([entry], 90, TODAY))

    def test_a_malformed_date_is_an_error_rather_than_an_assumed_pass(self):
        entry = component(decision={"status": "approved", "by": "owner", "date": "yesterday", "rationale": "because"})
        with self.assertRaises(toolchain.ToolchainError):
            toolchain.stale_decisions([entry], 90, TODAY)


class BudgetTests(unittest.TestCase):
    def test_exceeding_the_context_budget_fails(self):
        # C-11 by analogy: a governance tool may not become an unbounded producer of the resource
        # it governs. Here the resource is the context every session pays before any work begins.
        heavy = [component(id=f"c{n}", always_on_tokens=400) for n in range(4)]
        errors, _ = toolchain.evaluate(manifest(heavy, context_budget_tokens=1000), {}, TODAY)
        self.assertTrue(any("always-on context cost" in e for e in errors))

    def test_a_retired_component_does_not_consume_budget(self):
        retired = component(id="old", always_on_tokens=5000, decision={"status": "retired", "by": "o", "date": "2026-09-01", "rationale": "x"})
        errors, _ = toolchain.evaluate(manifest([retired], context_budget_tokens=1000), {}, TODAY)
        self.assertEqual([], errors)


class RejectionTests(unittest.TestCase):
    def test_a_rejection_without_a_reason_is_refused(self):
        data = manifest([], rejected=[{"id": "something", "reason": "  "}])
        errors, _ = toolchain.evaluate(data, {}, TODAY)
        self.assertTrue(any("records no reason" in e for e in errors))


class RealManifestTests(unittest.TestCase):
    def test_the_committed_manifest_passes(self):
        self.assertEqual(0, toolchain.main(["check"]))

    def test_the_report_renders(self):
        self.assertEqual(0, toolchain.main(["report"]))

    def test_every_committed_component_validates(self):
        for entry in toolchain.load_manifest()["components"]:
            with self.subTest(component=entry["id"]):
                self.assertEqual([], toolchain.validate_component(entry))

    def test_every_privileged_component_is_trusted_for_it(self):
        for entry in toolchain.load_manifest()["components"]:
            privileged = [c for c in entry["capabilities"] if c in toolchain.PRIVILEGED]
            if privileged:
                with self.subTest(component=entry["id"]):
                    self.assertIn(entry["trust"], toolchain.TRUSTED_FOR_PRIVILEGE)

    def test_the_committed_toolchain_is_inside_its_budget(self):
        data = toolchain.load_manifest()
        live = [c for c in data["components"] if c["decision"]["status"] != "retired"]
        self.assertLessEqual(sum(c["always_on_tokens"] for c in live), data["context_budget_tokens"])

    def test_the_skill_pack_is_a_managed_component(self):
        ids = {c["id"] for c in toolchain.load_manifest()["components"]}
        self.assertIn("cancellai-skill-pack", ids)
        self.assertIn("guard-generated-docs", ids)


if __name__ == "__main__":
    unittest.main()
