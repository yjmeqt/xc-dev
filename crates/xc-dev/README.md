# xc-dev

A task runner for Xcode projects. Declare `build`, `run`, `test`, `test-package`, … in
`.xc-dev/tasks.toml`; xc-dev injects `scheme`, `bundle_id`, `sim_id`, `derived_data`,
`git_branch`, and friends into your commands.

Think `package.json` scripts or a `Justfile`, but Xcode-aware out of the box.

**Built for git worktrees.** Each iOS checkout typically leaves a 10GB+ DerivedData
folder under `~/Library/Developer/Xcode/DerivedData` that survives after you delete
the worktree. xc-dev keeps a per-worktree cache in `.xc-dev/DerivedData` and
`xc-dev :clean` reclaims both that cache and matching system leftovers — so you can
run many feature branches without the disk filling up.

## Install

**The binary:**

```sh
# From source (recommended during early development)
cargo install --git https://github.com/yjmeqt/worktree-ios-dev-tool xc-dev

# From a local clone
cd xc-dev && cargo install --path .

# Future (once published)
brew install yjmeqt/tap/xc-dev
```

Requires Rust 1.75+.

**The Claude skill** (so Claude Code uses `xc-dev` instead of raw `xcodebuild`):

```sh
npx skills add yjmeqt/worktree-ios-dev-tool
```

## 60-second quickstart

```sh
cd path/to/MyApp                 # contains MyApp.xcworkspace or MyApp.xcodeproj
xc-dev :proj init                # writes .xc-dev/{project,simulator,tasks}.toml
$EDITOR .xc-dev/simulator.toml   # paste in a simulator id from `xcrun simctl list devices`
xc-dev                           # = xc-dev :list — see available tasks
xc-dev build                     # runs the generated build task
xc-dev build Release             # positional arg overrides the default
xc-dev -v build                  # echo expanded command to stderr
```

## Concepts

`.xc-dev/` at your project root holds three files:

- **`project.toml`** — scheme, bundle id, product name, optional `[vars]` for custom values.
- **`simulator.toml`** — labeled simulators (`[sim.default]`, `[sim.ipad]`, …).
- **`tasks.toml`** — your `build`, `run`, `test`, … task definitions.

Tasks reference parameters via `{{name}}` substitution:

```toml
[tasks.build]
args = ["config=Debug"]
cmd  = "xcodebuild -scheme {{scheme}} -configuration {{config}} -derivedDataPath {{derived_data}} build"

[tasks.test-package]
args = ["package"]
cmd  = "swift test --package-path Packages/{{package}}"
```

Call them:

```sh
xc-dev build
xc-dev build Release
xc-dev test-package Networking
```

## Built-in verbs

All built-ins are `:`-prefixed (they never collide with user task names):

| Command | What |
|---|---|
| `xc-dev :proj init` | Scaffold `.xc-dev/` (autodetects `scheme` etc. via `xcodebuild`) |
| `xc-dev :list` | List all tasks with their descriptions (default if no args) |
| `xc-dev :get <key>` | Print one variable (for shell substitution / CI) |
| `xc-dev :doctor` | Sanity-check the three TOMLs, `xcodebuild`, and every task's variable references |
| `xc-dev :clean` | Dry-run: list local + system DerivedData for this project root |
| `xc-dev :clean --yes` | Delete them (includes Packages under the root; skips `*.noindex`) |

## Built-in variables

A small set of variables are always available (lazy-evaluated where they call out to git):

`{{project_root}}`, `{{xc_dev_dir}}`, `{{derived_data}}`, `{{build_products_dir}}`,
`{{app_path}}`, `{{date}}`, `{{timestamp}}`, `{{user}}`, `{{home}}`,
`{{git_root}}`, `{{git_branch}}`, `{{git_sha}}`, `{{git_worktree_dir}}`,
`{{git_worktree_name}}`, `{{git_main_root}}`,
`{{sim_id}}`, `{{sim_name}}`, `{{sim_runtime}}` (default sim — use `{{sim.<label>.id}}` for others),
plus every field in `[project]` and every key in `[vars]`.

See [`docs/variables.md`](docs/variables.md) for the full list and resolution order.

## Docs

- [`docs/tasks.md`](docs/tasks.md) — Task DSL reference
- [`docs/variables.md`](docs/variables.md) — All built-in variables
- [`docs/config.md`](docs/config.md) — `project.toml` / `simulator.toml` schemas
- [`docs/examples.md`](docs/examples.md) — Real-world task recipes

## License

MIT.
