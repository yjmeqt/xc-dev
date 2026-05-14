# worktree-ios-dev-tool

Per-worktree iOS build / simulator / test CLI. Each git worktree gets its own simulator(s) and derived-data directory so parallel feature branches never collide.

## Install

Once per machine, from GitHub:

```bash
uv tool install "git+https://github.com/yjmeqt/worktree-ios-dev-tool.git#subdirectory=worktree_ios_dev_tool"
```

Upgrade to the latest `main` (`uv upgrade` caches the resolved commit — use `--reinstall`):

```bash
uv tool install --reinstall "git+https://github.com/yjmeqt/worktree-ios-dev-tool.git#subdirectory=worktree_ios_dev_tool"
```

Local development (editable, picks up source edits automatically):

```bash
uv tool install --editable /abs/path/to/worktree-ios-dev-tool/worktree_ios_dev_tool
```

Requires `uv` and `~/.local/bin` on `PATH`. Optional: `brew install mint` enables `xcbeautify` for prettier build output.

## Per-worktree setup

Run once from anywhere inside the worktree:

```bash
worktree-ios-dev-tool proj init   # scaffold worktree-ios-dev/, write project.toml, update .gitignore
worktree-ios-dev-tool sim pick    # create + boot the per-worktree simulator
```

`proj init` discovers the Xcode project and scheme interactively, then writes `worktree-ios-dev/project.toml`. `sim pick` creates a simulator named `<simulator_prefix>-<worktree_basename>-<label>` (default label: `default`) and boots it; the resulting entry lands in `worktree-ios-dev/simulator.toml`.

Interactive output uses a clack-style step format:

```
◇  Project: ios/App.xcodeproj
⠋  Fetching schemes...
◇  Scheme: App
│
◆  worktree-ios-dev/project.toml written
│  project          = ios/App.xcodeproj
│  scheme           = App
│  simulator_prefix = App
│
◆  Next: worktree-ios-dev-tool sim pick
```

Non-interactive (agent / non-TTY) output uses flat `[worktree-ios-dev-tool] <msg>` lines instead.

## Multiple simulators per worktree

For peer-to-peer scenarios — two simulators in the same worktree exchanging messages, for example — register additional sims under their own labels:

```bash
worktree-ios-dev-tool sim pick peer
worktree-ios-dev-tool run --sim peer
worktree-ios-dev-tool test --sim default
```

`--sim <label>` is required only when multiple entries exist under `[simulators.*]`. Single-sim setups inherit the only entry automatically.

Each sim's simctl device name is `<simulator_prefix>-<worktree_basename>-<label>`, so `sim cleanup` / `sim du` / `sim prune` can identify them by prefix scan without a separate registry file.

## Verb reference

### `proj` (project lifecycle)

| Verb | Description |
|---|---|
| `proj init` | Scaffold `worktree-ios-dev/` and seed `project.toml`. Idempotent; `--force` re-seeds without touching `simulator.toml`. |
| `proj config` | Print the resolved project + simulators view as JSON. Useful for debugging. |
| `proj doctor` | Sanity-check tooling (`xcodebuild`/`xcrun`/`mint`), `project.toml`, simulators, project path. Missing `simulator.toml` is a warn — fresh worktrees still pass. |

### `sim` (simulator lifecycle)

| Verb | Description |
|---|---|
| `sim pick [<label>]` | Pick + create + boot a simulator. Interactive on TTY, auto-selects first device/runtime otherwise. Use `--device` / `--runtime` for explicit choice. `<label>` defaults to `default`. Errors if it already exists; use `sim recreate <label>` to replace. `--all-devices` disables the iPhone 17 Pro filter. |
| `sim boot [<label>]` | Boot a configured sim. Single-sim setups can omit `<label>`; multi-sim must pass one or `--all`. |
| `sim shutdown [<label>]` | Same shape as `boot` but shuts down. |
| `sim list` | List this worktree's configured sims with simctl state. `--global` scans every `<simulator_prefix>-*` device on the machine, grouped by parsed worktree. |
| `sim recreate <label>` | Destroy + re-pick + re-boot. Always requires explicit label (no defaulting on destructive ops). |
| `sim remove <label>` | Drop the entry from `simulator.toml`. Default keeps the simctl device; `--destroy` deletes it too. |
| `sim cleanup` | Tear down every simulator owned by this worktree (prefix scan via simctl) and delete `simulator.toml`. Run before `git worktree remove`. `--yes` skips confirmation. |
| `sim du [--this-worktree]` | Disk-usage report for managed simulators, grouped by worktree. Default scans globally; `--this-worktree` restricts to the current basename. |
| `sim prune` | Find managed sims whose worktrees are gone (via `git worktree list`) and delete them. `--yes` skips confirmation. |

### Build verbs (top-level)

