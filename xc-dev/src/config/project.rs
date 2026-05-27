use crate::error::{Result, XcDevError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectFile {
    pub schema_version: u32,
    pub project: Project,
    #[serde(default)]
    pub vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub path: String,
    pub scheme: String,
    pub configuration: String,
    #[serde(default)]
    pub derived_data: Option<String>,
    #[serde(default)]
    pub configurations: BTreeMap<String, Configuration>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Configuration {
    pub bundle_id: String,
    pub product_name: String,
}

pub fn parse(s: &str) -> Result<ProjectFile> {
    toml::from_str(s).map_err(|source| XcDevError::Parse {
        path: Path::new("project.toml").to_path_buf(),
        source,
    })
}

pub fn load(path: &Path) -> Result<ProjectFile> {
    let s = std::fs::read_to_string(path).map_err(|source| XcDevError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&s).map_err(|source| XcDevError::Parse {
        path: path.to_path_buf(),
        source,
    })
}
