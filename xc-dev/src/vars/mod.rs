pub mod auto;
pub mod git;

use crate::config::project::ProjectFile;
use crate::config::simulator::{Sim, SimulatorFile};
use crate::error::{Result, XcDevError};
use crate::sim_resolver::SimResolver;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

const PROJECT_FIELD_NAMES: &[&str] = &[
    "path",
    "scheme",
    "configuration",
    "bundle_id",
    "product_name",
    "min_ios_version",
];

pub fn validate_project(p: &ProjectFile) -> Result<()> {
    for key in p.vars.keys() {
        if auto::is_reserved(key) || PROJECT_FIELD_NAMES.contains(&key.as_str()) {
            return Err(XcDevError::ReservedName {
                name: key.clone(),
                location: "[vars] key",
            });
        }
    }
    Ok(())
}

pub fn validate_tasks(t: &crate::config::tasks::Tasks) -> Result<()> {
    for task in t.tasks.values() {
        for a in &task.args {
            if auto::is_reserved(&a.name) || PROJECT_FIELD_NAMES.contains(&a.name.as_str()) {
                return Err(XcDevError::ReservedName {
                    name: a.name.clone(),
                    location: "task arg name",
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
    effective_config: String,
    effective_bundle_id: String,
    effective_product_name: String,
    effective_min_ios_version: String,
    /// Lazy UDID resolver (interior-mutable so resolve/try_resolve can take &self).
    sim_resolver: RefCell<SimResolver>,
}

impl Vars {
    pub fn build(
        project_root: PathBuf,
        project: ProjectFile,
        simulator: SimulatorFile,
        args: BTreeMap<String, String>,
        task_name: &str,
        effective_config: String,
    ) -> Result<Self> {
        // Resolve per-config overrides
        let overrides = project.project.configurations.get(&effective_config);
        let effective_bundle_id = overrides
            .and_then(|o| o.bundle_id.clone())
            .unwrap_or_else(|| project.project.bundle_id.clone());
        let effective_product_name = overrides
            .and_then(|o| o.product_name.clone())
            .unwrap_or_else(|| project.project.product_name.clone());
        let effective_min_ios_version = overrides
            .and_then(|o| o.min_ios_version.clone())
            .unwrap_or_else(|| project.project.min_ios_version.clone());

        let auto = auto::AutoVars::build(
            project_root.clone(),
            &project,
            &effective_config,
            &effective_product_name,
        );
        let git = git::GitVars::new(project_root.clone());
        let xc_dev_dir = crate::paths::xc_dev_dir(&project_root);
        let sim_resolver = RefCell::new(SimResolver::new(&xc_dev_dir));

        Ok(Self {
            auto,
            project,
            simulator,
            args,
            git,
            task_name: task_name.to_string(),
            effective_config,
            effective_bundle_id,
            effective_product_name,
            effective_min_ios_version,
            sim_resolver,
        })
    }

    /// Infallible resolver: ignores git/sim errors (treats as missing).
    pub fn resolve(&self, name: &str) -> Option<String> {
        if let Some(v) = self.args.get(name) {
            return Some(v.clone());
        }
        if let Some(v) = self.project.vars.get(name) {
            return Some(v.clone());
        }
        if let Ok(Some(v)) = self.sim_shortcut(name) {
            return Some(v);
        }
        if let Ok(Some(v)) = self.sim_qualified(name) {
            return Some(v);
        }
        if let Some(v) = self.project_field(name) {
            return Some(v);
        }
        if let Some(v) = self.auto.get(name) {
            return Some(v);
        }
        self.git.try_get(name).ok().flatten()
    }

    /// Fallible resolver: surfaces git/sim availability errors. Used by the runner.
    pub fn try_resolve(&self, name: &str) -> Result<Option<String>> {
        if let Some(v) = self.args.get(name) {
            return Ok(Some(v.clone()));
        }
        if let Some(v) = self.project.vars.get(name) {
            return Ok(Some(v.clone()));
        }
        if let Some(v) = self.sim_shortcut(name)? {
            return Ok(Some(v));
        }
        if let Some(v) = self.sim_qualified(name)? {
            return Ok(Some(v));
        }
        if let Some(v) = self.project_field(name) {
            return Ok(Some(v));
        }
        if let Some(v) = self.auto.get(name) {
            return Ok(Some(v));
        }
        self.git.try_get(name)
    }

    /// Resolve `sim_id` / `sim_name` / `sim_runtime` shortcuts for the "default" sim.
    fn sim_shortcut(&self, name: &str) -> Result<Option<String>> {
        let default = match self.simulator.sim.get("default") {
            Some(s) => s,
            None => return Ok(None),
        };
        self.sim_field_for(name, "default", default)
    }

    /// Resolve `sim.<label>.<field>` qualified names.
    fn sim_qualified(&self, name: &str) -> Result<Option<String>> {
        let rest = match name.strip_prefix("sim.") {
            Some(r) => r,
            None => return Ok(None),
        };
        let (label, field) = match rest.split_once('.') {
            Some(p) => p,
            None => return Ok(None),
        };
        let sim = match self.simulator.sim.get(label) {
            Some(s) => s,
            None => return Ok(None),
        };

        // Map qualified field names to the shortcut names used by sim_field_for.
        let shortcut = match field {
            "id" => match label {
                "default" => "sim_id",
                _ => return self.sim_field_for("sim_id", label, sim),
            },
            "name" => match label {
                "default" => "sim_name",
                _ => return self.sim_field_for("sim_name", label, sim),
            },
            "runtime" => match label {
                "default" => "sim_runtime",
                _ => return self.sim_field_for("sim_runtime", label, sim),
            },
            _ => return Ok(None),
        };
        self.sim_field_for(shortcut, label, sim)
    }

    /// Core sim field resolver.
    /// `shortcut` is one of "sim_id", "sim_name", "sim_runtime".
    /// `label` is the sim label (e.g. "default").
    fn sim_field_for(&self, shortcut: &str, label: &str, sim: &Sim) -> Result<Option<String>> {
        match shortcut {
            "sim_id" | "id" => {
                // If explicit id is set, use it.
                if let Some(id) = &sim.id {
                    return Ok(Some(id.clone()));
                }
                // Otherwise resolve lazily via device + runtime.
                let device = sim
                    .device
                    .as_deref()
                    .ok_or_else(|| XcDevError::VarUnavailable {
                        name: "sim_id".to_string(),
                        reason: format!(
                            "no device configured for [sim.{label}]; set `device` or `id`"
                        ),
                    })?;
                // Render sim.name through a restricted resolver (no sim_* recursion).
                let rendered_name = self.render_sim_name(&sim.name);
                let udid =
                    self.sim_resolver
                        .borrow_mut()
                        .resolve(&rendered_name, device, &sim.runtime)?;
                Ok(Some(udid))
            }
            "sim_name" | "name" => Ok(Some(self.render_sim_name(&sim.name))),
            "sim_runtime" | "runtime" => Ok(Some(sim.runtime.clone())),
            _ => Ok(None),
        }
    }

    /// Render a sim.name template using a restricted resolver that refuses to recurse
    /// into any variable whose name starts with "sim" — prevents infinite recursion.
    fn render_sim_name(&self, name_template: &str) -> String {
        // Fast path: no template markers.
        if !name_template.contains("{{") {
            return name_template.to_string();
        }
        crate::template::render(name_template, "_sim_name_render_", &|n| {
            // Refuse sim_* variables to prevent recursion.
            if n.starts_with("sim") {
                return None;
            }
            self.resolve(n)
        })
        .unwrap_or_else(|_| name_template.to_string())
    }

    fn project_field(&self, name: &str) -> Option<String> {
        let p = &self.project.project;
        Some(match name {
            "path" => p.path.clone(),
            "scheme" => p.scheme.clone(),
            "configuration" => self.effective_config.clone(),
            "bundle_id" => self.effective_bundle_id.clone(),
            "product_name" => self.effective_product_name.clone(),
            "min_ios_version" => self.effective_min_ios_version.clone(),
            _ => return None,
        })
    }
}
