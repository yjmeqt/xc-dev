use std::collections::BTreeMap;
use crate::config::tasks::{Task, Tasks};
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

pub fn bind_args(task: &Task, user_args: &[String]) -> Result<BTreeMap<String, String>> {
    let required = task.args.iter().filter(|a| a.default.is_none()).count();
    let max = task.args.len();
    if user_args.len() < required || user_args.len() > max {
        let mut expected = String::new();
        for a in &task.args {
            if let Some(d) = &a.default {
                expected.push_str(&format!("[{}={}] ", a.name, d));
            } else {
                expected.push_str(&format!("<{}> ", a.name));
            }
        }
        return Err(XcDevError::ArgCount {
            task: task.name.clone(),
            expected: expected.trim().to_string(),
            got: user_args.len(),
        });
    }
    let mut map = BTreeMap::new();
    for (i, spec) in task.args.iter().enumerate() {
        let value = user_args.get(i).cloned()
            .unwrap_or_else(|| spec.default.clone().expect(
                "default exists by invariant: required args precede optional"
            ));
        map.insert(spec.name.clone(), value);
    }
    Ok(map)
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