| Verb | Description |
|---|---|
| `build` | `xcodebuild build`. `--release`, `--scheme <name>`, `--sim <label>`. |
| `test` | `xcodebuild test`. `--release`, `--scheme <name>`, `--only-testing`, `--skip-testing`, `--sim <label>`. |
| `run` | Build → install → launch on the chosen simulator. `--release`, `--sim <label>`. Prints bundle id on success. |
| `clean` | `xcodebuild clean`. |
| `wipe-derived` | Delete `worktree-ios-dev/derivedData`. Prompts unless `--yes`. |
| `test-package <Name>` | `xcodebuild test` a local Swift package under `packages_root`. `--sim <label>`. |

## Global flags

- `--project-toml <path>` — override walk-up `project.toml` discovery. `simulator.toml` is resolved alongside it.
- `-v` / `--verbose` — stream subprocess output and show tracebacks on error.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | OK |
| `1` | User error (bad args, bad config, refused verb) |
| `2` | Environment error (project.toml not found, missing toolchain) |
| `3` | Subprocess failure (xcodebuild / simctl non-zero) |

## Agent (non-interactive) usage

The tool detects a non-TTY environment automatically and switches to flat `[worktree-ios-dev-tool] <msg>` output instead of the clack-style step format.

For `proj init`, supply `--project` and `--scheme` explicitly — without them, ambiguity causes an immediate error exit. Add `--yes` to suppress any remaining prompts:

```bash
worktree-ios-dev-tool proj init \
  --project ios/App.xcodeproj \
  --scheme App \
  --yes
```

`sim pick` auto-selects the first available device and latest runtime when stdin is not a TTY. Use `--device` / `--runtime` to pick explicitly (e.g. `--device "iPhone 17 Pro" --runtime "iOS 26.2"`); pass `--all-devices` if the default iPhone 17 Pro filter is too narrow. `sim cleanup` and `sim prune` require `--yes` outside a TTY (they're destructive and refuse to act silently).

## Cleanup and disk usage

When you're done with a feature branch, before `git worktree remove`:

```bash
worktree-ios-dev-tool sim cleanup --yes
```

This shuts down + deletes every simulator named `<simulator_prefix>-<this_worktree_basename>-*` and removes `simulator.toml`. It works even if the toml is missing — the scan is name-based.

To survey disk usage across all worktrees managed by this tool:

```bash
worktree-ios-dev-tool sim du
```

Output is grouped by worktree basename, with a total at the end. To find sims whose worktrees no longer exist (e.g. you ran `git worktree remove` without `sim cleanup` first):

```bash
worktree-ios-dev-tool sim prune
```

`prune` uses `git worktree list` from the current worktree to discover what's live; sims encoding any other basename are flagged as orphans.

## Why is `proj init` slow the first time?

Scheme discovery runs `xcodebuild -list -project ... -json`, which triggers Xcode to parse the project on cold start — expect 10–30 seconds on a large project. Subsequent runs are faster due to Xcode's internal cache.

Skip it entirely by passing `--scheme` directly:

```bash
worktree-ios-dev-tool proj init --scheme App
```

## Config files

`worktree-ios-dev/` (gitignored) holds two files. Both carry `schema_version = 1`.

`project.toml` (written by `proj init`, safe to hand-edit):

```toml
schema_version = 1

[project]
path             = "ios/App.xcodeproj"
scheme           = "App"
configuration    = "Debug"
simulator_prefix = "App"

[packages_root]
path = "ios/Packages"

[extras]
xcodebuild_flags = []
```

`simulator.toml` (written by `sim pick`/`sim recreate`/`sim remove`; do not hand-edit `udid` or `name`):

```toml
schema_version = 1

[simulators.default]
name    = "App-feat-auth-default"
udid    = "ABCD-..."
device  = "iPhone 17 Pro"
runtime = "iOS 18.2"

[simulators.peer]
name    = "App-feat-auth-peer"
udid    = "EFGH-..."
device  = "iPhone 17 Pro"
runtime = "iOS 18.2"
```

Edit `project.toml` by hand or re-run `proj init --force` to regenerate it. To replace a sim entry, run `sim recreate <label>` rather than editing `simulator.toml` directly.

## Migrating from the legacy `config.toml`

Pre-2026-05-05 worktrees use a single `worktree-ios-dev/config.toml` with combined `[project]` + `[simulator]`. The current tool refuses to operate on those — the discovery walk-up raises `UserError` listing the manual migration:

1. Read the legacy values:    `cat worktree-ios-dev/config.toml`
2. Write the new project.toml: `worktree-ios-dev-tool proj init --force --project <relpath> --scheme <name>`
3. Recreate the simulator:     `worktree-ios-dev-tool sim pick`
4. Remove the legacy file:     `rm worktree-ios-dev/config.toml`

There is no auto-conversion. Each worktree needs to do this once.
