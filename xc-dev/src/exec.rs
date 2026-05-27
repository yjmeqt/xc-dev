use crate::error::{Result, XcDevError};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::Command;

/// Run a shell script: write to a temp file, exec `sh <file>`,
/// honor optional cwd and merged env, return the child's exit code.
pub fn run_script(script: &str, cwd: Option<&Path>, env: &BTreeMap<String, String>) -> Result<i32> {
    let mut file = tempfile::Builder::new()
        .prefix("xc-dev-")
        .suffix(".sh")
        .tempfile()
        .map_err(|e| XcDevError::Other(format!("tempfile: {e}")))?;
    file.write_all(script.as_bytes())
        .map_err(|e| XcDevError::Other(format!("tempfile write: {e}")))?;
    file.flush().ok();

    let mut cmd = Command::new("sh");
    cmd.arg(file.path());
    if let Some(c) = cwd {
        cmd.current_dir(c);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let status = cmd
        .status()
        .map_err(|e| XcDevError::Other(format!("sh: {e}")))?;
    Ok(status.code().unwrap_or(1))
}
