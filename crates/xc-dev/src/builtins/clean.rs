//! `:clean` — wipe per-project DerivedData (local + system).

use crate::config;
use crate::derived_data::{scan_system_derived_data, Candidate};
use crate::error::{Result, XcDevError};
use crate::paths;
use crate::vars::auto::AutoVars;
use std::path::{Path, PathBuf};

/// Default Xcode system DerivedData location.
pub fn default_system_derived_data() -> PathBuf {
    dirs_home()
        .map(|h| h.join("Library/Developer/Xcode/DerivedData"))
        .unwrap_or_else(|| PathBuf::from("~/Library/Developer/Xcode/DerivedData"))
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Run `:clean` / `:clean --yes`.
///
/// When `yes` is false (default), only print what would be removed.
/// `system_dd` overrides the system DerivedData root (tests inject a temp dir).
pub fn run(cwd: &Path, yes: bool, system_dd: Option<&Path>) -> Result<i32> {
    let root = paths::find_project_root(cwd)?;
    let xc_dev_dir = paths::xc_dev_dir(&root);
    let pf = config::project::load(&xc_dev_dir.join("project.toml"))?;
    let effective_config = pf.project.configuration.clone();
    let product_name = pf
        .project
        .configurations
        .get(&effective_config)
        .map(|c| c.product_name.clone())
        .unwrap_or_else(|| "App".to_string());
    let auto = AutoVars::build(root.clone(), &pf, &effective_config, &product_name);
    let local_dd = auto.derived_data.clone();

    let system_root = system_dd
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os("XC_DEV_SYSTEM_DERIVED_DATA").map(PathBuf::from))
        .unwrap_or_else(default_system_derived_data);

    let mut delete_dirs: Vec<(PathBuf, String)> = Vec::new();

    // Local per-worktree DerivedData
    if local_dd.exists() {
        delete_dirs.push((local_dd.clone(), format!("local {}", local_dd.display())));
    }

    let candidates = if system_root.is_dir() {
        scan_system_derived_data(&system_root, &root)
    } else {
        Vec::new()
    };

    let mut keep = 0usize;
    let mut skip_orphan = 0usize;
    let mut skip_noindex = 0usize;

    for c in &candidates {
        match c {
            Candidate::Delete {
                dir,
                workspace_path,
            } => {
                delete_dirs.push((dir.clone(), workspace_path.clone()));
            }
            Candidate::Keep { .. } => keep += 1,
            Candidate::SkipOrphan { .. } => skip_orphan += 1,
            Candidate::SkipNoIndex { .. } => skip_noindex += 1,
        }
    }

    let mode = if yes { "DELETE" } else { "DRY-RUN" };
    println!("xc-dev :clean [{mode}]");
    println!("  project_root: {}", root.display());
    println!("  local derived_data: {}", local_dd.display());
    println!("  system DerivedData: {}", system_root.display());
    println!();

    if delete_dirs.is_empty() {
        println!("  (nothing to remove)");
    } else {
        for (dir, note) in &delete_dirs {
            println!(
                "  {}  {}",
                if yes { "rm" } else { "would rm" },
                dir.display()
            );
            println!("      {}", note);
        }
    }

    println!();
    println!(
        "  summary: {} to remove, {keep} kept, {skip_orphan} orphans skipped, {skip_noindex} noindex skipped",
        delete_dirs.len()
    );

    if !yes {
        println!();
        println!("  re-run with `xc-dev :clean --yes` to delete");
        return Ok(0);
    }

    for (dir, _) in &delete_dirs {
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|source| XcDevError::Io {
                path: dir.clone(),
                source,
            })?;
        }
    }
    println!();
    println!("  done: removed {}", delete_dirs.len());
    Ok(0)
}
