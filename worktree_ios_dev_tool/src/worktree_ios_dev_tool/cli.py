# worktree_ios_dev_tool/src/worktree_ios_dev_tool/cli.py
"""worktree-ios-dev-tool — argparse dispatcher + verb handlers."""
from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

from . import config as config_mod, packages as packages_mod, ui, xcodebuild
from .errors import WorktreeIosError
from .paths import find_project_toml
from .proc import run


def _add_common(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "--project-toml", dest="config", type=Path, default=None,
        help="Path to project.toml (overrides walk-up discovery). simulator.toml is resolved alongside it.",
    )
    parser.add_argument("-v", "--verbose", action="store_true", help="Show full xcodebuild output (no --quiet) and Python tracebacks.")


def _load_config(args: argparse.Namespace) -> config_mod.Config:
    path = args.config.resolve() if args.config else find_project_toml()
    return config_mod.load(path)


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(prog="worktree-ios-dev-tool", description="iOS worktree build / sim / test CLI.")
    sub = p.add_subparsers(dest="verb", required=True)

    proj = sub.add_parser("proj", help="Project lifecycle: init / config / doctor.")
    proj_sub = proj.add_subparsers(dest="proj_verb", required=True)

    pi = proj_sub.add_parser("init", help="Scaffold worktree-ios-dev/ and write project.toml.")
    pi.add_argument("--project", default=None, help="Relative path to .xcodeproj (skips auto-discovery).")
    pi.add_argument("--workspace", default=None, help="Relative path to .xcworkspace (skips auto-discovery).")
    pi.add_argument("--scheme", default=None, help="Scheme name (skips auto-discovery).")
    pi.add_argument("--yes", action="store_true", help="Accept all detected defaults; error if ambiguous.")
    pi.add_argument("--force", action="store_true", help="Re-write project.toml even if it already exists.")
    _add_common(pi)
    pi.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.proj", fromlist=["cmd_init"]
    ).cmd_init(a))

    pc = proj_sub.add_parser("config", help="Print the resolved project + simulators view as JSON.")
    _add_common(pc)
    pc.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.proj", fromlist=["cmd_config"]
    ).cmd_config(a))

    pd = proj_sub.add_parser("doctor", help="Run sanity checks on tooling, project, and simulators.")
    _add_common(pd)
    pd.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.proj", fromlist=["cmd_doctor"]
    ).cmd_doctor(a))

    sim_p = sub.add_parser("sim", help="Simulator lifecycle: pick / boot / shutdown / list / ...")
    sim_sub = sim_p.add_subparsers(dest="sim_verb", required=True)

    spk = sim_sub.add_parser("pick", help="Pick + create a simulator (interactive) or auto-select (non-TTY).")
    spk.add_argument("label", nargs="?", default=None,
                     help="Label under which to register the sim. Defaults to `default`.")
    spk.add_argument("--all-devices", action="store_true",
                     help="Disable the iPhone 17 filter when picking.")
    spk.add_argument("--device", default=None, help="Device type name (e.g. 'iPhone 17 Pro').")
    spk.add_argument("--runtime", default=None, help="Runtime name (e.g. 'iOS 26.2').")
    _add_common(spk)
    spk.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_pick"]
    ).cmd_pick(a))

    sbt = sim_sub.add_parser("boot", help="Boot a configured simulator.")
    sbt.add_argument("label", nargs="?", default=None,
                     help="Sim label to boot. Single-sim setups can omit; multi-sim must pass --all or a label.")
    sbt.add_argument("--all", action="store_true", help="Boot every configured simulator.")
    _add_common(sbt)
    sbt.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_boot"]
    ).cmd_boot(a))

    ssh = sim_sub.add_parser("shutdown", help="Shutdown a configured simulator.")
    ssh.add_argument("label", nargs="?", default=None, help="Sim label to shut down.")
    ssh.add_argument("--all", action="store_true", help="Shutdown every configured simulator.")
    _add_common(ssh)
    ssh.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_shutdown"]
    ).cmd_shutdown(a))

    sls = sim_sub.add_parser("list", help="List configured simulators (or all managed sims with --global).")
    sls.add_argument("--global", dest="global_", action="store_true",
                     help="Scan simctl for every <simulator_prefix>-* device, grouped by worktree.")
    _add_common(sls)
    sls.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_list"]
    ).cmd_list(a))

    src = sim_sub.add_parser("recreate", help="Destroy + re-pick the named simulator.")
    src.add_argument("label", help="Sim label to recreate (required; destructive).")
    src.add_argument("--all-devices", action="store_true",
                     help="Disable the iPhone 17 filter when picking.")
    src.add_argument("--device", default=None, help="Device type name (e.g. 'iPhone 17 Pro').")
    src.add_argument("--runtime", default=None, help="Runtime name (e.g. 'iOS 26.2').")
    _add_common(src)
    src.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_recreate"]
    ).cmd_recreate(a))

    srm = sim_sub.add_parser("remove", help="Remove a simulator entry from simulator.toml.")
    srm.add_argument("label", help="Sim label to remove.")
    srm.add_argument("--destroy", action="store_true",
                     help="Also delete the simctl device, not just the toml entry.")
    _add_common(srm)
    srm.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_remove"]
    ).cmd_remove(a))

    scl = sim_sub.add_parser("cleanup", help="Tear down all simulators owned by this worktree.")
    scl.add_argument("--yes", action="store_true", help="Skip the confirmation prompt.")
    _add_common(scl)
    scl.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_cleanup"]
    ).cmd_cleanup(a))

    sdu = sim_sub.add_parser("du", help="Report disk usage of every managed simulator.")
    sdu.add_argument("--this-worktree", action="store_true",
                     help="Restrict scan to this worktree's simulators.")
    _add_common(sdu)
    sdu.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_du"]
    ).cmd_du(a))

    spr = sim_sub.add_parser("prune", help="Find + delete simulators whose worktrees no longer exist.")
    spr.add_argument("--yes", action="store_true", help="Skip the confirmation prompt.")
    _add_common(spr)
    spr.set_defaults(func=lambda a: __import__(
        "worktree_ios_dev_tool.sim", fromlist=["cmd_prune"]
    ).cmd_prune(a))

    b = sub.add_parser("build", help="xcodebuild build the main scheme.")
    b.add_argument("--release", action="store_true", help="Use Release configuration.")
    b.add_argument("--scheme", default=None, help="Override project.scheme.")
    b.add_argument("--sim", default=None, help="Sim label when multiple are configured.")
    _add_common(b)
    b.set_defaults(func=_cmd_build)

    t = sub.add_parser("test", help="xcodebuild test the main scheme.")
    t.add_argument("--release", action="store_true", help="Use Release configuration.")
    t.add_argument("--scheme", default=None, help="Override project.scheme.")
    t.add_argument("--only-testing", action="append", default=[], metavar="TEST_ID")
    t.add_argument("--skip-testing", action="append", default=[], metavar="TEST_ID")
    t.add_argument("--sim", default=None, help="Sim label when multiple are configured.")
    _add_common(t)
    t.set_defaults(func=_cmd_test)

    rn = sub.add_parser("run", help="Build, install on the simulator, launch.")
    rn.add_argument("--release", action="store_true", help="Use Release configuration.")
    rn.add_argument("--sim", default=None, help="Sim label when multiple are configured.")
    _add_common(rn)
    rn.set_defaults(func=_cmd_run)

    cl = sub.add_parser("clean", help="xcodebuild clean.")
    _add_common(cl)
    cl.set_defaults(func=_cmd_clean)

    wd = sub.add_parser("wipe-derived", help="rm -rf the DerivedData dir.")
    wd.add_argument("--yes", action="store_true", help="Skip the confirmation prompt.")
    _add_common(wd)
    wd.set_defaults(func=_cmd_wipe_derived)

    tp = sub.add_parser("test-package", help="Run a local Swift package's tests.")
    tp.add_argument("name", help="Directory / scheme name under packages_root.")
    tp.add_argument("--sim", default=None, help="Sim label when multiple are configured.")
    _add_common(tp)
    tp.set_defaults(func=_cmd_test_package)

    return p


