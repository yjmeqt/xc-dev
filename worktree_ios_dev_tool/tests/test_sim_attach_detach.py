"""Tests for `sim attach`, `sim detach`, and the `--no-window` flag on
`boot` / `run`. We mock all subprocess interaction so these run anywhere
(no Simulator.app required)."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

_SRC = Path(__file__).resolve().parent.parent / "src"
if str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))

from worktree_ios_dev_tool import simulator as sim_mod  # noqa: E402


class AttachGuiTests(unittest.TestCase):
    def test_with_udid_passes_arg(self) -> None:
        with patch("worktree_ios_dev_tool.simulator.run") as mock_run:
            sim_mod.attach_gui("UDID-123")
        argv = mock_run.call_args.args[0]
        self.assertEqual(argv[:2], ["open", "-a"])
        self.assertIn("Simulator", argv)
        self.assertIn("--args", argv)
        self.assertIn("-CurrentDeviceUDID", argv)
        self.assertIn("UDID-123", argv)

    def test_without_udid_omits_args(self) -> None:
        with patch("worktree_ios_dev_tool.simulator.run") as mock_run:
            sim_mod.attach_gui(None)
        argv = mock_run.call_args.args[0]
        self.assertEqual(argv, ["open", "-a", "Simulator"])
        self.assertNotIn("--args", argv)


class DetachGuiTests(unittest.TestCase):
    def test_running_returns_true(self) -> None:
        mock_result = Mock()
        mock_result.returncode = 0
        with patch("subprocess.run", return_value=mock_result) as run:
            assert sim_mod.detach_gui() is True
        # Make sure we use pkill -x for exact-name matching (so we don't
        # accidentally kill another process called e.g. "SimulatorTrampoline").
        argv = run.call_args.args[0]
        self.assertEqual(argv[:3], ["pkill", "-x", "Simulator"])

    def test_not_running_returns_false(self) -> None:
        mock_result = Mock()
        mock_result.returncode = 1  # pkill: no matching processes
        with patch("subprocess.run", return_value=mock_result):
            assert sim_mod.detach_gui() is False


class BootOpenGuiTests(unittest.TestCase):
    def _patches(self, state: str = "Shutdown"):
        return [
            patch(
                "worktree_ios_dev_tool.simulator.find_device_by_udid",
                return_value={"state": state, "udid": "UDID-123", "name": "X"},
            ),
            patch("worktree_ios_dev_tool.simulator.ui"),
            patch("worktree_ios_dev_tool.simulator.run"),
        ]

    def test_default_opens_simulator_app(self) -> None:
        with self._patches()[0], self._patches()[1], patch(
            "worktree_ios_dev_tool.simulator.run"
        ) as run:
            sim_mod.boot("UDID-123")
        argvs = [c.args[0] for c in run.call_args_list]
        # Both `simctl boot` and `open -a Simulator` should run.
        self.assertTrue(any("boot" in a for a in argvs))
        self.assertTrue(any(a == ["open", "-a", "Simulator"] for a in argvs))

    def test_no_window_skips_open(self) -> None:
        with self._patches()[0], self._patches()[1], patch(
            "worktree_ios_dev_tool.simulator.run"
        ) as run:
            sim_mod.boot("UDID-123", open_gui=False)
        argvs = [c.args[0] for c in run.call_args_list]
        self.assertTrue(any("boot" in a for a in argvs))
        self.assertFalse(any(a == ["open", "-a", "Simulator"] for a in argvs))

    def test_already_booted_short_circuits(self) -> None:
        # When state == "Booted", we should neither re-boot nor open the GUI.
        with self._patches(state="Booted")[0], self._patches()[1], patch(
            "worktree_ios_dev_tool.simulator.run"
        ) as run:
            sim_mod.boot("UDID-123")
        self.assertEqual(run.call_count, 0)


if __name__ == "__main__":
    unittest.main()
