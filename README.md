# iOS dev tooling

A repository for our iOS dev-time tooling. Two tools live here:

- **[`xc-dev/`](xc-dev/)** — Rust-based task runner for Xcode projects.
  Active development; new projects should use this.
- **[`worktree_ios_dev_tool/`](worktree_ios_dev_tool/)** — Legacy Python CLI with fixed
  verbs (`proj`, `sim`, `build`, `run`, `test`). Deprecated; kept for projects that
  haven't migrated to xc-dev yet.

## xc-dev

A `package.json`-style task runner for Xcode. You declare your own `build`, `run`,
`test`, `test-package`, … tasks in `.xc-dev/tasks.toml`; xc-dev injects scheme, bundle
id, simulator id, derived data path, git branch, etc. as `{{name}}` substitutions.

Quick install:

```sh
cargo install --git https://github.com/yjmeqt/xc-dev
# or from a local clone:
cd xc-dev && cargo install --path .
```

See [`xc-dev/README.md`](xc-dev/README.md) for the quickstart and docs.

## Legacy: worktree_ios_dev_tool

The Python predecessor. Same domain (iOS / Xcode / simulator workflows) but with
hard-coded verbs. New work happens in `xc-dev/`; this directory is preserved for
projects that haven't migrated.

See [`worktree_ios_dev_tool/README.md`](worktree_ios_dev_tool/README.md) for usage.

## Spec / plan

- Design: [`docs/superpowers/specs/2026-05-26-xc-dev-v2-design.md`](docs/superpowers/specs/2026-05-26-xc-dev-v2-design.md)
- Implementation plan: [`docs/superpowers/plans/2026-05-26-xc-dev-v2.md`](docs/superpowers/plans/2026-05-26-xc-dev-v2.md)
