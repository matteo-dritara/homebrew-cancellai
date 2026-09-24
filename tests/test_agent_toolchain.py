"""Tests for agent-toolchain governance (E26-S01).

The manifest governs third-party code and third-party prompt content entering the agent that
writes this repository. The cases below are the ways such a control silently stops controlling:
an unmanaged component that nobody notices, a privileged capability admitted on trust nobody
granted, a decision that never expires, and a context budget that is only advisory.
"""

from __future__ import annotations

import datetime as dt
import unittest
from typing import ClassVar

from scripts import check_agent_toolchain as toolchain

TODAY = dt.date(2026, 9, 12)


def component(**overrides):
    base = {
        "id": "example",
        "kind": "plugin",
        "scope": "user",
        "source": "github:owner/name",
        "version": "1.0.0",
        "license": "MIT",
        "trust": "Vendor",
        "capabilities": ["prompt"],
        "always_on_tokens": 100,
        "purpose": "does a thing this project needs",
        "decision": {"status": "approved", "by": "owner", "date": "2026-09-01", "rationale": "because"},
    }
    base.update(overrides)
    return base


def manifest(components, **overrides):
    base = {
        "schema_version": 1,
        "review_cadence_days": 90,
        "context_budget_tokens": 1000,
        # A manifest with no allow-list admits every licence, so the checker refuses one; a
        # synthetic manifest that omitted it would be testing that refusal by accident.
        "license_allowlist": ["MIT", "Apache-2.0"],
        "components": components,
        "rejected": [],
    }
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
        # Scoped to expiry: E28-S04 added an unmeasured-cost warning that every user-scope
        # component now carries, and asserting on the whole list would make this test fail for a
        # reason it is not about.
        _, warnings = toolchain.evaluate(manifest([component()]), {}, TODAY)
        self.assertEqual([], [w for w in warnings if "review cadence" in w])

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
        self.assertIn("hook:guard-generated-docs", ids)

    def test_every_skill_in_the_pack_is_named_as_a_member(self):
        # The pack is one decision, but each skill is named: an independent review added a ninth
        # skill and the first enumerator reported "nothing unmanaged".
        pack = next(c for c in toolchain.load_manifest()["components"] if c["id"] == "cancellai-skill-pack")
        installed = {i for i in toolchain.installed_project_components() if i.startswith("skill:")}
        self.assertEqual(installed, set(pack["members"]))

    def test_the_enumerator_sees_every_component_kind(self):
        # Each of these was invisible to the first version, which reported nothing unmanaged while
        # seven unmanaged components of six kinds were installed.
        import inspect

        source = inspect.getsource(toolchain.installed_project_components) + inspect.getsource(toolchain._settings_components)
        for expected in (
            "agents",
            "commands",
            "output-styles",
            ".mcp.json",
            "statusLine",
            "hook-command",
            "unrecognised",
        ):
            with self.subTest(kind=expected):
                self.assertIn(expected, source)
        # settings.local.json is read too: an MCP server declared there was invisible.
        self.assertIn("settings.local.json", toolchain.SETTINGS_FILES)


class ReportManagedMembersTests(unittest.TestCase):
    """`report` and `check` must agree on what is managed.

    They did not: `check` counted a component's declared `members`, `report` subtracted only
    top-level ids, so all eight approved skills in the pack were listed as "decide or remove" -
    in the one report the owner reads before work starts. A report that cries wolf on its own
    repository is a report nobody finishes reading (E17-S11).
    """

    MANIFEST: ClassVar[dict] = {
        "schema_version": 1,
        "context_budget_tokens": 6000,
        "review_cadence_days": 90,
        "components": [
            {
                "id": "pack",
                "kind": "skill",
                "scope": "project",
                "source": ".claude/skills",
                "version": "tracked-in-repo",
                "trust": "FirstParty",
                "capabilities": ["prompt"],
                "always_on_tokens": 100,
                "purpose": "x",
                "decision": {"status": "approved", "by": "owner", "date": "2026-09-01", "rationale": "y"},
                "members": ["skill:.claude/skills/one", "skill:.claude/skills/two"],
            }
        ],
    }

    def test_an_approved_member_is_not_reported_unmanaged(self):
        installed = {
            "skill:.claude/skills/one": ".claude/skills/one",
            "skill:.claude/skills/two": ".claude/skills/two",
        }
        text = toolchain.render_report(self.MANIFEST, installed, dt.date(2026, 9, 13))
        self.assertNotIn("Installed but unmanaged", text)

    def test_an_undeclared_member_is_still_reported(self):
        installed = {
            "skill:.claude/skills/one": ".claude/skills/one",
            "skill:.claude/skills/three": ".claude/skills/three",
        }
        text = toolchain.render_report(self.MANIFEST, installed, dt.date(2026, 9, 13))
        self.assertIn("Installed but unmanaged", text)
        self.assertIn("skill:.claude/skills/three", text)
        self.assertNotIn("skill:.claude/skills/one at", text)

    def test_report_and_check_agree_on_the_real_repository(self):
        # The property the defect broke, asserted against the committed manifest rather than a
        # fixture: check passes, so report must find nothing unmanaged.
        self.assertEqual(0, toolchain.main(["check"]))
        data = toolchain.load_manifest()
        text = toolchain.render_report(data, toolchain.installed_project_components(), dt.date.today())
        self.assertNotIn("Installed but unmanaged", text)


