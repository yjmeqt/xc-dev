# worktree_ios_dev_tool/src/worktree_ios_dev_tool/config.py
"""TOML read + schema validation. Writes via tomlkit to preserve comments."""
from __future__ import annotations

import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import tomlkit

from .errors import UserError
from .paths import derived_data_dir, simulator_toml_for, worktree_root


@dataclass(frozen=True)
class ProjectConfig:
    path: Path             # absolute
    scheme: str
    configuration: str
    simulator_prefix: str | None = None


@dataclass(frozen=True)
class SimulatorEntry:
    """One configured simulator under [simulators.<label>] in simulator.toml.

    Attributes:
        name:    The simctl device name. Always synthesized as
                 ``<simulator_prefix>-<worktree_basename>-<label>``; never hand-edit.
        udid:    The simctl-assigned UUID; the source of truth for routing
                 build/test/run destinations.
        device:  Apple device-type display name (e.g. "iPhone 17 Pro").
        runtime: Apple runtime display name (e.g. "iOS 18.2").
    """
    name: str
    udid: str
    device: str
    runtime: str


@dataclass(frozen=True)
class PackageOverride:
    scheme: str | None = None


@dataclass(frozen=True)
class Config:
    """Resolved view of project.toml + simulator.toml.

    ``simulators`` is keyed by the user-chosen label (default key: ``"default"``).
    An empty dict means no simulator has been picked yet — commands that need
    one should raise via :func:`resolve_sim`.
    """
    config_path: Path                              # absolute path to project.toml
    worktree_root: Path                            # parent of worktree-ios-dev/
    derived_data: Path                             # absolute DerivedData dir
    project: ProjectConfig
    simulators: dict[str, SimulatorEntry] = field(default_factory=dict)
    packages_root: Path = field(default=Path("."))
    package_overrides: dict[str, PackageOverride] = field(default_factory=dict)
    extras_xcodebuild_flags: list[str] = field(default_factory=list)


PROJECT_SCHEMA_VERSION = 1
SIMULATOR_SCHEMA_VERSION = 1

_ALLOWED_PROJECT_TOP = {"schema_version", "project", "packages_root", "packages", "extras"}
_ALLOWED_SIMULATOR_TOP = {"schema_version", "simulators"}
_ALLOWED_PROJECT = {"path", "scheme", "configuration", "simulator_prefix"}
_ALLOWED_SIMULATOR = {"name", "udid", "device", "runtime"}
_ALLOWED_PACKAGES_ROOT = {"path"}
_ALLOWED_PACKAGE = {"scheme"}
_ALLOWED_EXTRAS = {"xcodebuild_flags"}


def _require_keys(table_name: str, table: dict[str, Any], allowed: set[str]) -> None:
    extra = set(table) - allowed
    if extra:
        raise UserError(
            f"Unknown key(s) in [{table_name}]: {sorted(extra)}. "
            f"Allowed: {sorted(allowed)}."
        )


def _read_toml(path: Path) -> dict:
    """Return parsed TOML, surfacing path in any error."""
    with path.open("rb") as fh:
        return tomllib.load(fh)


def _check_version(data: dict, path: Path, *, expected: int, recovery: str) -> None:
    """Validate ``schema_version`` at the top of *data*.

    Missing → UserError citing *recovery* (the verb to re-run).
    Mismatched → UserError naming the version we found vs what we accept.
    """
    if "schema_version" not in data:
        raise UserError(
            f"{path}: missing `schema_version`. "
            f"Re-run `worktree-ios-dev-tool {recovery}` to regenerate the file."
        )
    v = data["schema_version"]
    if v != expected:
        raise UserError(
            f"{path}: schema_version {v!r} not supported (expected {expected}). "
            f"Upgrade worktree-ios-dev-tool or migrate manually."
        )


def _load_simulators(simulator_toml: Path) -> dict[str, SimulatorEntry]:
    """Read ``simulator.toml``. A missing file means an empty dict (not yet picked)."""
    if not simulator_toml.exists():
        return {}
    data = _read_toml(simulator_toml)
    _check_version(
        data, simulator_toml,
        expected=SIMULATOR_SCHEMA_VERSION,
        recovery="sim pick",
    )
    _require_keys("<simulator.toml root>", data, _ALLOWED_SIMULATOR_TOP)
    out: dict[str, SimulatorEntry] = {}
    for label, entry in (data.get("simulators", {}) or {}).items():
        _require_keys(f"simulators.{label}", entry, _ALLOWED_SIMULATOR)
        for key in _ALLOWED_SIMULATOR:
            if key not in entry:
                raise UserError(
                    f"{simulator_toml}: [simulators.{label}] missing `{key}`."
                )
        out[label] = SimulatorEntry(
            name=entry["name"], udid=entry["udid"],
            device=entry["device"], runtime=entry["runtime"],
        )
    return out