# ---- handlers ----------------------------------------------------------------

def _cmd_clean(args: argparse.Namespace) -> int:
    cfg = _load_config(args)
    ui.step("Cleaning…")
    argv = xcodebuild.clean_argv(cfg)
    run(argv, verbose=args.verbose, quiet=not args.verbose)
    ui.done("Clean succeeded.")
    return 0


def _cmd_wipe_derived(args: argparse.Namespace) -> int:
    cfg = _load_config(args)
    target = cfg.derived_data
    if not target.exists():
        ui.done(f"Nothing to wipe — {target} does not exist.")
        return 0
    if not args.yes:
        resp = input(f"◇  Delete {target}? [y/N] ").strip().lower()
        if resp not in ("y", "yes"):
            ui.done("Aborted.")
            return 1
    shutil.rmtree(target)
    target.mkdir()
    ui.done(f"Wiped {target}.")
    return 0


def _cmd_build(args: argparse.Namespace) -> int:
    cfg = _load_config(args)
    scheme = args.scheme or cfg.project.scheme
    config = "Release" if args.release else cfg.project.configuration
    ui.step(f"Building {scheme} ({config})…")
    argv = xcodebuild.build_argv(
        cfg, release=args.release, scheme_override=args.scheme, sim_label=args.sim,
    )
    run(argv, verbose=args.verbose, quiet=not args.verbose)
    ui.done("Build succeeded.")
    return 0


def _cmd_test(args: argparse.Namespace) -> int:
    cfg = _load_config(args)
    scheme = args.scheme or cfg.project.scheme
    config = "Release" if args.release else cfg.project.configuration
    ui.step(f"Testing {scheme} ({config})…")
    argv = xcodebuild.test_argv(
        cfg, release=args.release, scheme_override=args.scheme,
        only_testing=args.only_testing, skip_testing=args.skip_testing,
        sim_label=args.sim,
    )
    run(argv, verbose=args.verbose, quiet=not args.verbose)
    ui.done("Tests passed.")
    return 0


def _cmd_run(args: argparse.Namespace) -> int:
    from .runapp import run as run_run
    return run_run(args)


def _cmd_test_package(args: argparse.Namespace) -> int:
    cfg = _load_config(args)
    ui.step(f"Testing package {args.name}…")
    argv, cwd = packages_mod.resolve(cfg, args.name, sim_label=args.sim)
    run(argv, cwd=cwd, verbose=args.verbose, quiet=not args.verbose)
    ui.done(f"Tests passed — {args.name}.")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except WorktreeIosError as e:
        print(f"error: {e}", file=sys.stderr)
        if args.verbose:
            import traceback
            traceback.print_exc()
        return e.exit_code
    except KeyboardInterrupt:
        return 130