class DeclaredCostAgainstMeasurement(unittest.TestCase):
    """The budget decided against Task Observer; until E28-S04 it summed numbers a person typed."""

    def test_an_inflated_declaration_refuses(self) -> None:
        pack = component(id="pack", scope="project", members=["skill:.claude/skills/orient"], always_on_tokens=9000)
        errors, _ = toolchain.token_errors([pack])
        self.assertEqual(len(errors), 1)
        self.assertIn("9000", errors[0])

    def test_the_real_declaration_is_inside_the_tolerance(self) -> None:
        """Answers the question the story asked: was hand-entry reliable? For this component, yes."""
        data = toolchain.load_manifest()
        errors, _ = toolchain.token_errors(data["components"])
        self.assertEqual(errors, [])

    def test_the_tolerance_is_tested_from_both_sides(self) -> None:
        pack = component(id="pack", scope="project", members=["skill:.claude/skills/orient"])
        measured = toolchain.measured_tokens(pack)
        assert measured is not None
        inside = round(measured * (1 + toolchain.TOKEN_TOLERANCE * 0.5))
        outside = round(measured * (1 + toolchain.TOKEN_TOLERANCE * 2))
        self.assertEqual(toolchain.token_errors([{**pack, "always_on_tokens": inside}])[0], [])
        self.assertEqual(len(toolchain.token_errors([{**pack, "always_on_tokens": outside}])[0]), 1)

    def test_an_unmeasurable_component_is_noted_not_confirmed(self) -> None:
        plugin = component(id="remote", scope="user", always_on_tokens=500)
        errors, notes = toolchain.token_errors([plugin])
        self.assertEqual(errors, [])
        self.assertEqual(len(notes), 1)
        self.assertIn("unmeasured", notes[0])

    def test_a_member_that_is_not_a_skill_path_makes_the_component_unmeasured(self) -> None:
        """The first version read members as bare names, measured nothing, and said so silently."""
        pack = component(id="pack", scope="project", members=["orient"])
        self.assertIsNone(toolchain.measured_tokens(pack))

    def test_a_project_scope_prompt_component_that_cannot_be_measured_refuses(self) -> None:
        """Only an absent user-scope component is honestly unmeasured; this one is carried here."""
        pack = component(id="pack", scope="project", members=["orient"])
        errors, notes = toolchain.token_errors([pack])
        self.assertEqual(notes, [])
        self.assertEqual(len(errors), 1)
        self.assertIn("cannot be measured", errors[0])


class ComponentLicences(unittest.TestCase):
    ALLOWED: ClassVar[dict] = {"license_allowlist": ["MIT", "Apache-2.0", "CC-BY-SA-4.0"]}

    def test_an_unlicensed_source_refuses_as_its_own_distinct_state(self) -> None:
        """Three of the census's most popular candidates carried no licence file at all."""
        errors = toolchain.license_errors(self.ALLOWED, [component(license=toolchain.UNLICENSED)])
        self.assertEqual(len(errors), 1)
        self.assertIn("all rights", errors[0])

    def test_a_licence_outside_the_allow_list_refuses(self) -> None:
        """GPL-3.0 is the real candidate the census rejected on this ground."""
        errors = toolchain.license_errors(self.ALLOWED, [component(license="GPL-3.0")])
        self.assertEqual(len(errors), 1)
        self.assertIn("GPL-3.0", errors[0])

    def test_an_absent_allow_list_refuses_rather_than_admitting_everything(self) -> None:
        errors = toolchain.license_errors({}, [component(license="MIT")])
        self.assertEqual(len(errors), 1)

    def test_the_committed_manifest_passes_its_own_allow_list(self) -> None:
        data = toolchain.load_manifest()
        self.assertEqual(toolchain.license_errors(data, data["components"]), [])

    def test_every_component_records_a_licence(self) -> None:
        for entry in toolchain.load_manifest()["components"]:
            self.assertIn("license", entry, f"{entry['id']} records no licence")

    def test_the_allow_list_is_not_a_copy_of_the_crate_allow_list(self) -> None:
        """Two lists that must agree eventually do not; these govern different artifacts."""
        data = toolchain.load_manifest()
        self.assertIn("CC-BY-SA-4.0", data["license_allowlist"], "the pack carried under it must be expressible")


