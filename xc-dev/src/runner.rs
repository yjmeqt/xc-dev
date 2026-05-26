use crate::config::tasks::Tasks;
use crate::error::{Result, XcDevError};

/// Topologically order all tasks that need to run to execute `root`.
/// Returns dependencies first; `root` last. Each task appears exactly once.
pub fn resolve_chain(root: &str, tasks: &Tasks) -> Result<Vec<String>> {
    if !tasks.tasks.contains_key(root) {
        return Err(XcDevError::UnknownTask(root.to_string()));
    }
    let mut order = Vec::new();
    let mut visiting: Vec<String> = Vec::new();
    let mut visited = std::collections::HashSet::new();
    visit(root, tasks, &mut visiting, &mut visited, &mut order)?;
    Ok(order)
}

fn visit(
    name: &str,
    tasks: &Tasks,
    visiting: &mut Vec<String>,
    visited: &mut std::collections::HashSet<String>,
    order: &mut Vec<String>,
) -> Result<()> {
    if visited.contains(name) { return Ok(()); }
    if visiting.iter().any(|n| n == name) {
        let mut chain = visiting.clone();
        chain.push(name.to_string());
        return Err(XcDevError::DependsCycle(chain.join(" -> ")));
    }
    let task = tasks.tasks.get(name).ok_or_else(||
        XcDevError::Other(format!("unknown task `{name}` referenced from depends"))
    )?;
    visiting.push(name.to_string());
    for dep in &task.depends {
        visit(dep, tasks, visiting, visited, order)?;
    }
    visiting.pop();
    visited.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}
