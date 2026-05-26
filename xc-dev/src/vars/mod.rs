pub mod auto;
pub mod git;

use std::collections::BTreeMap;
use std::path::PathBuf;
use crate::config::project::ProjectFile;
use crate::config::simulator::SimulatorFile;
use crate::config::tasks::Tasks;
use crate::error::{Result, XcDevError};

const PROJECT_FIELD_NAMES: &[&str] = &[
    "path", "scheme", "configuration", "bundle_id", "product_name", "min_ios_version",
];

pub fn validate_project(p: &ProjectFile) -> Result<()> {
    for key in p.vars.keys() {
        if auto::is_reserved(key) || PROJECT_FIELD_NAMES.contains(&key.as_str()) {
            return Err(XcDevError::ReservedName {
                name: key.clone(), location: "[vars] key",
            });
        }
    }
    Ok(())
}

pub fn validate_tasks(t: &Tasks) -> Result<()> {
    for task in t.tasks.values() {
        for a in &task.args {
            if auto::is_reserved(&a.name) || PROJECT_FIELD_NAMES.contains(&a.name.as_str()) {
                return Err(XcDevError::ReservedName {
                    name: a.name.clone(), location: "task arg name",
                });
            }
        }
    }
    Ok(())
}

pub struct Vars {
    auto: auto::AutoVars,
    project: ProjectFile,
    simulator: SimulatorFile,
    args: BTreeMap<String, String>,
    git: git::GitVars,
    #[allow(dead_code)]
    task_name: String,
}

impl Vars {
    pub fn build(
        project_root: PathBuf,
        project: ProjectFile,
        simulator: SimulatorFile,
        args: BTreeMap<String, String>,
        task_name: &str,
    ) -> Result<Self> {
        let auto = auto::AutoVars::build(project_root.clone(), &project);
        let git = git::GitVars::new(project_root);
        Ok(Self { auto, project, simulator, args, git, task_name: task_name.to_string() })
    }

    /// Infallible resolver: ignores git errors (treats as missing).
    pub fn resolve(&self, name: &str) -> Option<String> {
        if let Some(v) = self.args.get(name) { return Some(v.clone()); }
        if let Some(v) = self.project.vars.get(name) { return Some(v.clone()); }
        if let Some(v) = self.sim_shortcut(name) { return Some(v); }
        if let Some(v) = self.sim_qualified(name) { return Some(v); }
        if let Some(v) = self.project_field(name) { return Some(v); }
        if let Some(v) = self.auto.get(name) { return Some(v); }
        self.git.try_get(name).ok().flatten()
    }

    /// Fallible resolver: surfaces git availability errors. Used by the runner.
    pub fn try_resolve(&self, name: &str) -> Result<Option<String>> {
        if let Some(v) = self.args.get(name) { return Ok(Some(v.clone())); }
        if let Some(v) = self.project.vars.get(name) { return Ok(Some(v.clone())); }
        if let Some(v) = self.sim_shortcut(name) { return Ok(Some(v)); }
        if let Some(v) = self.sim_qualified(name) { return Ok(Some(v)); }
        if let Some(v) = self.project_field(name) { return Ok(Some(v)); }
        if let Some(v) = self.auto.get(name) { return Ok(Some(v)); }
        self.git.try_get(name)
    }

    fn sim_shortcut(&self, name: &str) -> Option<String> {
        let default = self.simulator.sim.get("default")?;
        match name {
            "sim_id" => Some(default.id.clone()),
            "sim_name" => Some(default.name.clone()),
            "sim_runtime" => Some(default.runtime.clone()),
            _ => None,
        }
    }

    fn sim_qualified(&self, name: &str) -> Option<String> {
        let rest = name.strip_prefix("sim.")?;
        let (label, field) = rest.split_once('.')?;
        let sim = self.simulator.sim.get(label)?;
        match field {
            "id" => Some(sim.id.clone()),
            "name" => Some(sim.name.clone()),
            "runtime" => Some(sim.runtime.clone()),
            _ => None,
        }
    }

    fn project_field(&self, name: &str) -> Option<String> {
        let p = &self.project.project;
        Some(match name {
            "path" => p.path.clone(),
            "scheme" => p.scheme.clone(),
            "configuration" => p.configuration.clone(),
            "bundle_id" => p.bundle_id.clone(),
            "product_name" => p.product_name.clone(),
            "min_ios_version" => p.min_ios_version.clone(),
            _ => return None,
        })
    }
}
