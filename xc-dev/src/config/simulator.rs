use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use crate::error::{Result, XcDevError};

#[derive(Debug, Clone, Deserialize)]
pub struct SimulatorFile {
    pub schema_version: u32,
    #[serde(default)]
    pub sim: BTreeMap<String, Sim>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Sim {
    pub id: String,
    pub name: String,
    pub runtime: String,
}

pub fn parse(s: &str) -> Result<SimulatorFile> {
    toml::from_str(s).map_err(|source| XcDevError::Parse {
        path: Path::new("simulator.toml").to_path_buf(), source,
    })
}

pub fn load(path: &Path) -> Result<SimulatorFile> {
    let s = std::fs::read_to_string(path)
        .map_err(|source| XcDevError::Io { path: path.to_path_buf(), source })?;
    toml::from_str(&s).map_err(|source| XcDevError::Parse { path: path.to_path_buf(), source })
}
