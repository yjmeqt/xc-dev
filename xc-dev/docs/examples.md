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

```toml
[tasks.run]
depends = ["build"]
desc    = "Build then boot the default sim, install, and launch"
cmd     = """
xcrun simctl boot {{sim_id}} 2>/dev/null || true
xcrun simctl install {{sim_id}} {{app_path}}
xcrun simctl launch {{sim_id}} {{bundle_id}}
"""
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
