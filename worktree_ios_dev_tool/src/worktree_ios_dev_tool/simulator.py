"""xcrun simctl wrappers + interactive picker for `boot`.

Only this module and proc.py touch simctl.
"""
from __future__ import annotations

import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path

from . import ui
from .config import SimulatorEntry
from .errors import EnvError, UserError
from .proc import require, run, run_json


@dataclass(frozen=True)
class DeviceType:
    identifier: str
    name: str


@dataclass(frozen=True)
class Runtime:
    identifier: str
    name: str
    version: str


_IPHONE17_RE = re.compile(r"iPhone 17(\b| )")
_LABEL_RE = re.compile(r"^[A-Za-z0-9_]+$")


def validate_label(label: str) -> None:
    """Reject labels that would break the reverse-parser.

    Labels must be non-empty and match ``[A-Za-z0-9_]+``. Hyphens are
    explicitly disallowed because the reverse-parser relies on the *last*
    hyphen to split ``<basename>-<label>``; a label containing a hyphen
    would corrupt that split when the basename also has hyphens.
    """
    if not label or not _LABEL_RE.match(label):
        raise UserError(
            f"Invalid simulator label `{label}`. Use alphanumerics and underscores only."
        )


def synth_managed_name(prefix: str, worktree_basename: str, label: str) -> str:
    """Build a simctl device name in the canonical managed format.

    Format: ``<simulator_prefix>-<worktree_basename>-<label>``. The prefix
    and basename may contain hyphens; only the trailing ``-<label>`` segment
    is reverse-parsed by :func:`parse_managed_name`.
    """
    return f"{prefix}-{worktree_basename}-{label}"


def parse_managed_name(name: str, *, prefix: str) -> tuple[str, str] | None:
    """Reverse the :func:`synth_managed_name` format.

    Returns ``(worktree_basename, label)`` if *name* matches the managed
    format under *prefix*, else ``None`` (so callers can ignore non-managed
    sims). The basename may contain hyphens; the label, by validation, may
    not — so we split on the *last* hyphen only.

    Empty basename ("Pulse-default" with prefix="Pulse" — no separator
    between basename and label) returns None: ambiguous, leave alone.
    """
    if not name.startswith(prefix + "-"):
        return None
    tail = name[len(prefix) + 1 :]
    if "-" not in tail:
        return None
    basename, _, label = tail.rpartition("-")
    if not basename or not label:
        return None
    return basename, label


def ensure_tooling() -> None:
    require("xcrun")


def list_device_types(*, iphone_17_only: bool = True) -> list[DeviceType]:
    ensure_tooling()
    data = json.loads(run_json(["xcrun", "simctl", "list", "devicetypes", "--json"], verbose=True))
    types = [
        DeviceType(identifier=t["identifier"], name=t["name"])
        for t in data.get("devicetypes", [])
        if t.get("productFamily") == "iPhone"
    ]
    if iphone_17_only:
        types = [t for t in types if _IPHONE17_RE.search(t.name)]
    types.sort(key=lambda t: t.name)
    return types


def list_runtimes() -> list[Runtime]:
    ensure_tooling()
    data = json.loads(run_json(["xcrun", "simctl", "list", "runtimes", "--json"], verbose=True))
    out: list[Runtime] = []
    for r in data.get("runtimes", []):
        if not r.get("isAvailable", False):
            continue
        if r.get("platform", "iOS") != "iOS":
            continue
        out.append(Runtime(identifier=r["identifier"], name=r["name"], version=r["version"]))
    out.sort(key=lambda r: [int(p) for p in r.version.split(".") if p.isdigit()], reverse=True)
    return out


def find_device_by_name(name: str) -> dict | None:
    """Return the first device dict matching name, or None."""
    data = json.loads(run_json(["xcrun", "simctl", "list", "devices", "--json"], verbose=True))
    for _, devices in data.get("devices", {}).items():
        for dev in devices:
            if dev.get("name") == name:
                return dev
    return None


def find_device_by_udid(udid: str) -> dict | None:
    data = json.loads(run_json(["xcrun", "simctl", "list", "devices", "--json"], verbose=True))
    for _, devices in data.get("devices", {}).items():
        for dev in devices:
            if dev.get("udid") == udid:
                return dev
    return None


def create(name: str, device_type: DeviceType, runtime: Runtime) -> str:
    """Create a simulator and return the new UDID."""
    out = run_json(["xcrun", "simctl", "create", name, device_type.identifier, runtime.identifier], verbose=True)
    return out.strip()


def boot(udid: str) -> None:
    """Boot the simulator. No-ops if already booted."""
    dev = find_device_by_udid(udid)
    if dev is None:
        raise UserError(f"No simulator with UDID {udid}.")
    if dev.get("state") == "Booted":
        ui.step(f"Simulator already booted ({udid}).")
        return
    ui.step("Booting simulator…")
    run(["xcrun", "simctl", "boot", udid], verbose=True)
    run(["open", "-a", "Simulator"])