class LocalMeasurementsForWhatCiCannotSee(unittest.TestCase):
    """Ten of twelve components are user-scope, so most of the budget rested on hand entry (E29-S05)."""

    def test_a_measurement_bound_to_the_current_version_counts(self) -> None:
        entry = component(scope="user", version="1.0.0", measured={"tokens": 250, "component_version": "1.0.0", "taken_on": "2026-09-15"})
        tokens, staleness = toolchain.recorded_measurement(entry)
        self.assertEqual(tokens, 250)
        self.assertIsNone(staleness)

    def test_a_measurement_taken_against_another_version_is_stale_not_current(self) -> None:
        entry = component(scope="user", version="2.0.0", measured={"tokens": 250, "component_version": "1.0.0", "taken_on": "2026-09-15"})
        tokens, staleness = toolchain.recorded_measurement(entry)
        self.assertIsNone(tokens, "a stale measurement must not be read as a current one")
        assert staleness is not None
        self.assertIn("stale", staleness)

    def test_a_measurement_without_a_taken_on_date_is_unmeasured(self) -> None:
        """A version alone cannot attest to when the local observation was made."""
        entry = component(scope="user", version="1.0.0", measured={"tokens": 250, "component_version": "1.0.0"})
        tokens, staleness = toolchain.recorded_measurement(entry)
        self.assertIsNone(tokens)
        assert staleness is not None
        self.assertIn("no taken_on date", staleness)

    def test_a_measurement_with_an_invalid_taken_on_date_is_unmeasured(self) -> None:
        entry = component(
            scope="user",
            version="1.0.0",
            measured={"tokens": 250, "component_version": "1.0.0", "taken_on": "not-a-date"},
        )
        tokens, staleness = toolchain.recorded_measurement(entry)
        self.assertIsNone(tokens)
        assert staleness is not None
        self.assertIn("ISO-8601", staleness)

    def test_no_measurement_stays_unmeasured_rather_than_zero(self) -> None:
        """The absence of a measurement is not a measurement of zero."""
        tokens, staleness = toolchain.recorded_measurement(component(scope="user"))
        self.assertIsNone(tokens)
        self.assertIsNone(staleness)

    def test_a_stale_measurement_warns_and_does_not_fail(self) -> None:
        entry = component(scope="user", version="2.0.0", always_on_tokens=10, measured={"tokens": 999, "component_version": "1.0.0"})
        errors, notes = toolchain.token_errors([entry])
        self.assertEqual(errors, [], "staleness is reported, not refused")
        self.assertTrue(any("stale" in note for note in notes))

    def test_ci_passes_with_no_local_measurements_at_all(self) -> None:
        data = toolchain.load_manifest()
        stripped = [{k: v for k, v in c.items() if k != "measured"} for c in data["components"]]
        errors, _ = toolchain.token_errors(stripped)
        self.assertEqual(errors, [])


class LicenceEvidenceIsBoundToARevision(unittest.TestCase):
    """A licence is checked at the recorded revision and against the live upstream (E29-S06)."""

    def test_a_licence_without_a_source_revision_is_reported(self) -> None:
        notes = toolchain.license_evidence_notes([component(license="MIT")])
        self.assertEqual(len(notes), 1)
        self.assertIn("relicensed", notes[0])

    def test_a_licence_bound_to_a_revision_is_not_reported(self) -> None:
        entry = component(license="MIT", license_source_revision="github:owner/name@abc123")
        self.assertEqual(toolchain.license_evidence_notes([entry]), [])

    def test_every_committed_component_names_the_revision_it_was_read_from(self) -> None:
        self.assertEqual(toolchain.license_evidence_notes(toolchain.load_manifest()["components"]), [])

    def test_the_report_is_a_warning_and_never_an_error(self) -> None:
        """Binding the evidence is the mechanism; refusing on its absence is a later decision."""
        data = toolchain.load_manifest()
        stripped = [{k: v for k, v in c.items() if k != "license_source_revision"} for c in data["components"]]
        self.assertEqual(toolchain.license_errors(data, stripped), [])

    def test_a_relicensed_upstream_is_reported(self) -> None:
        entry = component(license="MIT", license_source_revision="github:owner/name@abc123")

        def lookup(_owner: str, _name: str, ref: str | None) -> str | None:
            return "MIT" if ref == "abc123" else "Apache-2.0"

        result = toolchain.license_comparison(entry, lookup)
        self.assertIn("upstream now declares 'Apache-2.0'", result)
        self.assertIn("relicensed", result)

    def test_a_disagreement_at_the_recorded_revision_names_both_values(self) -> None:
        entry = component(license="MIT", license_source_revision="github:owner/name@abc123")
        result = toolchain.license_comparison(entry, lambda *_args: "Apache-2.0")
        self.assertIn("recorded licence 'MIT'", result)
        self.assertIn("declares 'Apache-2.0'", result)

    def test_no_network_evidence_is_not_reported_as_agreement(self) -> None:
        entry = component(license="MIT", license_source_revision="github:owner/name@abc123")
        result = toolchain.license_comparison(entry, lambda *_args: None)
        self.assertIn("could not compare", result)


