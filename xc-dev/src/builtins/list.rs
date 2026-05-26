use std::path::Path;
use crate::error::{Result, XcDevError};
pub fn run(_cwd: &Path) -> Result<i32> {
    Err(XcDevError::Other("`:list` not yet implemented (Task 15)".into()))
}
