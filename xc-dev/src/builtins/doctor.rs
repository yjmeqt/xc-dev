use std::collections::BTreeMap;
use std::path::Path;
use crate::error::Result;
use crate::{paths, config, runner, template, vars::{self, Vars}};

pub fn run(cwd: &Path) -> Result<i32> {
    let mut failed = false;

    let root = match paths::find_project_root(cwd) {
        Ok(r) => {
            println!("  PASS  project_root: {}", r.display());
            r
        }
        Err(e) => {
            println!("  FAIL  project_root: {e}");
            return Ok(1);
        }
    };

    let dotdir = paths::xc_dev_dir(&root);

    let pf = match config::project::load(&dotdir.join("project.toml")) {
        Ok(p) => {
            println!("  PASS  project.toml: parsed");
            p
        }
        Err(e) => {
            println!("  FAIL  project.toml: {e}");
            return Ok(1);
        }
    };
    match vars::validate_project(&pf) {
        Ok(()) => println!("  PASS  project reserved-names: ok"),
        Err(e) => { println!("  FAIL  project reserved-names: {e}"); failed = true; }
    }

    let sf = match config::simulator::load(&dotdir.join("simulator.toml")) {
        Ok(s) => { println!("  PASS  simulator.toml: parsed"); s }
        Err(e) => { println!("  FAIL  simulator.toml: {e}"); failed = true;
            // we cannot continue var checks meaningfully without a sim file;
            // synthesize an empty one to keep going.
            crate::config::simulator::SimulatorFile { schema_version: 1, sim: BTreeMap::new() }
        }
    };

    let tasks = match config::tasks::load(&dotdir.join("tasks.toml")) {
        Ok(t) => {
            println!("  PASS  tasks.toml: {} task(s)", t.tasks.len());
            t
        }
        Err(e) => {
            println!("  FAIL  tasks.toml: {e}");
            // Without tasks we can't run per-task checks; finish with other top-level checks.
            return Ok(1);
        }
    };
    match vars::validate_tasks(&tasks) {
        Ok(()) => println!("  PASS  tasks reserved-names: ok"),
        Err(e) => { println!("  FAIL  tasks reserved-names: {e}"); failed = true; }
    }

    let xcodebuild_ok = std::process::Command::new("xcodebuild").arg("-version")
        .output().map(|o| o.status.success()).unwrap_or(false);
    if xcodebuild_ok { println!("  PASS  xcodebuild: found"); }
    else { println!("  FAIL  xcodebuild: missing on PATH"); failed = true; }

    let path_exists = root.join(&pf.project.path).exists();
    if path_exists { println!("  PASS  project.path: {}", pf.project.path); }
    else { println!("  FAIL  project.path: {} (not found at {})", pf.project.path, root.display()); failed = true; }

    for (name, task) in &tasks.tasks {
        match runner::resolve_chain(name, &tasks) {
            Ok(_) => println!("  PASS  task `{name}` depends: ok"),
            Err(e) => { println!("  FAIL  task `{name}` depends: {e}"); failed = true; }
        }
        let args = runner::bind_args(task, &[]).unwrap_or_default();
        let vbuilt = Vars::build(root.clone(), pf.clone(), sf.clone(), args, name);
        match vbuilt {
            Err(e) => { println!("  FAIL  task `{name}` vars: {e}"); failed = true; continue; }
            Ok(v) => {
                let res = template::render(&task.cmd, name, &|n| {
                    v.try_resolve(n).ok().flatten()
                });
                match res {
                    Ok(_) => println!("  PASS  task `{name}` vars: ok"),
                    Err(e) => { println!("  FAIL  task `{name}` vars: {e}"); failed = true; }
                }
            }
        }
    }

    Ok(if failed { 1 } else { 0 })
}
