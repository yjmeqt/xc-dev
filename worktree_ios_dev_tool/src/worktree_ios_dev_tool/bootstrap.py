# worktree_ios_dev_tool/src/worktree_ios_dev_tool/bootstrap.py
"""Implementation of the ``proj init`` verb.

The module name is historical (predates the ``proj`` namespace); the public
entry point is :func:`run`, which is invoked from
:mod:`worktree_ios_dev_tool.proj.cmd_init`.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from . import ui
from .errors import UserError
from .paths import find_worktree_root_for_bootstrap

_GITIGNORE_MARKER = "# added by worktree-ios-dev-tool proj init"
_GITIGNORE_ENTRY = "worktree-ios-dev/"


def find_xcodeprojs(root: Path) -> list[Path]:
    """Find all *.xcodeproj directories under root, sorted. Does not recurse inside .xcodeproj."""
    results = []
    for p in sorted(root.rglob("*.xcodeproj")):
        rel_parts = p.relative_to(root).parts[:-1]
        if not any(part.endswith(".xcodeproj") for part in rel_parts):
            results.append(p)
    return results


def find_xcworkspaces(root: Path) -> list[Path]:
    """Find all *.xcworkspace directories under root, sorted.

    Skips workspaces nested inside an ``.xcodeproj`` (Xcode places an internal
    ``project.xcworkspace`` there that is not a user-buildable target).
    """
    results = []
    for p in sorted(root.rglob("*.xcworkspace")):
        rel_parts = p.relative_to(root).parts[:-1]
        if any(part.endswith(".xcodeproj") for part in rel_parts):
            continue
        results.append(p)
    return results


def find_build_targets(root: Path) -> list[Path]:
    """Discover buildable targets (``.xcworkspace`` + ``.xcodeproj``) under *root*.

    When a workspace and a project share the same parent directory, the
    project is dropped — Xcode/CocoaPods/Tuist convention is to drive builds
    through the workspace, and listing both would just clutter the picker.
    """
    workspaces = find_xcworkspaces(root)
    projects = find_xcodeprojs(root)
    workspace_dirs = {w.parent for w in workspaces}
    projects = [p for p in projects if p.parent not in workspace_dirs]
    return sorted(workspaces + projects, key=lambda p: (p.parent, p.name))


def _target_flag(target: Path) -> str:
    """Return ``-workspace`` or ``-project`` depending on the target's suffix."""
    return "-workspace" if target.suffix == ".xcworkspace" else "-project"


def _parse_leading_json(text: str):
    """Parse the first JSON value in *text*, tolerating trailing non-JSON noise.

    ``xcodebuild`` occasionally appends warnings like
    ``Found unhandled resource at …`` on stdout after the JSON blob, which
    breaks plain ``json.loads`` with an "Extra data" error.
    """
    if not text:
        return None
    stripped = text.lstrip()
    offset = len(text) - len(stripped)
    try:
        value, _end = json.JSONDecoder().raw_decode(text, idx=offset)
    except (json.JSONDecodeError, ValueError):
        return None
    return value


