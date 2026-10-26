use crate::error::{Result, XcDevError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub struct GitVars {
    root: PathBuf,
    cache: RefCell<HashMap<String, std::result::Result<String, String>>>,
}

impl GitVars {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            root: project_root,
            cache: RefCell::new(HashMap::new()),
        }
    }

    fn run(&self, args: &[&str]) -> std::result::Result<String, String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| format!("git not on PATH: {e}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                format!("git {} failed", args.join(" "))
            } else {
                stderr
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn compute(&self, name: &str) -> Option<std::result::Result<String, String>> {
        match name {
            "git_root" | "git_worktree_dir" => Some(self.run(&["rev-parse", "--show-toplevel"])),
            "git_branch" => Some(self.run(&["rev-parse", "--abbrev-ref", "HEAD"])),
            "git_sha" => Some(self.run(&["rev-parse", "--short", "HEAD"])),
            "git_worktree_name" => Some(self.run(&["rev-parse", "--show-toplevel"]).map(|p| {
                std::path::Path::new(&p)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })),
            "git_worktree_slug" => Some(self.git_worktree_slug()),
            "git_main_root" => Some(self.git_main_root()),
            _ => None,
        }
    }

    fn git_main_root(&self) -> std::result::Result<String, String> {
        let porc = self.run(&["worktree", "list", "--porcelain"])?;
        for line in porc.lines() {
            if let Some(rest) = line.strip_prefix("worktree ") {
                return Ok(rest.to_string());
            }
        }
        Err("no worktree entries from `git worktree list`".into())
    }

    fn git_worktree_slug(&self) -> std::result::Result<String, String> {
        let worktree = self.run(&["rev-parse", "--show-toplevel"])?;
        let main = self.git_main_root()?;
        Ok(worktree_slug(Path::new(&worktree), Path::new(&main)))
    }

    pub fn try_get(&self, name: &str) -> Result<Option<String>> {
        // 1) Cache lookup
        if let Some(cached) = self.cache.borrow().get(name) {
            return match cached {
                Ok(v) => Ok(Some(v.clone())),
                Err(e) => Err(XcDevError::VarUnavailable {
                    name: name.to_string(),
                    reason: e.clone(),
                }),
            };
        }
        // 2) Compute (or recognize as unknown)
        let computed = match self.compute(name) {
            Some(r) => r,
            None => return Ok(None), // not a supported git_* name
        };
        // 3) Cache and return
        self.cache
            .borrow_mut()
            .insert(name.to_string(), computed.clone());
        match computed {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(XcDevError::VarUnavailable {
                name: name.to_string(),
                reason: e,
            }),
        }
    }
}

/// A name for `worktree` that no other worktree of the same repo shares.
///
/// The main checkout keeps its basename. A worktree inside it (or beside it)
/// is named by its path from there, with leading hidden directories such as
/// `.worktrees` dropped and the rest joined by `-`, so
/// `.worktrees/feat/main` becomes `feat-main`. Anything else falls back to
/// the basename.
pub fn worktree_slug(worktree: &Path, main_root: &Path) -> String {
    let basename = || {
        worktree
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    if worktree == main_root {
        return basename();
    }
    let relative = worktree
        .strip_prefix(main_root)
        .ok()
        .or_else(|| worktree.strip_prefix(main_root.parent()?).ok());
    let Some(relative) = relative else {
        return basename();
    };
    let parts: Vec<String> = relative
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .skip_while(|s| s.starts_with('.'))
        .collect();
    if parts.is_empty() {
        basename()
    } else {
        parts.join("-")
    }
}

#[cfg(test)]
mod tests {
    use super::worktree_slug;
    use std::path::Path;

    fn slug(worktree: &str, main: &str) -> String {
        worktree_slug(Path::new(worktree), Path::new(main))
    }

    #[test]
    fn main_checkout_keeps_its_basename() {
        assert_eq!(slug("/dev/App", "/dev/App"), "App");
    }

    #[test]
    fn nested_worktrees_drop_hidden_container_and_join_the_rest() {
        assert_eq!(
            slug("/dev/App/.worktrees/feat/main", "/dev/App"),
            "feat-main"
        );
        assert_eq!(slug("/dev/App/.worktrees/feat", "/dev/App"), "feat");
        assert_eq!(slug("/dev/App/wt/feat/main", "/dev/App"), "wt-feat-main");
    }

    #[test]
    fn same_leaf_under_different_features_differs() {
        assert_ne!(
            slug("/dev/App/.worktrees/a/main", "/dev/App"),
            slug("/dev/App/.worktrees/b/main", "/dev/App"),
        );
    }

    #[test]
    fn sibling_worktree_is_named_from_the_shared_parent() {
        assert_eq!(slug("/dev/App-wt/feat", "/dev/App"), "App-wt-feat");
    }

    #[test]
    fn unrelated_location_falls_back_to_basename() {
        assert_eq!(slug("/tmp/feat", "/dev/App"), "feat");
    }
}