if __name__ == "__main__":
    unittest.main()


class OpenCodeComponentsAreEnumerated(unittest.TestCase):
    """OpenCode loads agents, plugins, MCP servers, language servers and skills from `.opencode/`
    and `opencode.json` exactly as Claude Code loads them from `.claude/`. A walk that only knew
    `.claude/` would let the second reviewer's runtime carry anything unseen (E34-S04)."""

    def enumerate(self, files):
        import json
        import tempfile
        from pathlib import Path
        from unittest import mock

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for relative, content in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content if isinstance(content, str) else json.dumps(content), encoding="utf-8")
            with mock.patch.object(toolchain, "ROOT", root):
                return toolchain.installed_project_components()

    def test_agents_plugins_and_unknown_entries_are_found(self):
        found = self.enumerate(
            {
                ".opencode/agents/verifier.md": "x",
                ".opencode/plugins/hook.ts": "x",
                ".opencode/mystery/file": "x",
                ".opencode/node_modules/pkg/index.js": "x",
                ".opencode/package.json": "{}",
            }
        )
        self.assertIn("subagent:opencode/verifier", found)
        self.assertIn("plugin:opencode/hook", found)
        self.assertIn("unrecognised:.opencode/mystery", found)
        self.assertFalse(any("node_modules" in key or "package.json" in key for key in found), found)

    def test_config_components_and_default_code_execution_are_found(self):
        found = self.enumerate(
            {"opencode.json": {"mcp": {"srv": {}}, "plugin": ["npm-plugin"], "skills": {"paths": ["elsewhere"], "urls": ["https://x"]}}}
        )
        for key in ("mcp:srv", "plugin:npm-plugin", "unrecognised:skills-path:elsewhere", "unrecognised:skills-url:https://x"):
            self.assertIn(key, found)
        # Language servers and formatters are on unless switched off, and both execute code.
        self.assertIn("lsp:opencode", found)
        self.assertIn("unrecognised:opencode-formatter", found)

    def test_the_committed_configuration_carries_nothing_implicit(self):
        found = self.enumerate({"opencode.json": {"skills": {"paths": [".claude/skills"]}, "lsp": False, "formatter": False}})
        self.assertEqual({}, found)

    def test_unknown_entries_inside_component_directories_are_reported(self):
        # Round 1 (E34-S04): a nested directory under agents/ and a stray file under skills/
        # vanished from the walk instead of being reported as unrecognised.
        import tempfile
        from pathlib import Path
        from unittest import mock

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for relative in (
                ".opencode/agents/known.md",
                ".opencode/agents/unknown/inner.md",
                ".opencode/agents/notes.txt",
                ".opencode/plugins/hook.ts",
                ".opencode/skills/rogue.txt",
                ".opencode/skills/real/SKILL.md",
                ".opencode/skills/no-skill-file/README.md",
                # Round 2: a hidden entry used to be dropped before it was judged.
                ".opencode/agents/.hidden.md",
                ".opencode/skills/.hidden/SKILL.md",
                ".opencode/agents/.DS_Store",
                # Self-review of round 2: a directory named like Finder's file, holding an agent.
                ".opencode/plugins/.DS_Store/sneaky.md",
                ".opencode/skills/.DS_Store/SKILL.md",
            ):
                (root / relative).parent.mkdir(parents=True, exist_ok=True)
                (root / relative).write_text("x", encoding="utf-8")
            (root / ".opencode" / "agents" / "empty").mkdir()
            (root / ".opencode" / "skills" / "empty").mkdir()
            with mock.patch.object(toolchain, "ROOT", root):
                found = toolchain.installed_project_components()
        self.assertIn("subagent:opencode/known", found)
        self.assertIn("plugin:opencode/hook", found)
        self.assertIn("skill:.opencode/skills/real", found)
        for entry in (
            "agents/unknown",
            "agents/empty",
            "agents/notes.txt",
            "skills/rogue.txt",
            "skills/empty",
            "skills/no-skill-file",
            "agents/.hidden.md",
            "skills/.hidden",
            "plugins/.DS_Store",
            "skills/.DS_Store",
        ):
            self.assertIn(f"unrecognised:.opencode/{entry}", found)
        self.assertNotIn("subagent:opencode/inner", found)
        self.assertNotIn("unrecognised:.opencode/agents/.DS_Store", found)
