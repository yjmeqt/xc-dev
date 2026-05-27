# iOS dev tooling

xc-dev — a `package.json`-style task runner for Xcode projects. Declare your own
`build`, `run`, `test`, `test-package`, … tasks in `.xc-dev/tasks.toml`; xc-dev
injects scheme, bundle id, simulator id, derived data path, git branch, etc. as
`{{name}}` substitutions.

## Install

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
