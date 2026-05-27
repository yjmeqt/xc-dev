# Configuration files

xc-dev reads three TOML files under `.xc-dev/` at your project root. `xc-dev :proj init`
scaffolds all three.

## `project.toml`

```toml
schema_version = 1

[project]
path             = "MyApp.xcworkspace"     # or "MyApp.xcodeproj"
scheme           = "MyApp"
configuration    = "Debug"
bundle_id        = "com.example.myapp.debug"
product_name     = "MyApp Dev"
min_ios_version  = "26.0"
# Optional — override DerivedData location. Default is "{{xc_dev_dir}}/DerivedData".
# derived_data   = "/Volumes/External/DerivedData/MyApp"

[vars]
# Optional — your own custom variables, accessible as {{name}} in tasks.
xcbeautify = "xcbeautify --quiet"
```

### Fields

| Field | Type | Required | Notes |
|---|---|---|---|
| `schema_version` | u32 | yes | Currently `1`. |
| `[project].path` | string | yes | `.xcworkspace` or `.xcodeproj`, relative to project root. |
| `[project].scheme` | string | yes | Xcode scheme name. |
| `[project].configuration` | string | yes | Default configuration, e.g. `Debug`. Used when a task has no `configuration` field. |
| `[project].bundle_id` | string | yes | App bundle identifier for the default configuration. |
| `[project].product_name` | string | yes | Used to derive `{{app_path}}` for the default configuration. |
| `[project].min_ios_version` | string | yes | E.g. `26.0`. |
| `[project].derived_data` | string | no | Default: `{{xc_dev_dir}}/DerivedData` (per-worktree isolation). |
| `[vars].<key>` | string | no | User-defined variables. See [variables.md](variables.md). |

`[vars]` keys cannot collide with reserved variable names (see [variables.md](variables.md)).

### Per-configuration overrides

Xcode projects often have multiple configurations (Debug, Release, AdHoc, Beta, …) each
with different `PRODUCT_BUNDLE_IDENTIFIER`, `PRODUCT_NAME`, or deployment target. Declare
overrides under `[project.configurations.<name>]`:

```toml
[project]
scheme         = "MyApp"
configuration  = "Debug"
bundle_id      = "com.example.myapp.debug"
product_name   = "MyApp Dev"
min_ios_version = "26.0"

[project.configurations.Release]
bundle_id    = "com.example.myapp"
product_name = "MyApp"

[project.configurations.AdHoc]
bundle_id = "com.example.myapp.adhoc"
# product_name omitted — falls back to [project].product_name
```

All three override keys (`bundle_id`, `product_name`, `min_ios_version`) are optional
within each `[project.configurations.<name>]` block. Omitted keys fall back to the
corresponding `[project]` value.

`xc-dev :proj init` detects these automatically by calling `xcodebuild -showBuildSettings`
for each configuration and emitting override blocks only for values that differ from the
default configuration.

## `simulator.toml`

```toml
schema_version = 1

[sim.default]
id      = "ABC-123-…"
name    = "iPhone 17 Pro"
runtime = "iOS 26.2"

[sim.ipad]
id      = "DEF-456-…"
name    = "iPad Pro 11"
runtime = "iOS 26.2"
```

### Fields

| Field | Type | Required | Notes |
|---|---|---|---|
| `schema_version` | u32 | yes | Currently `1`. |
| `[sim.<label>].id` | string | yes | UDID from `xcrun simctl list devices`. |
| `[sim.<label>].name` | string | yes | Display name. |
| `[sim.<label>].runtime` | string | yes | E.g. `iOS 26.2`. |

`[sim.default]` is the implicit lookup for `{{sim_id}}` / `{{sim_name}}` / `{{sim_runtime}}`.
Other labels are accessed via `{{sim.<label>.<field>}}`.

xc-dev does **not** include an interactive simulator picker (yet) — populate this file
by hand (or via your own script). `xcrun simctl list devices --json` is a useful starting
point.

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
