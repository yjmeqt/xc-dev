# xc-dev Workspace Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure this repo as a Cargo workspace centered on the `xc-dev` crate, remove the legacy `prd/` directory, and prepare for renaming the GitHub repo to `xc-dev`.

**Architecture:** Move `xc-dev/` to `crates/xc-dev/`, introduce a root workspace `Cargo.toml`, hoist the lockfile, update the README install URL, and clean up legacy artifacts. No user-facing API or compatibility shims since the project has no current users.

**Tech Stack:** Rust + Cargo workspaces, Git.

---

## Task 1: Remove legacy `prd/` directory

- [ ] **Step 1.1: Remove the directory from git**
  - Command: `git rm -r prd/`
  - Expected output: lines like `rm 'prd/index.xml'`, `rm 'prd/project/...'`, `rm 'prd/sim/...'` for each tracked file.

- [ ] **Step 1.2: Verify directory is gone from working tree**
  - Command: `ls prd 2>&1`
  - Expected output: `ls: prd: No such file or directory`

- [ ] **Step 1.3: Verify deletions are staged**
  - Command: `git status --short`
  - Expected output: lines beginning with `D  prd/...` for every removed file (no untracked `prd/` entries).

- [ ] **Step 1.4: Commit**
  - Command:
    ```sh
    git commit -m "chore: remove legacy prd/ sample directory"
    ```
  - Expected output: commit summary showing files deleted under `prd/`.

---

## Task 2: Move `xc-dev/` crate to `crates/xc-dev/`

- [ ] **Step 2.1: Create the `crates/` directory**
  - Command: `mkdir crates`
  - Expected output: (no output; directory created).

- [ ] **Step 2.2: Move the crate via git**
  - Command: `git mv xc-dev crates/xc-dev`
  - Expected output: (no output on success).

- [ ] **Step 2.3: Verify crate contents are in new location**
  - Command: `ls crates/xc-dev`
  - Expected output: includes `Cargo.toml`, `src`, `tests`, `docs`, `README.md`, `skills-lock.json` (and `Cargo.lock` until Task 3).

- [ ] **Step 2.4: Verify the old location is gone**
  - Command: `test ! -e xc-dev && echo gone`
  - Expected output: `gone`

- [ ] **Step 2.5: DO NOT commit yet**
  - Note: Task 3 changes structure further; commit them together.

---

## Task 3: Hoist `Cargo.lock` and add workspace `Cargo.toml`

- [ ] **Step 3.1: Move the lockfile to repo root**
  - Command: `git mv crates/xc-dev/Cargo.lock Cargo.lock`
  - Expected output: (no output on success).

- [ ] **Step 3.2: Create root `Cargo.toml`**
  - File: `/Users/yi.jiang/Developer/worktree-ios-dev-tool/Cargo.toml`
  - Exact contents:
    ```toml
    [workspace]
    members = ["crates/xc-dev"]
    resolver = "2"
    ```

- [ ] **Step 3.3: Verify workspace builds from repo root**
  - Command: `cargo build`
  - Expected output: `Compiling xc-dev v...` followed by `Finished ...` (no errors). The crate is compiled from `crates/xc-dev`.

- [ ] **Step 3.4: Verify workspace tests pass from repo root**
  - Command: `cargo test`
  - Expected output: test summary `test result: ok.` for the `xc-dev` crate (matches pre-move baseline).

- [ ] **Step 3.5: Stage and commit Tasks 2 + 3 together**
  - Command:
    ```sh
    git add -A && git commit -m "refactor: move xc-dev crate into Cargo workspace under crates/"
    ```
  - Expected output: commit summary showing renames under `xc-dev/ -> crates/xc-dev/`, plus new `Cargo.toml` at root and renamed `Cargo.lock`.

---

## Task 4: Update README install URL and crate path

- [ ] **Step 4.1: Replace the cargo-install-from-git URL**
  - File: `/Users/yi.jiang/Developer/worktree-ios-dev-tool/README.md`
  - Replace: `cargo install --git https://github.com/yjmeqt/worktree-ios-dev-tool xc-dev`
  - With:    `cargo install --git https://github.com/yjmeqt/xc-dev xc-dev`

- [ ] **Step 4.2: Replace the local-clone install path**
  - File: `/Users/yi.jiang/Developer/worktree-ios-dev-tool/README.md`
  - Replace: `cd xc-dev && cargo install --path .`
  - With:    `cargo install --path crates/xc-dev`

- [ ] **Step 4.3: Replace the `npx skills add` slug**
  - File: `/Users/yi.jiang/Developer/worktree-ios-dev-tool/README.md`
  - Replace: `npx skills add yjmeqt/worktree-ios-dev-tool`
  - With:    `npx skills add yjmeqt/xc-dev`

