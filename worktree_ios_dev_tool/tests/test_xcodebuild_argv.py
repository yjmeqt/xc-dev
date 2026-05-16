"""Tests for xcodebuild argv builders — target flag selection."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

_SRC = Path(__file__).resolve().parent.parent / "src"
if str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))

from worktree_ios_dev_tool.config import (  # noqa: E402
    Config,
    ProjectConfig,
    SimulatorEntry,
)
from worktree_ios_dev_tool.xcodebuild import (  # noqa: E402
    build_argv,
    clean_argv,
)


def _cfg(target: Path) -> Config:
    return Config(
        config_path=Path("/tmp/x/worktree-ios-dev/project.toml"),
        worktree_root=Path("/tmp/x"),
        derived_data=Path("/tmp/x/worktree-ios-dev/derivedData"),
        project=ProjectConfig(
            path=target,
            scheme="App",
            configuration="Debug",
            simulator_prefix="App",
        ),
        simulators={
            "default": SimulatorEntry(
                name="App-x-default", udid="UDID", device="iPhone 17 Pro", runtime="iOS 18.2",
            ),
        },
        packages_root=Path("/tmp/x/ios/Packages"),
    )


class TargetFlagSelection(unittest.TestCase):
    def test_xcodeproj_uses_project_flag(self) -> None:
        argv = build_argv(_cfg(Path("/tmp/x/ios/App.xcodeproj")))
        self.assertIn("-project", argv)
        self.assertNotIn("-workspace", argv)
        self.assertEqual(argv[argv.index("-project") + 1], "/tmp/x/ios/App.xcodeproj")

    def test_xcworkspace_uses_workspace_flag(self) -> None:
        argv = build_argv(_cfg(Path("/tmp/x/ios/App.xcworkspace")))
        self.assertIn("-workspace", argv)
        self.assertNotIn("-project", argv)
        self.assertEqual(argv[argv.index("-workspace") + 1], "/tmp/x/ios/App.xcworkspace")

    def test_clean_respects_workspace(self) -> None:
        argv = clean_argv(_cfg(Path("/tmp/x/ios/App.xcworkspace")))
        self.assertIn("-workspace", argv)
        self.assertNotIn("-project", argv)


if __name__ == "__main__":
    unittest.main()
