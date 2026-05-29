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

// Raw form: vars may be a nested table; we flatten it after parsing.
#[derive(Deserialize)]
struct ProjectFileRaw {
    schema_version: u32,
    project: Project,
    #[serde(default)]
    vars: toml::Table,
}

fn flatten_vars(
    table: toml::Table,
    prefix: String,
    out: &mut BTreeMap<String, String>,
) -> Result<()> {
    for (k, v) in table {
        let key = if prefix.is_empty() {
            k
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::String(s) => {
                out.insert(key, s);
            }
            toml::Value::Integer(i) => {
                out.insert(key, i.to_string());
            }
            toml::Value::Boolean(b) => {
                out.insert(key, b.to_string());
            }
            toml::Value::Table(t) => {
                flatten_vars(t, key, out)?;
            }
            other => {
                return Err(XcDevError::Other(format!(
                    "[vars.{key}]: unsupported value type `{}` — only strings, integers, booleans, and nested tables are allowed",
                    other.type_str()
                )));
            }
        }
    }
    Ok(())
}

fn from_raw(raw: ProjectFileRaw) -> Result<ProjectFile> {
    let mut vars = BTreeMap::new();
    flatten_vars(raw.vars, String::new(), &mut vars)?;
    Ok(ProjectFile {
        schema_version: raw.schema_version,
        project: raw.project,
        vars,
    })
}

pub fn parse(s: &str) -> Result<ProjectFile> {
    let raw: ProjectFileRaw = toml::from_str(s).map_err(|source| XcDevError::Parse {
        path: Path::new("project.toml").to_path_buf(),
        source,
    })?;
    from_raw(raw)
}

pub fn load(path: &Path) -> Result<ProjectFile> {
    let s = std::fs::read_to_string(path).map_err(|source| XcDevError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: ProjectFileRaw = toml::from_str(&s).map_err(|source| XcDevError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    from_raw(raw)
}
