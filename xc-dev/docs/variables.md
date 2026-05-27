# Variables

Tasks reference variables via `{{name}}` substitution. The engine resolves names by walking
several layers in a fixed order; this page documents every source.

## Resolution order

When you write `{{foo}}` in a task's `cmd`, xc-dev tries these layers, in order, and stops
at the first match:

1. **Args** — positional args passed by the caller, after binding to the task's
   `args = [...]` declaration. (Highest priority within a task.)
2. **`[vars]`** — user-defined keys in `project.toml`.
3. **Simulator shortcuts** — `sim_id`, `sim_name`, `sim_runtime` resolve to `[sim.default]`.
4. **Simulator qualified** — `sim.<label>.<field>` for non-default labels.
5. **`[project]` fields** — `scheme`, `bundle_id`, `product_name`, `configuration`,
   `path`. For `bundle_id`, `product_name`, and `configuration`, the resolved value is
   the **effective configuration** for the task (see below), not always the literal
   `[project]` value.
6. **Automatic variables** — listed below.
7. **Git variables** — listed below; lazy-evaluated and cached.

A name that no layer provides is a `XcDevError::UnknownVar` at substitution time.

## Reserved names

The following names are reserved by xc-dev. You cannot use them as:

- Keys in `[project]` (the schema rejects unknown fields anyway).
- Keys in `[vars]`.
- Names in any task's `args` list.

Doing so is a startup error (`reserved name X cannot be used as Y`).

This guarantees that `{{scheme}}`, `{{git_branch}}`, etc. always mean the same thing
regardless of which layer the engine consulted.

Reserved set:

```
project_root  xc_dev_dir  derived_data  build_products_dir  app_path
date  timestamp  user  home
git_root  git_branch  git_sha  git_worktree_dir  git_worktree_name  git_main_root
sim_id  sim_name  sim_runtime
path  scheme  configuration  bundle_id  product_name
```

## Automatic variables

These are computed from the project state and environment. No git involvement.

| Variable | Value |
|---|---|
| `{{project_root}}` | Absolute path of the directory containing `.xc-dev/`. |
| `{{xc_dev_dir}}` | `{{project_root}}/.xc-dev` |
| `{{derived_data}}` | `[project].derived_data` if set; otherwise `{{xc_dev_dir}}/DerivedData`. Per-worktree isolation by default. |
| `{{build_products_dir}}` | `{{derived_data}}/Build/Products/{{configuration}}-iphonesimulator` |
| `{{app_path}}` | `{{build_products_dir}}/{{product_name}}.app` |
| `{{date}}` | `YYYY-MM-DD`, local timezone, captured at process start. |
| `{{timestamp}}` | Unix seconds at process start. |
| `{{user}}` | `$USER` |
| `{{home}}` | `$HOME` |

### Customizing `{{derived_data}}`

Per-worktree DerivedData is the default for a reason: parallel branches in git worktrees
no longer trample each other's incremental builds. To override:

```toml
[project]
derived_data = "/Volumes/External/DerivedData/MyApp"
```

## Git variables

Evaluated lazily — only the variables actually referenced in a task's rendered `cmd` cause
`git` subprocesses to run. Results (success and failure) are cached for the lifetime of
the `xc-dev` invocation, so referencing `{{git_branch}}` five times in one task only
shells out once.

| Variable | Source command |
|---|---|
| `{{git_root}}` | `git rev-parse --show-toplevel` |
| `{{git_branch}}` | `git rev-parse --abbrev-ref HEAD` |
| `{{git_sha}}` | `git rev-parse --short HEAD` |
| `{{git_worktree_dir}}` | `git rev-parse --show-toplevel` (same as `git_root` in normal clones) |
| `{{git_worktree_name}}` | `basename({{git_worktree_dir}})` |
| `{{git_main_root}}` | First `worktree …` line from `git worktree list --porcelain` |

Referencing a `git_*` variable outside a git repository (or when `git` exits non-zero)
fails with `variable X unavailable: <git error>`. Tasks that don't reference git vars are
unaffected — you can use xc-dev in non-git directories.

## Configuration-dependent variables and effective_config

Three system variables — `{{bundle_id}}`, `{{product_name}}`, and `{{configuration}}` —
are **effective-configuration-aware**:

