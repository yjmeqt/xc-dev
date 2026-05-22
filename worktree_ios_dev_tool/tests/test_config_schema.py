"""Tests for project.toml + simulator.toml schema parsing and version checks."""
from __future__ import annotations

import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

_SRC = Path(__file__).resolve().parent.parent / "src"
if str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))

from worktree_ios_dev_tool.config import load  # noqa: E402
from worktree_ios_dev_tool.errors import UserError  # noqa: E402


def _seed(tmp: Path, project: str, simulator: str | None = None) -> Path:
    """Seed a fake worktree with project.toml (and optional simulator.toml)."""
    cfg_dir = tmp / "worktree-ios-dev"
    cfg_dir.mkdir()
    (tmp / "ios").mkdir()
    (tmp / "ios" / "Pulse.xcodeproj").mkdir()
    project_toml = cfg_dir / "project.toml"
    project_toml.write_text(textwrap.dedent(project))
    if simulator is not None:
        (cfg_dir / "simulator.toml").write_text(textwrap.dedent(simulator))
    return project_toml


class ProjectVersionTests(unittest.TestCase):
    def test_loads_v1(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                schema_version = 1
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "Pulse"
                configuration = "Debug"
                simulator_prefix = "Pulse"
            """)
            cfg = load(path)
            self.assertEqual(cfg.project.scheme, "Pulse")
            self.assertEqual(cfg.simulators, {})

    def test_missing_version_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "Pulse"
                configuration = "Debug"
                simulator_prefix = "Pulse"
            """)
            with self.assertRaises(UserError) as cm:
                load(path)
            self.assertIn("schema_version", str(cm.exception))
            self.assertIn("proj init", str(cm.exception))

    def test_unsupported_version_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                schema_version = 99
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "Pulse"
                configuration = "Debug"
                simulator_prefix = "Pulse"
            """)
            with self.assertRaises(UserError) as cm:
                load(path)
            self.assertIn("99", str(cm.exception))


class SimulatorTomlTests(unittest.TestCase):
    def _common_project(self) -> str:
        return """
            schema_version = 1
            [project]
            path = "ios/Pulse.xcodeproj"
            scheme = "Pulse"
            configuration = "Debug"
            simulator_prefix = "Pulse"
        """

    def test_loads_two_simulators(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, self._common_project(), simulator="""
                schema_version = 1

                [simulators.default]
                name = "Pulse-feat-default"
                udid = "AAAA"
                device = "iPhone 17 Pro"
                runtime = "iOS 18.2"

                [simulators.peer]
                name = "Pulse-feat-peer"
                udid = "BBBB"
                device = "iPhone 17 Pro"
                runtime = "iOS 18.2"
            """)
            cfg = load(path)
            self.assertEqual(set(cfg.simulators), {"default", "peer"})
            self.assertEqual(cfg.simulators["peer"].udid, "BBBB")

    def test_simulator_missing_version_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, self._common_project(), simulator="""
                [simulators.default]
                name = "Pulse-feat-default"
                udid = "AAAA"
                device = "iPhone 17 Pro"
                runtime = "iOS 18.2"
            """)
            with self.assertRaises(UserError) as cm:
                load(path)
            self.assertIn("schema_version", str(cm.exception))
            self.assertIn("sim pick", str(cm.exception))


class AppMetadataTests(unittest.TestCase):
    """schema_version 2 carries optional resolved-from-xcodebuild metadata."""

    def test_loads_v2_with_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                schema_version = 2
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "Pulse"
                configuration = "Debug"
                simulator_prefix = "Pulse"
                bundle_id = "com.example.pulse"
                product_name = "Pulse"
                display_name = "Pulse"
                min_ios_version = "17.0"
            """)
            cfg = load(path)
            self.assertEqual(cfg.project.bundle_id, "com.example.pulse")
            self.assertEqual(cfg.project.product_name, "Pulse")
            self.assertEqual(cfg.project.display_name, "Pulse")
            self.assertEqual(cfg.project.min_ios_version, "17.0")

    def test_v2_metadata_is_optional(self) -> None:
        # framework / CLI schemes legitimately won't have a bundle_id.
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                schema_version = 2
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "PulseKit"
                configuration = "Debug"
            """)
            cfg = load(path)
            self.assertIsNone(cfg.project.bundle_id)
            self.assertIsNone(cfg.project.product_name)

    def test_v1_legacy_still_loads(self) -> None:
        # Existing worktrees that haven't been re-init'd keep working;
        # the new fields just stay None.
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                schema_version = 1
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "Pulse"
                configuration = "Debug"
                simulator_prefix = "Pulse"
            """)
            cfg = load(path)
            self.assertEqual(cfg.project.scheme, "Pulse")
            self.assertIsNone(cfg.project.bundle_id)

    def test_unknown_key_in_project_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            path = _seed(tmp, """
                schema_version = 2
                [project]
                path = "ios/Pulse.xcodeproj"
                scheme = "Pulse"
                configuration = "Debug"
                made_up_key = "boom"
            """)
            with self.assertRaises(UserError) as cm:
                load(path)
            self.assertIn("made_up_key", str(cm.exception))


if __name__ == "__main__":
    unittest.main()