def load(project_toml: Path) -> Config:
    """Load and validate ``project.toml`` and (optionally) ``simulator.toml``.

    *project_toml* must point at the new split file; legacy ``config.toml``
    is rejected at discovery time by :func:`paths.find_project_toml`. Each
    file carries a ``schema_version`` integer; we currently only accept
    version 1, and unknown versions are a hard UserError so future bumps
    can carry migration logic without silent misreads.
    """
    proj_data = _read_toml(project_toml)
    _check_version(
        proj_data, project_toml,
        expected=PROJECT_SCHEMA_VERSION,
        recovery="proj init",
    )
    _require_keys("<project.toml root>", proj_data, _ALLOWED_PROJECT_TOP)

    if "project" not in proj_data:
        raise UserError(
            f"Missing [project] section in {project_toml}. "
            f"Re-run `worktree-ios-dev-tool proj init --force` to re-seed."
        )
    proj = proj_data["project"]
    _require_keys("project", proj, _ALLOWED_PROJECT)
    for key in ("path", "scheme"):
        if key not in proj:
            raise UserError(f"[project] missing `{key}` in {project_toml}.")
    wt_root = worktree_root(project_toml)
    proj_path = (wt_root / proj["path"]).resolve()
    if proj_path.suffix not in (".xcworkspace", ".xcodeproj"):
        raise UserError(
            f"[project].path must point at a .xcworkspace or .xcodeproj. "
            f"Got: {proj_path}"
        )
    project = ProjectConfig(
        path=proj_path,
        scheme=proj["scheme"],
        configuration=proj.get("configuration", "Debug"),
        simulator_prefix=proj.get("simulator_prefix") or proj["scheme"],
    )

    pkg_root_cfg = proj_data.get("packages_root", {"path": "ios/Packages"})
    _require_keys("packages_root", pkg_root_cfg, _ALLOWED_PACKAGES_ROOT)
    packages_root = (wt_root / pkg_root_cfg.get("path", "ios/Packages")).resolve()

    overrides: dict[str, PackageOverride] = {}
    for name, table in (proj_data.get("packages", {}) or {}).items():
        _require_keys(f"packages.{name}", table, _ALLOWED_PACKAGE)
        overrides[name] = PackageOverride(scheme=table.get("scheme"))

    extras = proj_data.get("extras", {})
    _require_keys("extras", extras, _ALLOWED_EXTRAS)
    flags = extras.get("xcodebuild_flags", [])
    if not isinstance(flags, list) or not all(isinstance(f, str) for f in flags):
        raise UserError(
            f"[extras].xcodebuild_flags must be a list of strings in {project_toml}."
        )

    simulators = _load_simulators(simulator_toml_for(project_toml))

    return Config(
        config_path=project_toml,
        worktree_root=wt_root,
        derived_data=derived_data_dir(project_toml),
        project=project,
        simulators=simulators,
        packages_root=packages_root,
        package_overrides=overrides,
        extras_xcodebuild_flags=list(flags),
    )


def resolve_sim(cfg: Config, label: str | None) -> SimulatorEntry:
    """Pick the simulator a command should target.

    Resolution rules:
      * No simulators configured: raise UserError pointing at ``sim pick``.
      * ``label`` omitted and exactly one configured: return that one.
      * ``label`` omitted and multiple configured: raise UserError listing
        the available labels — destructive ambiguity is never silent.
      * ``label`` present but unknown: raise UserError listing what's available.
      * ``label`` present and known: return that entry.
    """
    if not cfg.simulators:
        raise UserError(
            "No simulators configured. Run `worktree-ios-dev-tool sim pick` first."
        )
    if label is None:
        if len(cfg.simulators) == 1:
            (only,) = cfg.simulators.values()
            return only
        labels = ", ".join(sorted(cfg.simulators))
        raise UserError(
            f"Multiple simulators configured ({labels}). "
            f"Pass `--sim <label>` to disambiguate."
        )
    try:
        return cfg.simulators[label]
    except KeyError:
        labels = ", ".join(sorted(cfg.simulators))
        raise UserError(
            f"No simulator labeled `{label}`. Configured: {labels}."
        )


def write_simulators_toml(simulator_toml: Path, simulators: dict[str, SimulatorEntry]) -> None:
    """Render *simulators* to ``simulator.toml`` (creating the file).

    Uses tomlkit so future hand-edits keep their comments; we still rewrite
    every key the tool owns. ``schema_version`` is always set to
    :data:`SIMULATOR_SCHEMA_VERSION`.
    """
    doc = tomlkit.document()
    doc.add(tomlkit.comment("worktree-ios-dev/simulator.toml"))
    doc.add(tomlkit.comment(
        "Managed by `worktree-ios-dev-tool sim pick / recreate / remove`. "
        "Do not hand-edit udid or name."
    ))
    doc.add(tomlkit.nl())
    doc["schema_version"] = SIMULATOR_SCHEMA_VERSION

    if simulators:
        outer = tomlkit.table(is_super_table=True)
        for label, entry in simulators.items():
            inner = tomlkit.table()
            inner["name"] = entry.name
            inner["udid"] = entry.udid
            inner["device"] = entry.device
            inner["runtime"] = entry.runtime
            outer[label] = inner
        doc["simulators"] = outer

    simulator_toml.write_text(tomlkit.dumps(doc))


def upsert_simulator_entry(simulator_toml: Path, label: str, entry: SimulatorEntry) -> None:
    """Insert/replace a single ``[simulators.<label>]`` entry, preserving siblings."""
    existing = _load_simulators(simulator_toml) if simulator_toml.exists() else {}
    existing[label] = entry
    write_simulators_toml(simulator_toml, existing)


def remove_simulator_entry(simulator_toml: Path, label: str) -> bool:
    """Delete the ``[simulators.<label>]`` entry. Returns True if removed, False if absent."""
    if not simulator_toml.exists():
        return False
    existing = _load_simulators(simulator_toml)
    if label not in existing:
        return False
    del existing[label]
    write_simulators_toml(simulator_toml, existing)
    return True
