# worktree_ios_dev_tool/src/worktree_ios_dev_tool/sim.py
"""Implementations of the `sim` namespace verbs.

Each function corresponds to one ``sim <verb>`` CLI invocation. The module
owns the policy layer (single/multi-sim resolution, name synthesis, output
formatting); :mod:`simulator` owns the simctl subprocess wrappers.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from pathlib import Path

from . import simulator as sim_mod, ui
from .config import (
    Config,
    SimulatorEntry,
    load,
    remove_simulator_entry,
    resolve_sim,
    upsert_simulator_entry,
)
from .errors import UserError
from .paths import find_project_toml, simulator_toml_for


# ── shared helpers ───────────────────────────────────────────────────────────

def _load(args: argparse.Namespace) -> tuple[Config, Path]:
    """Resolve project.toml and the matching simulator.toml path."""
    path = args.config.resolve() if args.config else find_project_toml()
    return load(path), simulator_toml_for(path)


def _resolve_label(cfg: Config, label: str | None) -> str:
    """Return the label of the sim a verb without an explicit arg should target.

    Same single/multi rules as :func:`config.resolve_sim`, but returns the
    *label* rather than the entry — useful for verbs like ``shutdown`` that
    operate on the entry by label.
    """
    entry = resolve_sim(cfg, label=label)
    for k, v in cfg.simulators.items():
        if v is entry:
            return k
    raise AssertionError("resolve_sim returned an entry not in cfg.simulators")


# ── verbs ────────────────────────────────────────────────────────────────────

def cmd_pick(args: argparse.Namespace) -> int:
    """Interactively pick + create a simulator and write it to simulator.toml.

    Errors if an entry with that label already exists; the user must call
    ``sim recreate <label>`` to replace one.
    """
    cfg, sim_toml = _load(args)
    label = args.label or "default"
    sim_mod.validate_label(label)

    if label in cfg.simulators:
        raise UserError(
            f"A simulator labeled `{label}` already exists. "
            f"Use `worktree-ios-dev-tool sim recreate {label}` to replace it."
        )

    sim_mod.ensure_tooling()

    device, runtime = sim_mod.pick_device_and_runtime(
        iphone_17_only=not args.all_devices,
        device_name=args.device,
        runtime_name=args.runtime,
    )
    name = sim_mod.synth_managed_name(
        cfg.project.simulator_prefix, cfg.worktree_root.name, label,
    )

    existing = sim_mod.find_device_by_name(name)
    if existing is not None:
        ui.warn(f"Simulator named '{name}' already exists (udid={existing['udid']}).")
        choice = input("Reuse it? [Y/n] ").strip().lower() if ui.is_interactive() else "y"
        if choice in ("", "y", "yes"):
            udid = existing["udid"]
        else:
            raise UserError(
                "Aborted. Delete the existing sim manually or use "
                f"`sim recreate {label}` next time."
            )
    else:
        ui.step(f"Creating {name}…")
        udid = sim_mod.create(name, device, runtime)

    entry = SimulatorEntry(name=name, udid=udid, device=device.name, runtime=runtime.name)
    upsert_simulator_entry(sim_toml, label, entry)
    sim_mod.boot(udid)

    ui.sep()
    ui.done(f"Simulator ready  [{label}]")
    ui.info(f"name    = {entry.name}")
    ui.info(f"udid    = {entry.udid}")
    ui.info(f"device  = {entry.device}")
    ui.info(f"runtime = {entry.runtime}")
    return 0


def cmd_boot(args: argparse.Namespace) -> int:
    """Boot a configured simulator. ``--all`` boots every entry.

    ``--no-window`` boots without opening ``Simulator.app`` — the simulator
    runs entirely headless (consumed by embedded mirrors, CI, log-only
    workflows). If Simulator.app is already running, we *additionally*
    detach it so the booted device doesn't pop a window after the fact.
    """
    cfg, _ = _load(args)
    open_gui = not getattr(args, "no_window", False)
    if args.all:
        if not cfg.simulators:
            raise UserError("No simulators configured. Run `sim pick` first.")
        for label, entry in cfg.simulators.items():
            ui.step(f"Booting {label} ({entry.udid})…")
            sim_mod.boot(entry.udid, open_gui=open_gui)
        if not open_gui and sim_mod.detach_gui():
            ui.step("Detached Simulator.app (booted sims stay running).")
        ui.done(f"Booted {len(cfg.simulators)} simulator(s).")
        return 0

    entry = resolve_sim(cfg, label=args.label)
    sim_mod.boot(entry.udid, open_gui=open_gui)
    if not open_gui and sim_mod.detach_gui():
        ui.step("Detached Simulator.app (booted sim stays running).")
    ui.done(f"Booted {entry.name} ({entry.udid}).")
    return 0


def cmd_attach(args: argparse.Namespace) -> int:
    """Bring up ``Simulator.app`` focused on the named sim (booting it if
    needed). The inverse of ``sim detach``. ``--all`` skips the per-device
    arg and just opens Simulator.app, which shows every booted device.
    """
    cfg, _ = _load(args)
    if getattr(args, "all", False):
        sim_mod.attach_gui(None)
        ui.done("Attached Simulator.app (no specific device pinned).")
        return 0
    entry = resolve_sim(cfg, label=args.label)
    sim_mod.boot(entry.udid, open_gui=False)
    sim_mod.attach_gui(entry.udid)
    ui.done(f"Attached Simulator.app to {entry.name} ({entry.udid}).")
    return 0


def cmd_detach(args: argparse.Namespace) -> int:
    """Terminate ``Simulator.app`` (GUI only). The booted simulators keep
    running — ``simd`` is independent. Idempotent; exits 0 either way.
    """
    _ = args  # no flags; signature kept consistent with other cmd_*
    if sim_mod.detach_gui():
        ui.done("Detached Simulator.app.")
    else:
        ui.done("Simulator.app was not running.")
    return 0


def cmd_shutdown(args: argparse.Namespace) -> int:
    """Shutdown a configured simulator. ``--all`` shuts down every entry."""
    cfg, _ = _load(args)
    if args.all:
        if not cfg.simulators:
            ui.done("No simulators configured.")
            return 0
        for label, entry in cfg.simulators.items():
            ui.step(f"Shutdown {label} ({entry.udid})…")
            sim_mod.shutdown(entry.udid)
        ui.done(f"Shut down {len(cfg.simulators)} simulator(s).")
        return 0

    entry = resolve_sim(cfg, label=args.label)
    sim_mod.shutdown(entry.udid)
    ui.done(f"Shut down {entry.name} ({entry.udid}).")
    return 0


def cmd_list(args: argparse.Namespace) -> int:
    """List configured simulators (default: this worktree; ``--global`` for all).

    Local mode uses ``simulator.toml`` as truth and consults simctl for state.
    Global mode scans simctl for ``<simulator_prefix>-*`` and groups by the
    parsed worktree-basename segment of each device name.
    """
    cfg, _ = _load(args)
    if args.global_:
        prefix = cfg.project.simulator_prefix
        groups: dict[str, list[dict]] = defaultdict(list)
        for dev in sim_mod.list_devices_by_prefix(prefix):
            parsed = sim_mod.parse_managed_name(dev["name"], prefix=prefix)
            if parsed is None:
                continue
            basename, _label = parsed
            groups[basename].append(dev)
        if not groups:
            ui.done(f"No managed simulators found (prefix: {prefix}).")
            return 0
        for basename in sorted(groups):
            ui.info(f"worktree={basename}")
            for dev in sorted(groups[basename], key=lambda d: d["name"]):
                state = dev.get("state", "?")
                ui.info(f"  {dev['name']:<40} {dev['udid']}  state={state}")
        return 0

    if not cfg.simulators:
        ui.done("No simulators configured. Run `sim pick`.")
        return 0
    for label, entry in cfg.simulators.items():
        dev = sim_mod.find_device_by_udid(entry.udid)
        state = dev.get("state", "?") if dev else "MISSING"
        ui.info(f"{label:<10} {entry.name:<40} {entry.udid}  state={state}")
    return 0


def cmd_recreate(args: argparse.Namespace) -> int:
    """Destroy + re-pick + re-boot the simulator under *label*.

    Always requires an explicit label — destructive operations should not
    silently default. Removes the simctl device, deletes the entry from
    ``simulator.toml``, then re-runs the pick flow.
    """
    sim_mod.validate_label(args.label)
    cfg, sim_toml = _load(args)

    existing = cfg.simulators.get(args.label)
    if existing is not None:
        dev = sim_mod.find_device_by_udid(existing.udid)
        if dev is not None:
            ui.step(f"Deleting {existing.name} ({existing.udid})…")
            sim_mod.shutdown(existing.udid)
            sim_mod.delete(existing.udid)
        remove_simulator_entry(sim_toml, args.label)

    # Reuse cmd_pick by synthesising a fake namespace.
    pick_args = argparse.Namespace(
        config=args.config, verbose=args.verbose,
        label=args.label, all_devices=args.all_devices,
        device=args.device, runtime=args.runtime,
    )
    return cmd_pick(pick_args)


def cmd_remove(args: argparse.Namespace) -> int:
    """Remove the entry under *label* from simulator.toml.

    Default: leave the simctl device alone (the user may want to re-register
    it under a different label). ``--destroy`` shuts down + deletes the
    device too, matching ``sim recreate``'s teardown half.
    """
    sim_mod.validate_label(args.label)
    cfg, sim_toml = _load(args)
    entry = cfg.simulators.get(args.label)
    if entry is None:
        raise UserError(f"No simulator labeled `{args.label}`.")

    if args.destroy:
        sim_mod.shutdown(entry.udid)
        sim_mod.delete(entry.udid)
        ui.step(f"Deleted simctl device {entry.name} ({entry.udid}).")

    remove_simulator_entry(sim_toml, args.label)
    ui.done(f"Removed `{args.label}` from simulator.toml.")
    return 0


def match_for_cleanup(devices: list[dict], *, prefix: str, basename: str) -> list[dict]:
    """Filter *devices* to those whose name starts with ``<prefix>-<basename>-``.

    Pure function so we can unit-test the prefix-scan logic without simctl.
    The trailing dash is required to distinguish ``Pulse-feat-auth-*`` from
    ``Pulse-feat-auth-extras-*`` if a future basename were a prefix of another.
    """
    needle = f"{prefix}-{basename}-"
    return [d for d in devices if d.get("name", "").startswith(needle)]


def cmd_cleanup(args: argparse.Namespace) -> int:
    """Decommission every simulator owned by this worktree.

    Algorithm:
      1. Scan simctl for all devices.
      2. Filter to ``<simulator_prefix>-<worktree_basename>-*`` (prefix-only;
         we do not consult ``simulator.toml`` so a corrupt/missing file is
         survivable).
      3. Confirm with the user (skipped under ``--yes``).
      4. Shutdown + delete each.
      5. Delete ``simulator.toml`` if present.
    """
    cfg, sim_toml = _load(args)
    prefix = cfg.project.simulator_prefix
    basename = cfg.worktree_root.name

    devices = sim_mod.list_all_devices()
    matched = match_for_cleanup(devices, prefix=prefix, basename=basename)
    if not matched:
        ui.done(f"No managed simulators found for worktree `{basename}`.")
        if sim_toml.exists():
            sim_toml.unlink()
            ui.info(f"Removed stale {sim_toml}.")
        return 0

    ui.info(f"Will shutdown + delete {len(matched)} simulator(s):")
    for d in matched:
        ui.info(f"  {d['name']}  ({d['udid']})")
    if not args.yes:
        if not ui.is_interactive():
            raise UserError(
                "`sim cleanup` requires --yes when stdin is not a TTY."
            )
        resp = input("Proceed? [y/N] ").strip().lower()
        if resp not in ("y", "yes"):
            ui.done("Aborted.")
            return 1

    for d in matched:
        sim_mod.shutdown(d["udid"])
        sim_mod.delete(d["udid"])
    if sim_toml.exists():
        sim_toml.unlink()
    ui.done(f"Cleaned up {len(matched)} simulator(s) for worktree `{basename}`.")
    return 0


def _format_bytes(n: int) -> str:
    """Render a byte count as a power-of-1024 string with one decimal."""
    units = ("B", "KiB", "MiB", "GiB", "TiB")
    size = float(n)
    for unit in units:
        if size < 1024:
            return f"{size:5.1f} {unit}"
        size /= 1024
    return f"{size:5.1f} PiB"


def cmd_du(args: argparse.Namespace) -> int:
    """Report disk usage for every managed simulator.

    Default: scan globally; ``--this-worktree`` filters to the current basename.
    Output is grouped by parsed worktree-basename, sorted alphabetically.
    """
    cfg, _ = _load(args)
    prefix = cfg.project.simulator_prefix
    devices = sim_mod.list_devices_by_prefix(prefix)

    rows: list[tuple[str, str, str, int]] = []  # (basename, label, name, bytes)
    for dev in devices:
        parsed = sim_mod.parse_managed_name(dev["name"], prefix=prefix)
        if parsed is None:
            continue
        basename, label = parsed
        if args.this_worktree and basename != cfg.worktree_root.name:
            continue
        size = sim_mod.du_bytes(sim_mod.device_data_dir(dev["udid"]))
        rows.append((basename, label, dev["name"], size))

    if not rows:
        scope = "this worktree" if args.this_worktree else f"prefix {prefix!r}"
        ui.done(f"No managed simulators found for {scope}.")
        return 0

    rows.sort()
    grouped: dict[str, list[tuple[str, str, int]]] = defaultdict(list)
    total = 0
    for basename, label, name, size in rows:
        grouped[basename].append((label, name, size))
        total += size

    ui.info(f"{cfg.project.simulator_prefix} simulators (prefix={prefix}):")
    ui.sep()
    for basename in sorted(grouped):
        ui.info(f"worktree={basename}")
        for label, name, size in sorted(grouped[basename]):
            ui.info(f"  {label:<10} {name:<40} {_format_bytes(size)}")
    ui.sep()
    ui.done(f"Total: {_format_bytes(total)} across {len(rows)} simulator(s).")
    return 0


def find_orphans(
    devices: list[dict], *, prefix: str, live_basenames: set[str],
) -> list[tuple[dict, str, str]]:
    """Return ``[(device, basename, label), ...]`` for every managed device
    whose worktree-basename segment is *not* in *live_basenames*.

    Pure function — gets all simctl/git work injected — so we can unit-test
    orphan detection without forking either tool.
    """
    out: list[tuple[dict, str, str]] = []
    for dev in devices:
        parsed = sim_mod.parse_managed_name(dev.get("name", ""), prefix=prefix)
        if parsed is None:
            continue
        basename, label = parsed
        if basename not in live_basenames:
            out.append((dev, basename, label))
    return out


def git_worktree_basenames(start: Path) -> set[str]:
    """Run ``git worktree list --porcelain`` from *start* and return basenames.

    A worktree's "basename" is ``Path(<worktree path>).name`` — that's the
    same string we encode into simulator names, so it's the right key for
    the orphan check.
    """
    import subprocess
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        cwd=start, capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise UserError(
            "`sim prune` must be run from inside a git worktree of the project repo. "
            f"`git worktree list` failed: {result.stderr.strip()}"
        )
    basenames: set[str] = set()
    for line in result.stdout.splitlines():
        if line.startswith("worktree "):
            basenames.add(Path(line[len("worktree "):]).name)
    return basenames


def cmd_prune(args: argparse.Namespace) -> int:
    """Detect + remove orphaned managed simulators (worktrees that no longer exist).

    Combines a global simctl scan with ``git worktree list`` to find any
    managed device whose basename segment is missing from the live worktrees.
    Interactive confirmation by default; ``--yes`` skips it.
    """
    cfg, _ = _load(args)
    prefix = cfg.project.simulator_prefix
    devices = sim_mod.list_devices_by_prefix(prefix)
    live = git_worktree_basenames(cfg.worktree_root)

    orphans = find_orphans(devices, prefix=prefix, live_basenames=live)
    if not orphans:
        ui.done("No orphaned simulators.")
        return 0

    by_basename: dict[str, list[tuple[dict, str]]] = defaultdict(list)
    for dev, basename, label in orphans:
        by_basename[basename].append((dev, label))

    ui.info(f"Found {len(orphans)} orphaned simulator(s):")
    for basename in sorted(by_basename):
        ui.info(f"  worktree={basename} (gone)")
        for dev, label in sorted(by_basename[basename], key=lambda t: t[1]):
            ui.info(f"    {dev['name']:<40} {dev['udid']}  label={label}")

    if not args.yes:
        if not ui.is_interactive():
            raise UserError("`sim prune` requires --yes when stdin is not a TTY.")
        resp = input("Delete these simulators? [y/N] ").strip().lower()
        if resp not in ("y", "yes"):
            ui.done("Aborted.")
            return 1

    for dev, _b, _l in orphans:
        sim_mod.shutdown(dev["udid"])
        sim_mod.delete(dev["udid"])
    ui.done(f"Pruned {len(orphans)} orphaned simulator(s).")
    return 0
