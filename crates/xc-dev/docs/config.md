# Configuration files

xc-dev reads three TOML files under `.xc-dev/` at your project root. `xc-dev :proj init`
scaffolds all three.

## `project.toml`

### Single configuration (minimal)

```toml
schema_version = 1

[project]
path          = "MyApp.xcworkspace"     # or "MyApp.xcodeproj"
scheme        = "MyApp"
configuration = "Debug"               # default; can be overridden per-task
# Optional — override DerivedData location. Default is "{{xc_dev_dir}}/DerivedData".
# derived_data = "/Volumes/External/DerivedData/MyApp"

[project.configurations.Debug]
bundle_id    = "com.example.myapp.debug"
product_name = "MyApp Dev"

[vars]
# Optional — your own custom variables, accessible as {{name}} in tasks.
xcbeautify = "xcbeautify --quiet"
```

### Multiple configurations

```toml
schema_version = 1

[project]
path          = "MyApp.xcworkspace"
scheme        = "MyApp"
configuration = "Debug"

[project.configurations.Debug]
bundle_id    = "com.example.myapp.debug"
product_name = "MyApp Dev"

[project.configurations.Release]
bundle_id    = "com.example.myapp"
product_name = "MyApp"

[project.configurations.AdHoc]
bundle_id    = "com.example.myapp.adhoc"
product_name = "MyApp"
```

Every configuration that you reference (directly or via a task's `configuration` field)
**must** have its own `[project.configurations.<name>]` block with both `bundle_id` and
`product_name`. There is no fallback — missing blocks are a startup error.

`xc-dev :proj init` detects all configurations via `xcodebuild -list -json` and emits a
full block for each one automatically.

### Fields

| Field | Type | Required | Notes |
|---|---|---|---|
| `schema_version` | u32 | yes | Currently `1`. |
| `[project].path` | string | yes | `.xcworkspace` or `.xcodeproj`, relative to project root. |
| `[project].scheme` | string | yes | Xcode scheme name. |
| `[project].configuration` | string | yes | Default configuration, e.g. `Debug`. Used when a task has no `configuration` field. |
| `[project].derived_data` | string | no | Default: `{{xc_dev_dir}}/DerivedData` (per-worktree isolation). |
| `[project.configurations.<name>].bundle_id` | string | yes | App bundle identifier for this configuration. |
| `[project.configurations.<name>].product_name` | string | yes | Used to derive `{{app_path}}` for this configuration. |
| `[vars].<key>` | string, integer, or boolean | no | User-defined flat variable. See [variables.md](variables.md). |
| `[vars.<group>.<key>]` | table | no | Nested group; flattened to `group.key.*` dot-separated names. See [variables.md](variables.md). |

A `[project.configurations.<name>]` block is **required** for every configuration name
used — including `[project].configuration` (the default). `[vars]` keys cannot collide
with reserved variable names (see [variables.md](variables.md)).

## `simulator.toml`

xc-dev supports two modes for simulator entries: **self-resolving** (recommended) and
**pinned UDID** (for full control).

### Self-resolving (recommended)

```toml
schema_version = 1

[sim.default]
name    = "xc-dev-default"
device  = "iPhone 16"
runtime = "iOS 26.2"

[sim.ipad]
name    = "xc-dev-ipad"
device  = "iPad Pro 11-inch (M4)"
runtime = "iOS 26.2"
```

When `{{sim_id}}` is first used, xc-dev:
1. Looks for an existing simulator matching `name` + `runtime` via `xcrun simctl list devices`.
2. If none is found, creates one via `xcrun simctl create <name> <device-type-id> <runtime-id>`.
3. Caches the resolved UDID in `.xc-dev/simulator.local.toml` (gitignored, per-machine).

On subsequent calls the cached UDID is validated against `xcrun simctl list devices` and
reused. Stale entries (e.g. after the simulator was deleted) are dropped and re-resolved.

**Opt-in alternative: `:sim bake`** — for teams who prefer to check the resolved UDID into
source control, run `xc-dev :sim bake` once. It resolves all `[sim.<label>]` entries that
don't have an `id` and writes `id = "<udid>"` directly into `simulator.toml` (in-place,
preserving comments and formatting). After baking, `simulator.local.toml` is no longer needed
for those entries.

`name` may contain template variables (e.g. `"{{git_worktree_name}}-sim"`); variables
starting with `sim` are excluded to prevent recursion.

### Pinned UDID

```toml
schema_version = 1

[sim.default]
id      = "ABC-123-XXXX-…"
name    = "iPhone 17 Pro"
runtime = "iOS 26.2"
```

When `id` is present it is used directly — no simctl calls are made.

### Fields

| Field | Type | Required | Notes |
|---|---|---|---|
| `schema_version` | u32 | yes | Currently `1`. |
| `[sim.<label>].name` | string | yes | Display name (also used as the simctl device name for self-resolving entries). |
| `[sim.<label>].runtime` | string | yes | E.g. `iOS 26.2`. |
| `[sim.<label>].id` | string | no | Explicit UDID. When present, `device` is ignored and no simctl calls are made. |
| `[sim.<label>].device` | string | no | Device type for auto-resolution, e.g. `"iPhone 16"`. Required when `id` is absent and `{{sim_id}}` is used. |

`[sim.default]` is the implicit lookup for `{{sim_id}}` / `{{sim_name}}` / `{{sim_runtime}}`.
Other labels are accessed via `{{sim.<label>.<field>}}`.

`xc-dev :proj init` scaffolds a self-resolving `[sim.default]` with `device = "iPhone 16"`.
Run `xcrun simctl list devicetypes` to see all valid device type names.
Run `xcrun simctl list runtimes` to see installed runtimes.

The per-machine UDID cache lives in `.xc-dev/simulator.local.toml` (gitignored). Its format
mirrors `simulator.toml` but only contains the `id` field per entry:

```toml
# .xc-dev/simulator.local.toml
# Auto-managed by xc-dev. Per-machine UDID cache. Gitignored.
# Do not edit by hand.

[sim.default]
id = "ABC-DEF-..."

[sim.ipad]
id = "GHI-JKL-..."
```

## `tasks.toml`

See [tasks.md](tasks.md) for the full Task DSL. Minimal form:

```toml
[tasks.build]
args = ["config=Debug"]
desc = "Build the app"
cmd  = "xcodebuild -scheme {{scheme}} -configuration {{config}} build"
```

## Where xc-dev looks

`xc-dev` walks up from the current directory looking for a `.xc-dev/` directory (similar
to git's `.git/` discovery). All three TOMLs must live inside it. If no `.xc-dev/` is
found, `xc-dev` exits with an error suggesting `xc-dev :proj init`.

## Schema evolution

`schema_version = 1` is the only version that exists today. Future breaking changes will
bump the number; xc-dev will refuse to load files with an unknown version.