```
effective_config = task.configuration  (if set)
                  OR  project.configuration  (fallback)
```

The engine consults `[project.configurations.<effective_config>]` first. If a value is
present in that override block it wins; otherwise the `[project]` field is used.
`{{configuration}}` resolves to `effective_config`, not always the literal
`[project].configuration`.

Automatic path variables that include the configuration or product name
(`{{build_products_dir}}`, `{{app_path}}`) also use the effective values, so they
point to the correct build output directory for the task's configuration.

Example — a task that archives the Release build:

```toml
[tasks.archive-release]
configuration = "Release"
cmd = """
xcodebuild -scheme {{scheme}} \
  -configuration {{configuration}} \
  archive \
  -archivePath {{xc_dev_dir}}/archives/{{git_branch}}.xcarchive
"""
```

When this task runs, `{{bundle_id}}` resolves from
`[project.configurations.Release].bundle_id` (if present), and `{{app_path}}` points
to `…/Release-iphonesimulator/MyApp.app`.

## Project fields

The fields under `[project]` are exposed as flat top-level names. Example:

```toml
[project]
path             = "MyApp.xcworkspace"
scheme           = "MyApp"
configuration    = "Debug"
bundle_id        = "com.example.myapp"
product_name     = "MyApp"
```

becomes available as `{{path}}`, `{{scheme}}`, `{{configuration}}`, `{{bundle_id}}`,
`{{product_name}}`.

## Simulator variables

`simulator.toml` declares one or more labeled simulators. The `[sim.default]` block is the
implicit target for the shorthand variables.

Access:

- `{{sim_id}}` / `{{sim_name}}` / `{{sim_runtime}}` — the default sim (`[sim.default]`).
- `{{sim.<label>.<field>}}` — any sim, any field. Example: `{{sim.ipad.id}}`.

If `[sim.default]` isn't defined and you reference `{{sim_id}}`, you'll get `UnknownVar`.

### Self-resolving simulators (lazy — triggered only by `{{sim_id}}`)

The recommended approach: set `device` + `runtime` and let xc-dev manage the UDID.

```toml
[sim.default]
name    = "xc-dev-default"
device  = "iPhone 16"
runtime = "iOS 26.2"
```

`{{sim_id}}` triggers resolution:
1. Load cache from `.xc-dev/.sim-cache.json` (per-machine, gitignored).
2. Validate cached UDIDs against `xcrun simctl list devices --json`; drop stale entries.
3. If a cache hit exists, return it immediately.
4. Otherwise look for an existing simulator with matching `name` + `runtime`.
5. If none found, create one via `xcrun simctl create <name> <device-type> <runtime>`.
6. Write the UDID to cache.

`{{sim_name}}` and `{{sim_runtime}}` do **not** trigger any simctl calls — they return the
literal template values (after template rendering, see below).

The `name` value may contain template variables, e.g.:

```toml
name = "{{git_worktree_name}}-sim"
```

Variables whose names start with `sim` are excluded from this rendering step to prevent
recursion. All other variables (git vars, project fields, auto vars) are available.

### Pinned UDID

When `id` is set it is used directly — no simctl calls, no cache.

```toml
[sim.default]
id      = "ABC-123-XXXX-…"
name    = "iPhone 17 Pro"
runtime = "iOS 26.2"
```

### Doctor validation

`xc-dev :doctor` emits `WARN sim.<label>: neither id nor device is set` for entries that
would fail at runtime if `{{sim_id}}` were referenced.

## Custom `[vars]`

For values you don't want to hard-code into every task:

```toml
[vars]
xcbeautify = "xcbeautify --quiet"
team_id    = "ABCDE12345"
```

Used in tasks like:

```toml
[tasks.build]
cmd = "xcodebuild ... build | {{xcbeautify}}"
```

`[vars]` keys must not collide with any reserved name (see above).

## Inspecting variables from the shell

The `:get` built-in resolves and prints a single variable. Useful for ad-hoc scripts and
CI:

```sh
SCHEME=$(xc-dev :get scheme)
xc-dev :get derived_data
xc-dev :get sim.ipad.id
xc-dev :get git_branch
```

Unknown variables exit non-zero.
