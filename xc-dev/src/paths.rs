use crate::error::{Result, XcDevError};
use std::path::{Path, PathBuf};

pub fn find_project_root(start: &Path) -> Result<PathBuf> {
    let start_abs = start.canonicalize().map_err(|source| XcDevError::Io {
        path: start.to_path_buf(),
        source,
    })?;
    let mut cur = start_abs.as_path();
    loop {
        if cur.join(".xc-dev").is_dir() {
            return Ok(cur.to_path_buf());
        }
        match cur.parent() {
            Some(p) => cur = p,
            None => return Err(XcDevError::NoProjectDir(start_abs)),
        }
    }
}

pub fn xc_dev_dir(project_root: &Path) -> PathBuf {
    project_root.join(".xc-dev")
}