- [ ] **Step 4.4: Update the README link to the crate-level README**
  - File: `/Users/yi.jiang/Developer/worktree-ios-dev-tool/README.md`
  - Replace: ``[`xc-dev/README.md`](xc-dev/README.md)``
  - With:    ``[`crates/xc-dev/README.md`](crates/xc-dev/README.md)``

- [ ] **Step 4.5: Verify no `worktree-ios-dev-tool` references remain in README**
  - Command: `grep -n worktree-ios-dev-tool README.md`
  - Expected output: (no matches; exit code 1).

- [ ] **Step 4.6: Verify new crate path is referenced**
  - Command: `grep -n 'crates/xc-dev' README.md`
  - Expected output: at least 2 matching lines.

- [ ] **Step 4.7: Commit README updates**
  - Command:
    ```sh
    git add README.md && git commit -m "docs(readme): update install URL and crate path for xc-dev rename"
    ```
  - Expected output: commit summary showing `README.md` modified.

---

## Task 5: Audit and update other path references

- [ ] **Step 5.1: Search for stale `xc-dev/` path references**
  - Command: `grep -rn --exclude-dir=target --exclude-dir=.git --exclude-dir=docs '\bxc-dev/' .`
  - Expected output: a list of matches. For each match outside `crates/xc-dev/` and `skills/xc-dev/`, treat it as needing an update.

- [ ] **Step 5.2: Inspect GitHub workflows**
  - Command: `ls .github/workflows/ 2>/dev/null && grep -rn 'xc-dev' .github/ 2>/dev/null`
  - Expected output: workflow filenames followed by any matches.
  - Action: if any line in `.github/workflows/*.yml` references `xc-dev/` (as a path, not just the binary name), replace it with `crates/xc-dev/`.

- [ ] **Step 5.3: Inspect the skill file**
  - Command: `grep -n 'xc-dev/' skills/xc-dev/SKILL.md`
  - Expected output: matches (if any). Action: if a hit references a source path inside the crate (e.g., `xc-dev/src/...`), update it to `crates/xc-dev/...`. Plain mentions of the binary or skill name need no change.

- [ ] **Step 5.4: Verify the workspace still builds**
  - Command: `cargo build`
  - Expected output: `Finished ...` with no errors.

- [ ] **Step 5.5: Commit only if there are changes**
  - Command: `git status --short` (expected: shows edits if any were made; otherwise empty).
  - If non-empty:
    ```sh
    git add -A && git commit -m "chore: update stale xc-dev/ path references after workspace move"
    ```
  - If empty: skip the commit.

---

## Task 6: Remove untracked local leftover `worktree_ios_dev_tool/`

- [ ] **Step 6.1: Confirm the directory is untracked**
  - Command: `git ls-files worktree_ios_dev_tool/ | head -1`
  - Expected output: (empty — no tracked files).

- [ ] **Step 6.2: Remove the directory**
  - Command: `rm -rf worktree_ios_dev_tool/`
  - Expected output: (no output).

- [ ] **Step 6.3: Verify it is gone**
  - Command: `ls worktree_ios_dev_tool 2>&1`
  - Expected output: `ls: worktree_ios_dev_tool: No such file or directory`

- [ ] **Step 6.4: No commit**
  - Note: directory was untracked; nothing to commit.

---

## Task 7: Final verification

- [ ] **Step 7.1: Build the entire workspace**
  - Command: `cargo build --workspace`
  - Expected output: `Finished ...` with no errors.

- [ ] **Step 7.2: Test the entire workspace**
  - Command: `cargo test --workspace`
  - Expected output: `test result: ok.` for every test target (or matches the pre-move baseline).

- [ ] **Step 7.3: Confirm a clean working tree**
  - Command: `git status`
  - Expected output: `nothing to commit, working tree clean`.

- [ ] **Step 7.4: Review recent commits**
  - Command: `git log --oneline -10`
  - Expected output: top entries include `chore: remove legacy prd/...`, `refactor: move xc-dev crate into Cargo workspace under crates/`, `docs(readme): update install URL...`, and (optionally) `chore: update stale xc-dev/ path references...`.

---

## Task 8: GitHub repo rename (manual — document only)

This task is documented for the human to perform; do not automate.

- [ ] **Step 8.1: Rename the GitHub repository** (user action)
  - On github.com, navigate to the repo settings for `worktree-ios-dev-tool` and rename it to `xc-dev`.

- [ ] **Step 8.2: Update the local git remote** (user action)
  - Command: `git remote set-url origin git@github.com:yjmeqt/xc-dev.git`

- [ ] **Step 8.3: Rename the local worktree directory** (user action)
  - Command: `mv /Users/yi.jiang/Developer/worktree-ios-dev-tool /Users/yi.jiang/Developer/xc-dev`

- [ ] **Step 8.4: Verify the new remote URL**
  - Command: `git remote -v`
  - Expected output: shows `origin git@github.com:yjmeqt/xc-dev.git (fetch)` and `(push)`.
