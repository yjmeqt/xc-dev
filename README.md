# iOS dev tooling

xc-dev — a `package.json`-style task runner for Xcode projects. Declare your own
`build`, `run`, `test`, `test-package`, … tasks in `.xc-dev/tasks.toml`; xc-dev
injects scheme, bundle id, simulator id, derived data path, git branch, etc. as
`{{name}}` substitutions.

## Save disk space with git worktrees

Git worktrees are cheap. iOS builds are not. Each checkout that hits Xcode
leaves a **10GB+** tree under `~/Library/Developer/Xcode/DerivedData`. Parallel
tests then clone the destination sim into `~/Library/Developer/XCTestDevices`
(`Clone N of …`) at another few GB each. Both survive after you delete the
worktree. A handful of feature branches can eat hundreds of gigabytes.

xc-dev is built for that workflow:

- **Per-worktree build cache** — `{{derived_data}}` defaults to
  `.xc-dev/DerivedData` in this checkout, so parallel branches don't trample
  each other's incremental builds.
- **Reclaim the leftovers** — `xc-dev :clean` lists (and `--yes` deletes) this
  worktree's local cache *and* any system DerivedData whose `WorkspacePath`
  belongs here. Tear down a feature branch without leaving 10GB behind.
- **Worktree-tagged simulators (and their XCTest clones)** — each checkout can
  own a sim (`{{product_name}}-{{git_worktree_name}}`). XCTest copies inherit
  that name (`Clone 2 of Pulse-feat-login`), so leftover clones stay
  attributable to the worktree instead of becoming anonymous gigabytes.
  `clean` can delete the parent sim too.

## Install

**Prebuilt binary (macOS, Apple Silicon)** — v0.0.1:

```sh
curl -L https://github.com/yjmeqt/xc-dev/releases/download/v0.0.1/xc-dev-v0.0.1-aarch64-apple-darwin.tar.gz | tar xz
sudo mv xc-dev-v0.0.1-aarch64-apple-darwin/xc-dev /usr/local/bin/
```

**From source** (any platform with a Rust toolchain):

```sh
cargo install --git https://github.com/yjmeqt/xc-dev xc-dev
# or from a local clone:
cargo install --path crates/xc-dev
```

## Install the Claude skill

```sh
npx skills add yjmeqt/xc-dev
```

See [`crates/xc-dev/README.md`](crates/xc-dev/README.md) for the quickstart and docs.
