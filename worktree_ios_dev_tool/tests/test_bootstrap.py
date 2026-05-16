"""Tests for bootstrap discovery helpers."""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

_SRC = Path(__file__).resolve().parent.parent / "src"
if str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))

from worktree_ios_dev_tool.bootstrap import (  # noqa: E402
    detect_packages_root,
    fetch_schemes,
    find_build_targets,
    find_xcodeprojs,
    find_xcworkspaces,
)


class FindXcodeprojs(unittest.TestCase):
    def test_finds_single_project(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            proj = root / "ios" / "MyApp.xcodeproj"
            proj.mkdir(parents=True)
            self.assertEqual(find_xcodeprojs(root), [proj])

    def test_finds_multiple_projects_sorted(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            a = root / "ios" / "App.xcodeproj"
            b = root / "ios" / "Ext.xcodeproj"
            b.mkdir(parents=True)
            a.mkdir(parents=True)
            result = find_xcodeprojs(root)
            self.assertEqual(result, [a, b])

    def test_empty_when_none(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(find_xcodeprojs(Path(tmp)), [])

    def test_does_not_recurse_inside_xcodeproj(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            outer = root / "ios" / "App.xcodeproj"
            outer.mkdir(parents=True)
            nested = outer / "Nested.xcodeproj"
            nested.mkdir()
            result = find_xcodeprojs(root)
            self.assertIn(outer, result)
            self.assertNotIn(nested, result)


class FindXcworkspacesTests(unittest.TestCase):
    def test_finds_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            ws = root / "ios" / "MyApp.xcworkspace"
            ws.mkdir(parents=True)
            self.assertEqual(find_xcworkspaces(root), [ws])

    def test_skips_workspace_inside_xcodeproj(self) -> None:
        # Xcode places a private project.xcworkspace inside every .xcodeproj.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            proj = root / "ios" / "App.xcodeproj"
            proj.mkdir(parents=True)
            (proj / "project.xcworkspace").mkdir()
            self.assertEqual(find_xcworkspaces(root), [])


class FindBuildTargetsTests(unittest.TestCase):
    def test_returns_both_kinds(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            proj = root / "a" / "Alpha.xcodeproj"
            ws = root / "b" / "Beta.xcworkspace"
            proj.mkdir(parents=True)
            ws.mkdir(parents=True)
            result = find_build_targets(root)
            self.assertIn(proj, result)
            self.assertIn(ws, result)

    def test_workspace_supersedes_sibling_xcodeproj(self) -> None:
        # CocoaPods / Tuist setups have both in the same directory; the
        # workspace is the canonical build target.
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "ios" / "App.xcodeproj").mkdir(parents=True)
            ws = root / "ios" / "App.xcworkspace"
            ws.mkdir()
            self.assertEqual(find_build_targets(root), [ws])


class FetchSchemesTests(unittest.TestCase):
    def test_returns_schemes_from_json(self) -> None:
        mock_output = json.dumps({"project": {"schemes": ["MyApp", "MyAppTests"]}})
        mock_result = Mock()
        mock_result.returncode = 0
        mock_result.stdout = mock_output
        with patch("subprocess.run", return_value=mock_result) as run_mock:
            result = fetch_schemes(Path("ios/MyApp.xcodeproj"))
        self.assertEqual(result, ["MyApp", "MyAppTests"])
        self.assertIn("-project", run_mock.call_args.args[0])

    def test_returns_schemes_from_workspace_json(self) -> None:
        mock_output = json.dumps({"workspace": {"schemes": ["WS-Scheme"]}})
        mock_result = Mock()
        mock_result.returncode = 0
        mock_result.stdout = mock_output
        with patch("subprocess.run", return_value=mock_result) as run_mock:
            result = fetch_schemes(Path("ios/MyApp.xcworkspace"))
        self.assertEqual(result, ["WS-Scheme"])
        self.assertIn("-workspace", run_mock.call_args.args[0])

    def test_returns_empty_on_nonzero_exit(self) -> None:
        mock_result = Mock()
        mock_result.returncode = 1
        mock_result.stdout = ""
        with patch("subprocess.run", return_value=mock_result):
            result = fetch_schemes(Path("ios/MyApp.xcodeproj"))
        self.assertEqual(result, [])

    def test_returns_empty_on_invalid_json(self) -> None:
        mock_result = Mock()
        mock_result.returncode = 0
        mock_result.stdout = "not json"
        with patch("subprocess.run", return_value=mock_result):
            result = fetch_schemes(Path("ios/MyApp.xcodeproj"))
        self.assertEqual(result, [])


class DetectPackagesRootTests(unittest.TestCase):
    def test_detects_sibling_packages_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            xcodeproj = root / "ios" / "MyApp.xcodeproj"
            xcodeproj.mkdir(parents=True)
            packages = root / "ios" / "Packages"
            packages.mkdir()
            result = detect_packages_root(xcodeproj)
            self.assertEqual(result, packages)

    def test_returns_none_when_packages_absent(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            xcodeproj = root / "ios" / "MyApp.xcodeproj"
            xcodeproj.mkdir(parents=True)
            result = detect_packages_root(xcodeproj)
            self.assertIsNone(result)


if __name__ == "__main__":
    unittest.main()
