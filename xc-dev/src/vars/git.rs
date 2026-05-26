use crate::error::{Result, XcDevError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

pub struct GitVars {
    root: PathBuf,
    cache: RefCell<HashMap<String, std::result::Result<String, String>>>,
}

impl GitVars {
    pub fn new(project_root: PathBuf) -> Self {
        Self { root: project_root, cache: RefCell::new(HashMap::new()) }
    }

    fn run(&self, args: &[&str]) -> std::result::Result<String, String> {
        let out = Command::new("git").args(args).current_dir(&self.root).output()
            .map_err(|e| format!("git not on PATH: {e}"))?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                format!("git {} failed", args.join(" "))
            } else { stderr });
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    fn compute(&self, name: &str) -> Option<std::result::Result<String, String>> {
        match name {
            "git_root" | "git_worktree_dir" =>
                Some(self.run(&["rev-parse", "--show-toplevel"])),
            "git_branch" =>
                Some(self.run(&["rev-parse", "--abbrev-ref", "HEAD"])),
            "git_sha" =>
                Some(self.run(&["rev-parse", "--short", "HEAD"])),
            "git_worktree_name" => Some(
                self.run(&["rev-parse", "--show-toplevel"]).map(|p|
                    std::path::Path::new(&p).file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                )
            ),
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

    pub fn try_get(&self, name: &str) -> Result<Option<String>> {
        // 1) Cache lookup
        if let Some(cached) = self.cache.borrow().get(name) {
            return match cached {
                Ok(v) => Ok(Some(v.clone())),
                Err(e) => Err(XcDevError::VarUnavailable {
                    name: name.to_string(), reason: e.clone(),
                }),
            };
        }
        // 2) Compute (or recognize as unknown)
        let computed = match self.compute(name) {
            Some(r) => r,
            None => return Ok(None), // not a supported git_* name
        };
        // 3) Cache and return
        self.cache.borrow_mut().insert(name.to_string(), computed.clone());
        match computed {
            Ok(v) => Ok(Some(v)),
            Err(e) => Err(XcDevError::VarUnavailable { name: name.to_string(), reason: e }),
        }
    }
}
