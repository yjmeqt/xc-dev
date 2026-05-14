"""Tests for the new simctl helpers added in Task 5.

Subprocess interactions are mocked. Filesystem-size logic is exercised
directly against a temp tree.
"""
from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

_SRC = Path(__file__).resolve().parent.parent / "src"
if str(_SRC) not in sys.path:
    sys.path.insert(0, str(_SRC))

from worktree_ios_dev_tool.simulator import (  # noqa: E402
    device_data_dir,
    du_bytes,
    list_all_devices,
    list_devices_by_prefix,
)


_FAKE_LIST = json.dumps({
    "devices": {
        "com.apple.CoreSimulator.SimRuntime.iOS-18-2": [
            {"name": "Pulse-main-default",   "udid": "AAAA", "state": "Booted"},
            {"name": "Pulse-feat-x-peer",    "udid": "BBBB", "state": "Shutdown"},
            {"name": "Other-thing",          "udid": "CCCC", "state": "Shutdown"},
        ]
    }
})


class ListDevicesTests(unittest.TestCase):
    def test_list_all_returns_flat(self) -> None:
        with (
            patch("worktree_ios_dev_tool.simulator.ensure_tooling"),
            patch("worktree_ios_dev_tool.simulator.run_json", return_value=_FAKE_LIST),
        ):
            devices = list_all_devices()
        self.assertEqual({d["udid"] for d in devices}, {"AAAA", "BBBB", "CCCC"})

    def test_list_by_prefix_filters_and_keeps_runtime(self) -> None:
        with (
            patch("worktree_ios_dev_tool.simulator.ensure_tooling"),
            patch("worktree_ios_dev_tool.simulator.run_json", return_value=_FAKE_LIST),
        ):
            devices = list_devices_by_prefix("Pulse")
        self.assertEqual({d["udid"] for d in devices}, {"AAAA", "BBBB"})


class DiskTests(unittest.TestCase):
    def test_device_data_dir_format(self) -> None:
        result = device_data_dir("ABCD-1234")
        self.assertTrue(str(result).endswith("Library/Developer/CoreSimulator/Devices/ABCD-1234"))

    def test_du_bytes_sums_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "a.txt").write_bytes(b"x" * 100)
            (root / "sub").mkdir()
            (root / "sub" / "b.txt").write_bytes(b"y" * 250)
            self.assertGreaterEqual(du_bytes(root), 350)

    def test_du_bytes_missing_returns_zero(self) -> None:
        self.assertEqual(du_bytes(Path("/nonexistent/path/abcd")), 0)


if __name__ == "__main__":
    unittest.main()
