use crate::error::Result;
use crate::{config, paths};
use std::path::Path;

pub fn run(cwd: &Path) -> Result<i32> {
    let root = paths::find_project_root(cwd)?;
    let tasks = config::tasks::load(&paths::xc_dev_dir(&root).join("tasks.toml"))?;

    if tasks.tasks.is_empty() {
        println!("(no tasks in .xc-dev/tasks.toml)");
        return Ok(0);
    }
    let width = tasks.tasks.keys().map(|k| k.len()).max().unwrap_or(0);
    for (name, task) in &tasks.tasks {
        match &task.desc {
            Some(d) => println!("  {:<width$}  {}", name, d, width = width),
            None => println!("  {}", name),
        }
    }
    Ok(0)
}
