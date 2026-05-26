use std::path::Path;
use crate::error::{Result, XcDevError};
pub fn run(_cwd: &Path) -> Result<i32> {
    Err(XcDevError::Other("`:doctor` not yet implemented (Task 17)".into()))
}
