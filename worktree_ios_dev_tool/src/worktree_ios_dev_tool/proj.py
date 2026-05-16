# worktree_ios_dev_tool/src/worktree_ios_dev_tool/proj.py
"""Implementations of the `proj` namespace verbs: init, config, doctor.

All three operate on ``project.toml``; ``proj init`` writes it,
``proj config`` prints the resolved view, and ``proj doctor`` runs sanity
checks. None of them touch ``simulator.toml`` (that's the ``sim`` namespace).
"""
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

from . import simulator as sim_mod, ui
from .config import Config, load
from .errors import EnvError
from .paths import find_project_toml


def _load(args: argparse.Namespace) -> Config:
    """Resolve project.toml using --config override or walk-up discovery."""
    path = args.config.resolve() if args.config else find_project_toml()
    return load(path)


def cmd_init(args: argparse.Namespace) -> int:
    """Scaffold worktree-ios-dev/, write project.toml, update .gitignore.

    Delegates to the existing :mod:`bootstrap` implementation for the
    project-discovery / scheme-discovery flow; we only own the verb name.
    """
    from . import bootstrap
    return bootstrap.run(
        project=args.project,
        workspace=args.workspace,
        scheme=args.scheme,
        yes=args.yes,
        force=args.force,
    )


def cmd_config(args: argparse.Namespace) -> int:
    """Print the resolved project + simulators view as JSON, useful for debugging."""
    cfg = _load(args)
    payload = {
        "config_path": str(cfg.config_path),
        "worktree_root": str(cfg.worktree_root),
        "derived_data": str(cfg.derived_data),
        "project": {
            "path": str(cfg.project.path),
            "scheme": cfg.project.scheme,
            "configuration": cfg.project.configuration,
            "simulator_prefix": cfg.project.simulator_prefix,
        },
        "simulators": {
            label: {
                "name": s.name, "udid": s.udid,
                "device": s.device, "runtime": s.runtime,
            } for label, s in cfg.simulators.items()
        },
        "packages_root": str(cfg.packages_root),
        "package_overrides": {k: {"scheme": v.scheme} for k, v in cfg.package_overrides.items()},
        "extras": {"xcodebuild_flags": cfg.extras_xcodebuild_flags},
    }
    print(json.dumps(payload, indent=2))
    return 0


def cmd_doctor(args: argparse.Namespace) -> int:
    """Sanity-check the worktree's project + simulators + tooling.

    Missing simulator.toml is a *warn*, not an *error* — a worktree that
    just ran ``proj init`` should still see a green doctor.
    """
    problems: list[str] = []
    cfg: Config | None = None

    try:
        cfg = _load(args)
        ui.step(f"project.toml   {cfg.config_path}")
    except EnvError as e:
        ui.problem(f"project.toml   {e}")
        problems.append(str(e))

    for binary in ("xcodebuild", "xcrun"):
        path = shutil.which(binary)
        if path is None:
            ui.problem(f"{binary:<14} not on PATH")
            problems.append(f"`{binary}` not on PATH.")
        else:
            ui.step(f"{binary:<14} {path}")

    mint_path = shutil.which("mint")
    if mint_path is None:
        ui.warn("mint           not installed (optional — enables xcbeautify)")
    else:
        ui.step(f"mint           {mint_path}")

    if cfg is not None:
        if not cfg.project.path.exists():
            ui.problem(f"project        not found: {cfg.project.path}")
            problems.append(f"project.path does not exist: {cfg.project.path}")
        else:
            ui.step(f"project        {cfg.project.path}")

        if not cfg.simulators:
            ui.warn("simulators     none configured — run `worktree-ios-dev-tool sim pick`")
        else:
            for label, sim in cfg.simulators.items():
                dev = sim_mod.find_device_by_udid(sim.udid)
                if dev is None:
                    ui.problem(f"simulators[{label}]  UDID not found: {sim.udid}")
                    problems.append(f"Simulator UDID not found in simctl list: {sim.udid}")
                else:
                    state = dev.get("state", "?")
                    ui.step(f"simulators[{label}]  {sim.name} ({sim.udid})  state={state}")

        if not cfg.derived_data.parent.exists():
            ui.problem(f"worktree-ios-dev/  missing: {cfg.derived_data.parent}")
            problems.append(f"worktree-ios-dev/ missing: {cfg.derived_data.parent}")

    ui.sep()
    if problems:
        ui.done(f"{len(problems)} problem(s) found.")
        for p in problems:
            ui.info(f"- {p}")
        return 1

    ui.done("All checks passed.")
    return 0
