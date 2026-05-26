use std::path::Path;
use crate::error::{Result, XcDevError};
pub fn run(
    _cwd: &Path,
    _force: bool,
    _yes: bool,
    _path: Option<&str>,
    _scheme: Option<&str>,
) -> Result<i32> {
    Err(XcDevError::Other("`:proj init` not yet implemented (Task 18)".into()))
}