def fetch_schemes(target: Path) -> list[str]:
    """Return scheme names from ``xcodebuild -list -json``. Returns [] on failure.

    The flag (``-workspace`` vs ``-project``) is chosen from *target*'s suffix
    so this helper works uniformly for both target kinds. The JSON response
    keys differ too: workspaces report under ``workspace.schemes``, projects
    under ``project.schemes``.
    """
    try:
        result = subprocess.run(
            ["xcodebuild", "-list", _target_flag(target), str(target), "-json"],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return []
    if result.returncode != 0:
        return []
    data = _parse_leading_json(result.stdout)
    if not isinstance(data, dict):
        return []
    container = data.get("workspace") or data.get("project") or {}
    return container.get("schemes", [])


def fetch_app_metadata(
    target: Path, scheme: str, configuration: str
) -> dict[str, str]:
    """Return resolved app metadata from ``xcodebuild -showBuildSettings``.

    Keys present in the result (any/all may be missing):
      * ``bundle_id``       — PRODUCT_BUNDLE_IDENTIFIER
      * ``product_name``    — FULL_PRODUCT_NAME with ``.app`` stripped,
                              else PRODUCT_NAME
      * ``display_name``    — INFOPLIST_KEY_CFBundleDisplayName
                              (Xcode 13+ generated Info.plist; only set when
                              the target overrides the display name)
      * ``min_ios_version`` — IPHONEOS_DEPLOYMENT_TARGET

    On any failure (xcodebuild missing, non-zero exit, JSON parse failure,
    timeout) returns ``{}``. Callers should treat the metadata as advisory
    and tolerate any subset being absent.
    """
    # ``-destination 'generic/platform=iOS Simulator'`` keeps xcodebuild from
    # interactively prompting for a destination or failing on multi-platform
    # schemes; we only need build settings, not a real device.
    try:
        result = subprocess.run(
            [
                "xcodebuild", "-showBuildSettings", "-json",
                _target_flag(target), str(target),
                "-scheme", scheme,
                "-configuration", configuration,
                "-destination", "generic/platform=iOS Simulator",
            ],
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return {}
    if result.returncode != 0:
        return {}
    entries = _parse_leading_json(result.stdout)
    if not isinstance(entries, list) or not entries:
        return {}
    # -showBuildSettings emits one entry per target/action; the first entry
    # corresponds to the scheme's primary target which is what we want.
    settings = entries[0].get("buildSettings", {}) or {}

    out: dict[str, str] = {}
    if (bid := settings.get("PRODUCT_BUNDLE_IDENTIFIER")):
        out["bundle_id"] = bid
    # FULL_PRODUCT_NAME is e.g. "BedTerm.app"; PRODUCT_NAME is "BedTerm".
    # Prefer the former with .app stripped so framework schemes report e.g.
    # "MyKit" not "MyKit.framework" later (we'd still want the basename).
    full = settings.get("FULL_PRODUCT_NAME") or ""
    if full:
        # strip a trailing ".app" / ".framework" / etc., otherwise use PRODUCT_NAME
        base = full.rsplit(".", 1)[0] if "." in full else full
        out["product_name"] = base
    elif (pname := settings.get("PRODUCT_NAME")):
        out["product_name"] = pname
    if (dn := settings.get("INFOPLIST_KEY_CFBundleDisplayName")):
        out["display_name"] = dn
    if (mv := settings.get("IPHONEOS_DEPLOYMENT_TARGET")):
        out["min_ios_version"] = mv
    return out


def detect_packages_root(target: Path) -> Path | None:
    """Return ``<target_dir>/Packages`` if it exists, else None."""
    candidate = target.parent / "Packages"
    return candidate if candidate.is_dir() else None


def _pick_one(label: str, options: list[str], yes: bool) -> str:
    """Return single option directly, prompt if TTY, error if non-interactive and ambiguous."""
    if len(options) == 1:
        return options[0]
    if yes or not sys.stdin.isatty() or not sys.stdout.isatty():
        lines = "\n".join(f"  {o}" for o in options)
        raise UserError(
            f"Multiple {label} found. Pass the appropriate flag:\n{lines}"
        )
    try:
        from InquirerPy import inquirer
        return inquirer.fuzzy(message=f"Select {label}:", choices=options).execute()
    except (ImportError, OSError, EOFError):
        print(f"Select {label}:")
        for i, opt in enumerate(options, start=1):
            print(f"  {i}. {opt}")
        while True:
            raw = input("Choice: ").strip()
            if raw.isdigit():
                idx = int(raw)
                if 1 <= idx <= len(options):
                    return options[idx - 1]
            print(f"Enter a number 1..{len(options)}.")


def _ensure_gitignored(root: Path) -> None:
    gi = root / ".gitignore"
    if gi.exists():
        content = gi.read_text()
        if any(line.strip() == _GITIGNORE_ENTRY for line in content.splitlines()):
            return
        suffix = "" if content.endswith("\n") else "\n"
        gi.write_text(content + f"{suffix}{_GITIGNORE_MARKER}\n{_GITIGNORE_ENTRY}\n")
    else:
        gi.write_text(f"{_GITIGNORE_MARKER}\n{_GITIGNORE_ENTRY}\n")


def _write_config(
    cfg_path: Path,
    target: Path,
    root: Path,
    scheme: str,
    sim_prefix: str,
    pkg_root: Path | None,
    app_metadata: dict[str, str] | None = None,
) -> None:
    import tomlkit
    from .config import PROJECT_SCHEMA_VERSION
    doc = tomlkit.document()
    doc.add(tomlkit.comment("worktree-ios-dev/project.toml"))
    doc.add(tomlkit.comment("Generated by `worktree-ios-dev-tool proj init`."))
    doc.add(tomlkit.comment("Edit as needed. Run `worktree-ios-dev-tool sim pick` to populate simulator.toml."))
    doc.add(tomlkit.nl())
    doc["schema_version"] = PROJECT_SCHEMA_VERSION

    proj_table = tomlkit.table()
    proj_table.add("path", str(target.relative_to(root)))
    proj_table.add("scheme", scheme)
    proj_table.add("configuration", "Debug")
    proj_table.add("simulator_prefix", sim_prefix)
    # Resolved-from-xcodebuild app metadata. Each key is only written when
    # `xcodebuild -showBuildSettings` actually returned a value, so framework
    # / CLI schemes don't get bogus `bundle_id = ""`. Consumers must treat
    # these as optional.
    for key in ("bundle_id", "product_name", "display_name", "min_ios_version"):
        if app_metadata and (value := app_metadata.get(key)):
            proj_table.add(key, value)
    doc.add("project", proj_table)

    if pkg_root is not None:
        pkg_table = tomlkit.table()
        pkg_table.add("path", str(pkg_root.relative_to(root)))
        doc.add("packages_root", pkg_table)

    extras_table = tomlkit.table()
    extras_table.add("xcodebuild_flags", tomlkit.array())
    doc.add("extras", extras_table)

    cfg_path.write_text(tomlkit.dumps(doc))


def run(
    *,
    project: str | None,
    workspace: str | None,
    scheme: str | None,
    yes: bool,
    force: bool,
) -> int:
    root = find_worktree_root_for_bootstrap()
    pid = root / "worktree-ios-dev"
    cfg_path = pid / "project.toml"
    interactive = ui.is_interactive() and not yes

    if cfg_path.exists() and not force:
        ui.done(f"Already initialised at {pid} (use --force to re-seed).")
        return 0

    if project and workspace:
        raise UserError("Pass either --project or --workspace, not both.")

    # ── Discover build target (xcworkspace or xcodeproj) ───────────────────
    explicit = workspace or project
    if explicit:
        target = (root / explicit).resolve()
        if not target.exists():
            raise UserError(f"Build target not found: {target}")
        if target.suffix not in (".xcworkspace", ".xcodeproj"):
            raise UserError(
                f"Build target must be a .xcworkspace or .xcodeproj. Got: {target}"
            )
        if workspace and target.suffix != ".xcworkspace":
            raise UserError(f"--workspace expects a .xcworkspace path. Got: {target}")
        if project and target.suffix != ".xcodeproj":
            raise UserError(f"--project expects a .xcodeproj path. Got: {target}")
        try:
            target.relative_to(root)
        except ValueError:
            flag = "--workspace" if workspace else "--project"
            raise UserError(
                f"{flag} path must be inside the worktree root ({root}). "
                f"Got: {target}"
            )
        ui.step(f"Target: {explicit}")
    else:
        with ui.spinner("Scanning for Xcode projects and workspaces..."):
            targets = find_build_targets(root)

        if not targets:
            raise UserError(
                "No *.xcworkspace or *.xcodeproj found under the worktree root. "
                "Pass --workspace <path> or --project <path>."
            )

        rel_targets = [str(t.relative_to(root)) for t in targets]
        chosen_rel = _pick_one("build target", rel_targets, yes)
        target = (root / chosen_rel).resolve()
        ui.step(f"Target: {chosen_rel}")

    # ── Discover scheme ─────────────────────────────────────────────────────
    if scheme:
        chosen_scheme = scheme
        ui.step(f"Scheme: {chosen_scheme}")
    else:
        with ui.spinner("Fetching schemes..."):
            schemes = fetch_schemes(target)

        if not schemes:
            raise UserError(f"No schemes found in {target}. Pass --scheme <name>.")

        chosen_scheme = _pick_one("scheme", schemes, yes)
        ui.step(f"Scheme: {chosen_scheme}")

    # ── packages_root ───────────────────────────────────────────────────────
    pkg_root = detect_packages_root(target)
    if pkg_root:
        ui.step(f"Packages root: {pkg_root.relative_to(root)}")

    # ── simulator_prefix ────────────────────────────────────────────────────
    sim_prefix = chosen_scheme
    if interactive:
        try:
            from InquirerPy import inquirer
            result = inquirer.text(
                message="Simulator prefix",
                default=chosen_scheme,
            ).execute()
            if result and result.strip():
                sim_prefix = result.strip()
        except (ImportError, OSError, EOFError):
            raw = input(f"Simulator prefix [{chosen_scheme}]: ").strip()
            if raw:
                sim_prefix = raw

    # ── App metadata (bundle_id, product_name, ...) ────────────────────────
    # Best-effort: a failure here just means the new keys won't be written.
    # Old verbs continue to work; downstream log filters / tooling fall back
    # to their own heuristics.
    with ui.spinner("Resolving app metadata (xcodebuild -showBuildSettings)…"):
        app_metadata = fetch_app_metadata(target, chosen_scheme, "Debug")
    if not app_metadata:
        ui.warn(
            "app metadata   could not be resolved from xcodebuild — "
            "bundle_id / product_name omitted from project.toml"
        )

    # ── Write ───────────────────────────────────────────────────────────────
    pid.mkdir(exist_ok=True)
    (pid / "derivedData").mkdir(exist_ok=True)
    _write_config(
        cfg_path, target, root, chosen_scheme, sim_prefix, pkg_root,
        app_metadata=app_metadata,
    )
    _ensure_gitignored(root)

    ui.sep()
    ui.done("worktree-ios-dev/project.toml written")
    ui.info(f"project          = {target.relative_to(root)}")
    ui.info(f"scheme           = {chosen_scheme}")
    ui.info(f"simulator_prefix = {sim_prefix}")
    if pkg_root:
        ui.info(f"packages_root    = {pkg_root.relative_to(root)}")
    for key, label in (
        ("bundle_id",       "bundle_id       "),
        ("product_name",    "product_name    "),
        ("display_name",    "display_name    "),
        ("min_ios_version", "min_ios_version "),
    ):
        if (val := app_metadata.get(key)):
            ui.info(f"{label} = {val}")
    ui.sep()
    ui.done("Next: worktree-ios-dev-tool sim pick")

    return 0
