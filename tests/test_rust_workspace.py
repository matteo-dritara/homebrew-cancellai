from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest import mock

from scripts import check_rust_workspace as rw


def write_crate(base: Path, name: str, deps: list[str]) -> None:
    dep_lines = "\n".join(f'{dep} = {{ path = "../{dep}" }}' for dep in deps)
    (base / name).mkdir(parents=True)
    (base / name / "Cargo.toml").write_text(
        f"""[package]
name = "{name}"
version = "0.1.0"

[dependencies]
{dep_lines}
""",
        encoding="utf-8",
    )


def write_target_doc(path: Path, crate_names: list[str]) -> None:
    entries = "\n".join(f"  {name}/            # test crate" for name in crate_names)
    path.write_text(
        f"""# Architecture: Target

## Target Rust workspace

Names may be refined through ADRs, but dependency direction is normative.

```text
crates/
{entries}
```
""",
        encoding="utf-8",
    )


class RustWorkspaceTests(unittest.TestCase):
    def test_real_workspace_is_valid(self):
        self.assertEqual([], rw.validate())

    def test_documented_crates_matches_real_target_doc(self):
        names = rw.documented_crates()
        self.assertIn("cancellai-model", names)
        self.assertIn("cancellai-safety", names)
        self.assertEqual(len(names), len(set(names)))

    def test_checker_detects_a_dependency_cycle(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-a", ["cancellai-b"])
            write_crate(crates_dir, "cancellai-b", ["cancellai-a"])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-a", "cancellai-b"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("dependency cycle" in e for e in errors), errors)

    def test_checker_detects_model_depending_on_a_provider_crate(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", ["cancellai-provider-claude"])
            write_crate(crates_dir, "cancellai-provider-claude", [])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model", "cancellai-provider-claude"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("cancellai-model" in e and "forbidden dependency" in e for e in errors), errors)

    def test_checker_allows_safety_depending_on_model(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", [])
            write_crate(crates_dir, "cancellai-safety", ["cancellai-model"])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model", "cancellai-safety"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertEqual([], errors)

    def test_checker_detects_an_undocumented_crate_on_disk(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", [])
            write_crate(crates_dir, "cancellai-mystery", [])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("cancellai-mystery" in e and "not documented" in e for e in errors), errors)

    def test_checker_detects_a_documented_crate_missing_on_disk(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            write_crate(crates_dir, "cancellai-model", [])
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model", "cancellai-ghost"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("cancellai-ghost" in e and "missing under" in e for e in errors), errors)

    def test_checker_detects_a_package_name_directory_mismatch(self):
        with tempfile.TemporaryDirectory() as td:
            crates_dir = Path(td) / "crates"
            (crates_dir / "cancellai-model").mkdir(parents=True)
            (crates_dir / "cancellai-model" / "Cargo.toml").write_text(
                '[package]\nname = "cancellai-wrong-name"\n\n[dependencies]\n', encoding="utf-8"
            )
            target_doc = Path(td) / "TARGET.md"
            write_target_doc(target_doc, ["cancellai-model"])

            with mock.patch.object(rw, "RUST_CRATES_DIR", crates_dir), mock.patch.object(rw, "TARGET_DOC", target_doc):
                errors = rw.validate()
        self.assertTrue(any("does not match its directory" in e for e in errors), errors)


if __name__ == "__main__":
    unittest.main()
