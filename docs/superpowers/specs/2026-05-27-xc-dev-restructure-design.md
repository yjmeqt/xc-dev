# Restructure repo into an xc-dev Cargo workspace

## Goal

Reshape this repo around the `xc-dev` crate as the sole product. Remove the legacy `prd/` sample directory, convert the layout into a Cargo workspace, and rename the repository to `xc-dev`. No backwards-compatibility shims — there are no current users.

## Scope

### In scope

- Delete `prd/` directory (contains: `index.xml`, `project/`, `sim/`).
- Move `xc-dev/` crate to `crates/xc-dev/`.
- Introduce a root workspace `Cargo.toml` with `members = ["crates/xc-dev"]` and `resolver = "2"`.
- Hoist `Cargo.lock` to the repo root (workspace lockfile).
- Update root `README.md`: install URL changes to the new repo name (`github.com/yjmeqt/xc-dev`) and install path becomes `cargo install --path crates/xc-dev`.
- Update any `.github/` workflow paths that reference `xc-dev/`.
- Rename the GitHub repo `worktree-ios-dev-tool` → `xc-dev`; rename the local worktree directory to match.
- Clean up untracked local leftover `worktree_ios_dev_tool/` directory (already removed from git in commit `1e0a0d1`).

### Out of scope (explicit keep)

- `.prd-tool.toml` — keep as-is.
- `prd` skill registration — keep (lives outside this repo).
- `docs/superpowers/` — keep (brainstorming output, unrelated to PRD tooling).
- `examples/HelloDemo/` — keep at repo root.
- `skills/xc-dev/SKILL.md` — keep at repo root; update internal paths only if it references the old `xc-dev/` location.

## Target layout

```
/  (repo: xc-dev)
  Cargo.toml          # [workspace] members = ["crates/xc-dev"], resolver = "2"
  Cargo.lock          # workspace lockfile
  crates/
    xc-dev/           # moved from /xc-dev (Cargo.toml, src/, tests/, docs/, README.md, skills-lock.json)
  skills/
    xc-dev/SKILL.md
  examples/
    HelloDemo/
    README.md
  docs/
    README.md
    plans/
    specs/
    superpowers/
  .prd-tool.toml
  README.md
  LICENSE
  .gitignore
  .github/
  .claude/
```

## Migration steps

1. `git rm -r prd/`
2. `git mv xc-dev crates/xc-dev`
3. Move `crates/xc-dev/Cargo.lock` to the repo root.
4. Create root `Cargo.toml` declaring the workspace (`members = ["crates/xc-dev"]`, `resolver = "2"`).
5. Update root `README.md` — replace `worktree-ios-dev-tool` with `xc-dev` in the install URL and adjust the local-clone path to `crates/xc-dev`.
6. Update `skills/xc-dev/SKILL.md` if it references the old `xc-dev/` path.
7. Update `.github/` workflows for the new crate path.
8. Run `cargo build && cargo test` from the repo root to verify the workspace builds.
9. Remove the untracked local `worktree_ios_dev_tool/` directory.
10. Rename the GitHub repo to `xc-dev`; rename the local worktree directory to match.

## Risks / call-outs

- The `prd` skill registered in the active session is installed outside this repo; it is intentionally untouched.
- GitHub repo rename creates auto-redirects, but since there are no users, no compatibility work is needed.
- `target/` inside the moved crate should not be committed; verify `.gitignore` still applies after the move.

## Verification

- `cargo build` succeeds from the repo root.
- `cargo test` succeeds from the repo root.
- `prd/` is no longer present.
- `crates/xc-dev/Cargo.toml` exists; root `Cargo.toml` declares the workspace.
