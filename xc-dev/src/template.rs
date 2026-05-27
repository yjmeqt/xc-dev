use crate::error::{Result, XcDevError};

pub fn render(src: &str, task: &str, resolver: &dyn Fn(&str) -> Option<String>) -> Result<String> {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after.find("}}").ok_or_else(|| {
            XcDevError::Other(format!("task `{task}`: unterminated `{{{{` in cmd"))
        })?;
        let name = after[..end].trim();
        let value = resolver(name).ok_or_else(|| XcDevError::UnknownVar {
            name: name.to_string(),
            task: task.to_string(),
        })?;
        out.push_str(&value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}
