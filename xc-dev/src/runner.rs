use crate::config::tasks::{Task, Tasks};
use crate::error::{Result, XcDevError};
use crate::vars::{self, Vars};
use crate::{config, exec, paths, template};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

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
        let value = user_args.get(i).cloned().unwrap_or_else(|| {
            spec.default
                .clone()
                .expect("default exists by invariant: required args precede optional")
        });
        map.insert(spec.name.clone(), value);
    }
    Ok(map)
}

/// Top-level entry: from a starting directory, find the project,
/// resolve the dependency chain for `task_name`, and execute each.
/// Returns the exit code of the last task run (0 if the chain succeeded).
pub fn run(start_dir: &Path, task_name: &str, user_args: &[String], verbose: bool) -> Result<i32> {
    let root = paths::find_project_root(start_dir)?;
    let dotdir = paths::xc_dev_dir(&root);
    let pf = config::project::load(&dotdir.join("project.toml"))?;
    let sf = config::simulator::load(&dotdir.join("simulator.toml"))?;
    let tasks = config::tasks::load(&dotdir.join("tasks.toml"))?;

    vars::validate_project(&pf)?;
    vars::validate_tasks(&tasks)?;

    let order = resolve_chain(task_name, &tasks)?;

    let mut last_code = 0;
    for name in &order {
        let task = tasks
            .tasks
            .get(name)
            .expect("resolve_chain only returns known tasks");
        let args_for_task = if name == task_name {
            bind_args(task, user_args)?
        } else {
            bind_args(task, &[])?
        };
        let v = Vars::build(root.clone(), pf.clone(), sf.clone(), args_for_task, name)?;
        let pending_err: RefCell<Option<XcDevError>> = RefCell::new(None);
        let rendered = template::render(&task.cmd, name, &|n| match v.try_resolve(n) {
            Ok(some) => some,
            Err(e) => {
                *pending_err.borrow_mut() = Some(e);
                None
            }
        });
        if let Some(e) = pending_err.into_inner() {
            return Err(e);
        }
        let rendered = rendered?;
        if verbose {
            eprintln!("+ {name}");
            for line in rendered.lines() {
                eprintln!("+ {line}");
            }
        }
        let cwd = task.cwd.as_ref().map(|c| root.join(c));
        let code = exec::run_script(&rendered, cwd.as_deref(), &task.env)?;
        last_code = code;
        if code != 0 {
            break;
        }
    }
    Ok(last_code)
}

fn visit(
    name: &str,
    tasks: &Tasks,
    visiting: &mut Vec<String>,
    visited: &mut std::collections::HashSet<String>,
    order: &mut Vec<String>,
) -> Result<()> {
    if visited.contains(name) {
        return Ok(());
    }
    if visiting.iter().any(|n| n == name) {
        let mut chain = visiting.clone();
        chain.push(name.to_string());
        return Err(XcDevError::DependsCycle(chain.join(" -> ")));
    }
    let task = tasks.tasks.get(name).ok_or_else(|| {
        XcDevError::Other(format!("unknown task `{name}` referenced from depends"))
    })?;
    visiting.push(name.to_string());
    for dep in &task.depends {
        visit(dep, tasks, visiting, visited, order)?;
    }
    visiting.pop();
    visited.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}
