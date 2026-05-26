use crate::error::Result;
use std::path::PathBuf;

pub struct GitVars {
    _root: PathBuf,
}

impl GitVars {
    pub fn new(project_root: PathBuf) -> Self {
        Self { _root: project_root }
    }
    pub fn try_get(&self, _name: &str) -> Result<Option<String>> {
        Ok(None)
    }
}