def delete(udid: str) -> None:
    run(["xcrun", "simctl", "delete", udid], verbose=True)


def _is_interactive() -> bool:
    return sys.stdin.isatty() and sys.stdout.isatty()


def _pick_fuzzy(prompt: str, options: list[str]) -> str:
    """Fuzzy picker using InquirerPy. Falls back to numbered list when InquirerPy
    cannot render (e.g. TERM=dumb). Raises EnvError if not on a TTY."""
    if not _is_interactive():
        raise EnvError(
            "`worktree-ios-dev-tool boot` first-run picker needs a real terminal. "
            "Re-run from an interactive shell."
        )
    try:
        from InquirerPy import inquirer
        return inquirer.fuzzy(message=prompt, choices=options).execute()
    except (ImportError, OSError, EOFError):
        # Fallback: numbered list when InquirerPy unavailable or terminal can't render
        print(prompt)
        for i, opt in enumerate(options, start=1):
            print(f"  {i}. {opt}")
        while True:
            raw = input("Choice: ").strip()
            if raw.isdigit():
                idx = int(raw)
                if 1 <= idx <= len(options):
                    return options[idx - 1]
            print(f"Enter a number 1..{len(options)}.")


def pick_device_and_runtime(
    *, iphone_17_only: bool, device_name: str | None = None, runtime_name: str | None = None,
) -> tuple[DeviceType, Runtime]:
    devices = list_device_types(iphone_17_only=iphone_17_only)
    if not devices:
        raise EnvError(
            "No matching iPhone device types found. "
            "Try `worktree-ios-dev-tool sim pick --all-devices`."
        )
    runtimes = list_runtimes()
    if not runtimes:
        raise EnvError("No iOS runtimes installed. Install via Xcode > Settings > Platforms.")

    if device_name:
        matches = [d for d in devices if d.name == device_name]
        if not matches:
            raise UserError(
                f"Device type `{device_name}` not found. "
                f"Available: {', '.join(d.name for d in devices)}"
            )
        device = matches[0]
    elif _is_interactive():
        device_name = _pick_fuzzy("Select device type:", [d.name for d in devices])
        device = next(d for d in devices if d.name == device_name)
    else:
        device = devices[0]
        ui.info(f"Auto-selected device: {device.name}")

    if runtime_name:
        matches = [r for r in runtimes if r.name == runtime_name]
        if not matches:
            raise UserError(
                f"Runtime `{runtime_name}` not found. "
                f"Available: {', '.join(r.name for r in runtimes)}"
            )
        runtime = matches[0]
    elif _is_interactive():
        runtime_name = _pick_fuzzy("Select runtime:", [r.name for r in runtimes])
        runtime = next(r for r in runtimes if r.name == runtime_name)
    else:
        runtime = runtimes[0]
        ui.info(f"Auto-selected runtime: {runtime.name}")

    return device, runtime


def to_config(name: str, udid: str, device: DeviceType, runtime: Runtime) -> SimulatorEntry:
    return SimulatorEntry(name=name, udid=udid, device=device.name, runtime=runtime.name)


def list_all_devices() -> list[dict]:
    """Return a flat list of device dicts from ``simctl list devices --json``.

    Each dict is the verbatim simctl entry plus an ``_runtime`` key copying
    the runtime identifier so callers can filter / display without re-parsing.
    """
    ensure_tooling()
    data = json.loads(run_json(["xcrun", "simctl", "list", "devices", "--json"], verbose=True))
    out: list[dict] = []
    for runtime_id, devices in data.get("devices", {}).items():
        for dev in devices:
            d = dict(dev)
            d["_runtime"] = runtime_id
            out.append(d)
    return out


def list_devices_by_prefix(prefix: str) -> list[dict]:
    """Return all devices whose ``name`` starts with ``<prefix>-``."""
    pattern = prefix + "-"
    return [d for d in list_all_devices() if d.get("name", "").startswith(pattern)]


def shutdown(udid: str) -> None:
    """Shutdown the device. No-op if it isn't currently booted."""
    dev = find_device_by_udid(udid)
    if dev is None:
        return
    if dev.get("state") != "Booted":
        return
    run(["xcrun", "simctl", "shutdown", udid], verbose=True)


def device_data_dir(udid: str) -> Path:
    """Return the macOS path where simctl stores a device's disk image and state."""
    return Path.home() / "Library" / "Developer" / "CoreSimulator" / "Devices" / udid


def du_bytes(path: Path) -> int:
    """Return the recursive on-disk size of *path* in bytes.

    Returns 0 for missing paths so callers can show "0 B" rather than crash.
    Uses ``os.walk`` to avoid the cost of forking ``du(1)`` per device.
    """
    import os
    if not path.exists():
        return 0
    total = 0
    for root, _, files in os.walk(path, followlinks=False):
        for f in files:
            try:
                total += (Path(root) / f).stat().st_size
            except (FileNotFoundError, PermissionError):
                # Devices can churn while we walk; ignore transient races.
                continue
    return total
