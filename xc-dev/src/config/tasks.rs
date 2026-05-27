use crate::error::{Result, XcDevError};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct TasksFileRaw {
    #[serde(default)]
    tasks: BTreeMap<String, RawTask>,
}

#[derive(Debug, Deserialize)]
struct RawTask {
    cmd: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    depends: Vec<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    configuration: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub name: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    pub cmd: String,
    pub args: Vec<ArgSpec>,
    pub depends: Vec<String>,
    pub desc: Option<String>,
    pub cwd: Option<String>,
    pub env: BTreeMap<String, String>,
    pub configuration: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Tasks {
    pub tasks: BTreeMap<String, Task>,
}

pub fn parse(s: &str) -> Result<Tasks> {
    let raw: TasksFileRaw = toml::from_str(s).map_err(|source| XcDevError::Parse {
        path: Path::new("tasks.toml").to_path_buf(),
        source,
    })?;
    let mut tasks = BTreeMap::new();
    for (name, raw_task) in raw.tasks {
        if name.starts_with(':') {
            return Err(XcDevError::Other(format!(
                "task name cannot start with `:` (reserved for built-ins): `{name}`"
            )));
        }
        let args = parse_args(&name, &raw_task.args)?;
        tasks.insert(
            name.clone(),
            Task {
                name,
                cmd: raw_task.cmd,
                args,
                depends: raw_task.depends,
                desc: raw_task.desc,
                cwd: raw_task.cwd,
                env: raw_task.env,
                configuration: raw_task.configuration,
            },
        );
    }
    Ok(Tasks { tasks })
}

pub fn load(path: &Path) -> Result<Tasks> {
    let s = std::fs::read_to_string(path).map_err(|source| XcDevError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse(&s)
}

fn parse_args(task_name: &str, raw: &[String]) -> Result<Vec<ArgSpec>> {
    let mut out = Vec::with_capacity(raw.len());
    let mut seen_optional = false;
    for entry in raw {
        let spec = if let Some((name, default)) = entry.split_once('=') {
            seen_optional = true;
            ArgSpec {
                name: name.trim().to_string(),
                default: Some(default.trim().to_string()),
            }
        } else {
            if seen_optional {
                return Err(XcDevError::Other(format!(
                    "task `{task_name}`: required arg `{entry}` after optional arg"
                )));
            }
            ArgSpec {
                name: entry.trim().to_string(),
                default: None,
            }
        };
        if spec.name.is_empty() {
            return Err(XcDevError::Other(format!(
                "task `{task_name}`: empty arg name"
            )));
        }
        out.push(spec);
    }
    Ok(out)
}
