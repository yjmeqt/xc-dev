# Task DSL

Tasks live in `.xc-dev/tasks.toml` under the `[tasks]` namespace. Each task is a TOML table.

## Minimal example

```toml
[tasks.build]
args = ["config=Debug"]
cmd  = "xcodebuild -scheme {{scheme}} -configuration {{config}} build"
```

Invoke as `xc-dev build` or `xc-dev build Release`.

## Fields

| Field | Type | Required | Default |
|---|---|---|---|
| `cmd` | string | yes | — |
| `args` | list of strings | no | `[]` |
| `depends` | list of strings | no | `[]` |
| `desc` | string | no | — |
| `cwd` | string | no | project root |
| `env` | table | no | `{}` |
| `configuration` | string | no | `[project].configuration` |

### `cmd`

Multi-line shell script. After `{{var}}` substitution, the full string is written to a
temp file and executed as `sh <tmpfile>`. Shell state (variables, `cd`, traps) **persists
across lines**, so you can write multi-step pipelines naturally:

```toml
[tasks.archive]
cmd = """
set -euo pipefail
mkdir -p {{xc_dev_dir}}/archives
xcodebuild -scheme {{scheme}} archive \\
  -archivePath {{xc_dev_dir}}/archives/{{git_branch}}-{{git_sha}}.xcarchive
"""
```

**`set -e` is NOT injected.** Add it yourself if you want fail-fast semantics. We refuse
to add magic that surprises users coming from `Makefile` or npm scripts.

The script's exit code becomes the task's exit code.

### `args`

Positional arguments. Two forms:

- `"name"` — required. Erroring if the caller omits it.
- `"name=default"` — optional. Falls back to `default` if not given.

Required args must come before optional args. Inside `cmd`, reference them as `{{name}}`.

```toml
[tasks.test-package]
args = ["package", "config=Debug"]
cmd  = "swift test --package-path Packages/{{package}} -c {{config}}"
```

Invoke:

```sh
xc-dev test-package Networking
xc-dev test-package Networking Release
xc-dev test-package             # error: expects <package> [config=Debug]
xc-dev test-package A B C       # error: got 3
```

Arg names cannot shadow reserved variable names (`scheme`, `git_branch`, etc.). The
parser rejects such tasks at startup.

### `depends`

Tasks to run **first**, in declaration order. The chain is computed via topological
sort:

- Dependencies appear before the task that depends on them.
- Diamond dependencies (`A ← B, A ← C, D ← B+C`) run `A` exactly once.
- Cycles are detected at startup (`cyclic depends chain: a -> b -> a`).

If any task in the chain exits non-zero, the chain aborts immediately and `xc-dev` exits
with that code. Subsequent tasks (including the one you invoked) do not run.

```toml
[tasks.run]
depends = ["build"]
cmd     = "xcrun simctl launch {{sim_id}} {{bundle_id}}"
```

**Args binding for dependencies:** only the user-invoked root task receives the caller's
positional args. Dependencies always bind with empty user-args and rely on their declared
defaults.

### `configuration`

The Xcode configuration to use when resolving `{{bundle_id}}`, `{{product_name}}`,
`{{configuration}}`, `{{app_path}}`, and `{{build_products_dir}}` for this task.
If omitted, the task inherits `[project].configuration`.

```toml
[tasks.archive-release]
configuration = "Release"
desc = "Archive the Release build"
cmd = """
xcodebuild -scheme {{scheme}} \
  -configuration {{configuration}} \
  archive \
  -archivePath {{xc_dev_dir}}/archives/{{git_branch}}.xcarchive
"""
```

The value is a hint for variable resolution only — it does **not** automatically pass
`-configuration Release` to `xcodebuild`. Include that flag explicitly in `cmd` using
`{{configuration}}`.

`xc-dev :doctor` emits a `WARN` (not `FAIL`) if `task.configuration` doesn't match any
name in `[project].configuration` or `[project.configurations.*]`.

### `desc`

One-line description shown by `xc-dev :list`. Optional but recommended.

```toml
[tasks.build]
desc = "Build the app into per-worktree DerivedData."
```

### `cwd`

Working directory for the shell, relative to `{{project_root}}`. Default = project root.

```toml
[tasks.format]
cwd = "ios"
cmd = "swiftformat ."
```

### `env`

Extra environment variables. Merged onto the parent process env (does not replace it).

```toml
[tasks.test]
env = { CI = "true", LOG_LEVEL = "debug" }
cmd = "xcodebuild test -scheme {{scheme}}"
```

## Template substitution

`{{name}}` is the only substitution syntax. Whitespace inside the braces is trimmed
(`{{ scheme }}` works). Dotted names like `{{sim.ipad.id}}` are supported.

There is **no shell escaping**. If a substituted value contains spaces or shell
metacharacters, you must quote it yourself:

```toml
cmd = 'xcodebuild -scheme "{{scheme}}" build'
```

Unknown variables fail the task with a clear error pointing at the task name and the
unknown name. See [`variables.md`](variables.md) for the full resolution order.

## Naming restrictions

- Task names cannot start with `:` — that prefix is reserved for built-ins (`:list`,
  `:get`, `:doctor`, `:proj init`, `:sim bake`, `:clean`).
- Task names are otherwise free-form TOML keys.

## Discovering and inspecting tasks

```sh
xc-dev               # = xc-dev :list — show all tasks with descriptions
xc-dev :doctor       # check every task's depends + variable references
xc-dev -v <task>     # echo the expanded command to stderr before running
```
