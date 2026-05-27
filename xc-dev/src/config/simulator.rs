use crate::error::{Result, XcDevError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct SimulatorFile {
    pub schema_version: u32,
    #[serde(default)]
    pub sim: BTreeMap<String, Sim>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Sim {
    /// Explicit UDID. When absent, the UDID is resolved lazily via `device` + `runtime`.
    pub id: Option<String>,
    pub name: String,
    pub runtime: String,
    /// Device type identifier (e.g. "iPhone 16"). Required when `id` is absent.
    pub device: Option<String>,
}

pub fn parse(s: &str) -> Result<SimulatorFile> {
    toml::from_str(s).map_err(|source| XcDevError::Parse {
        path: Path::new("simulator.toml").to_path_buf(),
        source,
    })
}

pub fn load(path: &Path) -> Result<SimulatorFile> {
    let s = std::fs::read_to_string(path).map_err(|source| XcDevError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    toml::from_str(&s).map_err(|source| XcDevError::Parse {
        path: path.to_path_buf(),
        source,
    })
}
