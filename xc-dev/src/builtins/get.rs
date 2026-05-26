use std::path::Path;
use crate::error::{Result, XcDevError};
pub fn run(_cwd: &Path, _key: &str) -> Result<i32> {
    Err(XcDevError::Other("`:get` not yet implemented (Task 16)".into()))
}
