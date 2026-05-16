"""Implementation of the `run` verb: build -> install -> launch."""
from __future__ import annotations

import argparse
import plistlib
from pathlib import Path

from . import simulator as sim_mod, ui, xcodebuild
from .config import load, resolve_sim
from .errors import UserError
from .paths import find_project_toml
from .proc import run as proc_run


def _find_app(derived_data: Path, scheme: str, configuration: str) -> Path:
    # xcodebuild writes to Build/Products/<Configuration>-iphonesimulator/<scheme>.app
    candidate_dir = derived_data / "Build" / "Products" / f"{configuration}-iphonesimulator"
    app_path = candidate_dir / f"{scheme}.app"
    if app_path.is_dir():
        return app_path
    # Fallback: glob.
    matches = list(candidate_dir.glob("*.app"))
    if len(matches) == 1:
        return matches[0]
    raise UserError(
        f"Could not locate a built .app under {candidate_dir}. "
        f"Expected {scheme}.app. Try `worktree-ios-dev-tool build` first."
    )


def _bundle_id(app_path: Path) -> str:
    info = app_path / "Info.plist"
    with info.open("rb") as fh:
        plist = plistlib.load(fh)
    bundle_id = plist.get("CFBundleIdentifier")
    if not bundle_id:
        raise UserError(f"CFBundleIdentifier missing in {info}.")
    return bundle_id


def run(args: argparse.Namespace) -> int:
    cfg_path = args.config.resolve() if args.config else find_project_toml()
    cfg = load(cfg_path)
    sim = resolve_sim(cfg, label=getattr(args, "sim", None))

    # 1. Build
    ui.step("Building…")
    argv = xcodebuild.build_argv(cfg, release=args.release, sim_label=getattr(args, "sim", None))
    proc_run(argv, verbose=args.verbose, quiet=not args.verbose)

    # 2. Locate .app
    configuration = "Release" if args.release else cfg.project.configuration
    app_path = _find_app(cfg.derived_data, cfg.project.scheme, configuration)
    bundle_id = _bundle_id(app_path)

    # 3. Boot sim if needed, install, launch.
    ui.step("Installing…")
    sim_mod.boot(sim.udid)
    proc_run(["xcrun", "simctl", "install", sim.udid, str(app_path)], verbose=True)

    ui.step("Launching…")
    result = proc_run(["xcrun", "simctl", "launch", sim.udid, bundle_id], capture=True, verbose=True)
    ui.sep()
    ui.done(f"Launched  {bundle_id}")
    ui.info(result.stdout.strip())

    return 0
