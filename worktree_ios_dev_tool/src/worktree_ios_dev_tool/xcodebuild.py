"""Pure argv builders for xcodebuild. No subprocess calls."""
from __future__ import annotations

from typing import Sequence

from .config import Config, SimulatorEntry, resolve_sim


def _destination(sim: SimulatorEntry) -> str:
    return f"platform=iOS Simulator,id={sim.udid}"


def _target_flag(cfg: Config) -> str:
    """Return ``-workspace`` for ``.xcworkspace`` targets, else ``-project``."""
    return "-workspace" if cfg.project.path.suffix == ".xcworkspace" else "-project"


def _common(cfg: Config, sim: SimulatorEntry | None, *, release: bool) -> list[str]:
    configuration = "Release" if release else cfg.project.configuration
    argv: list[str] = [
        "xcodebuild",
        _target_flag(cfg), str(cfg.project.path),
        "-scheme", cfg.project.scheme,
        "-configuration", configuration,
        "-derivedDataPath", str(cfg.derived_data),
    ]
    if sim is not None:
        argv += ["-destination", _destination(sim)]
    argv += cfg.extras_xcodebuild_flags
    return argv


def build_argv(
    cfg: Config, *,
    release: bool = False,
    scheme_override: str | None = None,
    sim_label: str | None = None,
) -> list[str]:
    """Build argv for ``xcodebuild build``. *sim_label* selects which entry
    in ``cfg.simulators`` provides the destination."""
    sim = resolve_sim(cfg, label=sim_label)
    argv = _common(cfg, sim, release=release)
    if scheme_override:
        argv[argv.index("-scheme") + 1] = scheme_override
    return argv + ["build"]


def test_argv(
    cfg: Config,
    *,
    release: bool = False,
    scheme_override: str | None = None,
    only_testing: Sequence[str] = (),
    skip_testing: Sequence[str] = (),
    sim_label: str | None = None,
) -> list[str]:
    sim = resolve_sim(cfg, label=sim_label)
    argv = _common(cfg, sim, release=release) + ["test"]
    if scheme_override:
        argv[argv.index("-scheme") + 1] = scheme_override
    for t in only_testing:
        argv += ["-only-testing", t]
    for t in skip_testing:
        argv += ["-skip-testing", t]
    return argv


def clean_argv(cfg: Config) -> list[str]:
    # clean doesn't need a destination.
    return _common(cfg, None, release=False) + ["clean"]
