use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, XcDevError>;

#[derive(Debug, Error)]
pub enum XcDevError {
    #[error("no .xc-dev/ found walking up from {0} — run `xc-dev :proj init`")]
    NoProjectDir(PathBuf),

    #[error("failed to parse {path}: {source}")]
    Parse { path: PathBuf, #[source] source: toml::de::Error },

    #[error("io error on {path}: {source}")]
    Io { path: PathBuf, #[source] source: std::io::Error },

    #[error("unknown variable `{name}` referenced in task `{task}`")]
    UnknownVar { name: String, task: String },

    #[error("unknown task `{0}` — run `xc-dev :list` to see available tasks")]
    UnknownTask(String),

    #[error("reserved name `{name}` cannot be used as {location}")]
    ReservedName { name: String, location: &'static str },

    #[error("cyclic `depends` chain: {0}")]
    DependsCycle(String),

    #[error("task `{task}` expects {expected}; got {got} positional args")]
    ArgCount { task: String, expected: String, got: usize },

    #[error("variable `{name}` unavailable: {reason}")]
    VarUnavailable { name: String, reason: String },

    #[error("{0}")]
    Other(String),
}
