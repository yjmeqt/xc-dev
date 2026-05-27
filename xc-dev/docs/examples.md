# Example task recipes

Real-world `tasks.toml` patterns you can copy.

## Build

```toml
[tasks.build]
args = ["config=Debug"]
desc = "Build the app into per-worktree DerivedData"
cmd  = """
xcodebuild -scheme {{scheme}} \\
  -configuration {{config}} \\
  -derivedDataPath {{derived_data}} \\
  build
"""
```

## Build with xcbeautify

```toml
[vars]                   # in project.toml
xcbeautify = "xcbeautify --quiet"

# in tasks.toml
[tasks.build]
args = ["config=Debug"]
cmd  = "set -o pipefail; xcodebuild -scheme {{scheme}} -configuration {{config}} build | {{xcbeautify}}"
```

## Run (boot + install + launch)

With a self-resolving simulator (recommended — `{{sim_id}}` triggers UDID resolution on
first use):

```toml
# simulator.toml
[sim.default]
name    = "xc-dev-default"
device  = "iPhone 16"
runtime = "iOS 26.2"

# tasks.toml
[tasks.run]
depends = ["build"]
desc    = "Build then boot the default sim, install, and launch"
cmd     = """
xcrun simctl boot {{sim_id}} 2>/dev/null || true
xcrun simctl install {{sim_id}} {{app_path}}
xcrun simctl launch {{sim_id}} {{bundle_id}}
"""
```

With a pinned UDID (when you need a specific device):

```toml
# simulator.toml
[sim.default]
id      = "ABC-123-…"
name    = "iPhone 17 Pro"
runtime = "iOS 26.2"
```

## Run on a specific sim label

```toml
[tasks.run-ipad]
depends = ["build"]
cmd     = """
xcrun simctl boot {{sim.ipad.id}} 2>/dev/null || true
xcrun simctl install {{sim.ipad.id}} {{app_path}}
xcrun simctl launch {{sim.ipad.id}} {{bundle_id}}
"""
```

## Per-worktree simulator via git_worktree_name

Each git worktree gets its own simulator automatically:

```toml
# simulator.toml
[sim.default]
name    = "{{git_worktree_name}}-sim"
device  = "iPhone 16"
runtime = "iOS 26.2"
```

When `{{sim_id}}` is first referenced in a worktree named `feat-login`, xc-dev looks for
(or creates) a simulator named `feat-login-sim`. Different worktrees get different
simulators, so they can run in parallel without boot conflicts.

## Test (xcodebuild)

```toml
[tasks.test]
cmd = """
xcodebuild test \\
  -scheme {{scheme}} \\
  -destination 'id={{sim_id}}' \\
  -derivedDataPath {{derived_data}}
"""
```

## Test a SwiftPM package

```toml
[tasks.test-package]
args = ["package"]
desc = "Run unit tests for a local SwiftPM package under Packages/"
cmd  = "swift test --package-path Packages/{{package}}"
```

Invoke: `xc-dev test-package Networking`.

## Archive with branch + sha in the path

```toml
[tasks.archive]
desc = "Archive into .xc-dev/archives/<branch>-<sha>.xcarchive"
cmd  = """
set -euo pipefail
mkdir -p {{xc_dev_dir}}/archives
xcodebuild -scheme {{scheme}} archive \\
  -archivePath {{xc_dev_dir}}/archives/{{git_branch}}-{{git_sha}}.xcarchive
"""
```

## Archive with per-configuration overrides (Release)

Use `configuration = "Release"` on the task so that `{{bundle_id}}`, `{{product_name}}`,
and `{{app_path}}` all resolve from `[project.configurations.Release]` automatically.

```toml
# project.toml
[project]
scheme         = "MyApp"
configuration  = "Debug"
bundle_id      = "com.example.myapp.debug"
product_name   = "MyApp Dev"
min_ios_version = "26.0"

[project.configurations.Release]
bundle_id    = "com.example.myapp"
product_name = "MyApp"

# tasks.toml
[tasks.archive-release]
configuration = "Release"
desc = "Archive the Release build with the correct bundle_id"
cmd  = """
set -euo pipefail
mkdir -p {{xc_dev_dir}}/archives
xcodebuild -scheme {{scheme}} \\
  -configuration {{configuration}} \\
  -derivedDataPath {{derived_data}} \\
  archive \\
  -archivePath {{xc_dev_dir}}/archives/{{git_branch}}-{{git_sha}}.xcarchive
"""
```

`{{bundle_id}}` resolves to `com.example.myapp` (from the Release override), and
`{{app_path}}` points to `…/Release-iphonesimulator/MyApp.app`.

## Log stream from the simulator

```toml
[tasks.log]
desc = "Stream OS log from {{bundle_id}}"
cmd  = """
xcrun simctl spawn {{sim_id}} log stream \\
  --predicate 'subsystem == "{{bundle_id}}"' --style compact
"""
```

## Clean DerivedData

```toml
[tasks.clean]
desc = "Wipe the per-worktree DerivedData directory"
cmd  = "rm -rf {{derived_data}}"
```

## CI: build, test, archive

```toml
[tasks.ci]
depends = ["build", "test", "archive"]
desc    = "Aggregate task used in CI"
cmd     = "echo 'CI pipeline complete on {{git_branch}} ({{git_sha}})'"
```

`depends` runs `build` → `test` → `archive` sequentially; the chain aborts on first failure.

## Using `:get` from shell scripts

```sh
#!/usr/bin/env bash
set -euo pipefail
SCHEME=$(xc-dev :get scheme)
SIM=$(xc-dev :get sim_id)
echo "scheme=$SCHEME sim=$SIM"
```

## Per-worktree caches via `{{git_worktree_name}}`

```toml
[tasks.cache]
desc = "Snapshot the build output into a worktree-specific cache dir"
cmd  = """
mkdir -p ~/Library/Caches/xc-dev/{{git_worktree_name}}
cp -R {{app_path}} ~/Library/Caches/xc-dev/{{git_worktree_name}}/
"""
```
