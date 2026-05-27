use crate::error::Result;
use crate::{
    config, paths, runner, template,
    vars::{self, Vars},
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

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
        Err(e) => {
            println!("  FAIL  project reserved-names: {e}");
            failed = true;
        }
    }

    let sf = match config::simulator::load(&dotdir.join("simulator.toml")) {
        Ok(s) => {
            println!("  PASS  simulator.toml: parsed");
            s
        }
        Err(e) => {
            println!("  FAIL  simulator.toml: {e}");
            failed = true;
            // we cannot continue var checks meaningfully without a sim file;
            // synthesize an empty one to keep going.
            crate::config::simulator::SimulatorFile {
                schema_version: 1,
                sim: BTreeMap::new(),
            }
        }
    };

    // Validate sim entries: warn if neither id nor device is set.
    for (label, sim) in &sf.sim {
        if sim.id.is_none() && sim.device.is_none() {
            println!(
                "  WARN  sim.{label}: neither `id` nor `device` is set — \
                 `{{{{sim_id}}}}` will fail at runtime; set `device` (for auto-resolution) \
                 or `id` (to pin a UDID)"
            );
        }
    }

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
        Err(e) => {
            println!("  FAIL  tasks reserved-names: {e}");
            failed = true;
        }
    }

    // Warn if any task.configuration doesn't match a known configuration name.
    {
        let known_configs: std::collections::BTreeSet<&str> =
            std::iter::once(pf.project.configuration.as_str())
                .chain(pf.project.configurations.keys().map(String::as_str))
                .collect();
        for (name, task) in &tasks.tasks {
            if let Some(cfg) = &task.configuration {
                if !known_configs.contains(cfg.as_str()) {
                    println!(
                        "  WARN  task `{name}` configuration = \"{cfg}\" not in known configurations ({known})",
                        known = known_configs
                            .iter()
                            .map(|s| format!("\"{s}\""))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
        }
    }

    let xcodebuild_ok = std::process::Command::new("xcodebuild")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if xcodebuild_ok {
        println!("  PASS  xcodebuild: found");
    } else {
        println!("  FAIL  xcodebuild: missing on PATH");
        failed = true;
    }

    let path_exists = root.join(&pf.project.path).exists();
    if path_exists {
        println!("  PASS  project.path: {}", pf.project.path);
    } else {
        println!(
            "  FAIL  project.path: {} (not found at {})",
            pf.project.path,
            root.display()
        );
        failed = true;
    }

    for (name, task) in &tasks.tasks {
        match runner::resolve_chain(name, &tasks) {
            Ok(_) => println!("  PASS  task `{name}` depends: ok"),
            Err(e) => {
                println!("  FAIL  task `{name}` depends: {e}");
                failed = true;
            }
        }
        // For the var-check we substitute a placeholder for every declared arg
        // (required or optional) so that tasks with required args are not
        // falsely reported as FAIL due to a missing arg value.
        let args: BTreeMap<String, String> = task
            .args
            .iter()
            .map(|a| {
                (
                    a.name.clone(),
                    a.default
                        .clone()
                        .unwrap_or_else(|| "<placeholder>".to_string()),
                )
            })
            .collect();
        let effective_config = task
            .configuration
            .clone()
            .unwrap_or_else(|| pf.project.configuration.clone());
        let vbuilt = Vars::build(
            root.clone(),
            pf.clone(),
            sf.clone(),
            args,
            name,
            effective_config,
        );
        match vbuilt {
            Err(e) => {
                println!("  FAIL  task `{name}` vars: {e}");
                failed = true;
                continue;
            }
            Ok(v) => {
                let pending_err: RefCell<Option<crate::error::XcDevError>> = RefCell::new(None);
                let res = template::render(&task.cmd, name, &|n| match v.try_resolve(n) {
                    Ok(some) => some,
                    Err(e) => {
                        *pending_err.borrow_mut() = Some(e);
                        None
                    }
                });
                if let Some(e) = pending_err.into_inner() {
                    println!("  FAIL  task `{name}` vars: {e}");
                    failed = true;
                } else {
                    match res {
                        Ok(_) => println!("  PASS  task `{name}` vars: ok"),
                        Err(e) => {
                            println!("  FAIL  task `{name}` vars: {e}");
                            failed = true;
                        }
                    }
                }
            }
        }
    }

    Ok(if failed { 1 } else { 0 })
}
