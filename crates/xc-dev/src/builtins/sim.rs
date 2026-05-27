//! `:sim bake` built-in — resolve UDIDs and write them into `simulator.toml` in-place.
//!
//! Uses `toml_edit` to preserve comments and formatting.

use crate::config;
use crate::error::{Result, XcDevError};
use crate::paths;
use crate::vars::Vars;
use std::collections::BTreeMap;
use std::path::Path;

/// Run `:sim bake [label]`.
///
/// - If `label` is `Some(l)`, bake only that label.
/// - If `label` is `None`, bake all labels that don't already have an `id`.
pub fn bake(cwd: &Path, label: Option<&str>) -> Result<i32> {
    // 1. Find project root and load configs.
    let root = paths::find_project_root(cwd)?;
    let xc_dev_dir = paths::xc_dev_dir(&root);

    let project = config::project::load(&xc_dev_dir.join("project.toml"))?;
    let simulator = config::simulator::load(&xc_dev_dir.join("simulator.toml"))?;

    // 2. Check there's something to bake.
    if simulator.sim.is_empty() {
        return Err(XcDevError::Other(
            "simulator.toml has no [sim] entries — nothing to bake".to_string(),
        ));
    }

    // 3. Determine which labels to process.
    let labels_to_bake: Vec<String> = if let Some(l) = label {
        // Validate the label exists.
        if !simulator.sim.contains_key(l) {
            return Err(XcDevError::Other(format!(
                "no such [sim.{l}] entry in simulator.toml"
            )));
        }
        let sim = &simulator.sim[l];
        // Skip if already baked.
        if sim.id.is_some() {
            println!("xc-dev: [sim.{l}] already has id set — skipping");
            return Ok(0);
        }
        vec![l.to_string()]
    } else {
        // All labels without an id.
        simulator
            .sim
            .iter()
            .filter(|(_, s)| s.id.is_none())
            .map(|(l, _)| l.clone())
            .collect()
    };

    if labels_to_bake.is_empty() {
        println!("xc-dev: all [sim.*] entries already have id set — nothing to bake");
        return Ok(0);
    }

    // 4. Build Vars and resolve each label.
    let effective_config = project.project.configuration.clone();
    let vars = Vars::build(
        root.clone(),
        project,
        simulator,
        BTreeMap::new(),
        "_sim_bake_",
        effective_config,
    )?;

    let mut resolved: Vec<(String, String)> = Vec::new();
    for lbl in &labels_to_bake {
        // Resolve the UDID via Vars (this triggers SimResolver).
        let var_name = if lbl == "default" {
            "sim_id".to_string()
        } else {
            format!("sim.{lbl}.id")
        };
        let udid = vars
            .try_resolve(&var_name)?
            .ok_or_else(|| XcDevError::VarUnavailable {
                name: var_name.clone(),
                reason: format!("[sim.{lbl}] could not be resolved"),
            })?;
        resolved.push((lbl.clone(), udid));
    }

    // 5. Edit simulator.toml in-place using toml_edit (preserves comments + formatting).
    let sim_path = xc_dev_dir.join("simulator.toml");
    let raw = std::fs::read_to_string(&sim_path).map_err(|source| XcDevError::Io {
        path: sim_path.clone(),
        source,
    })?;

    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e| XcDevError::Other(format!("failed to parse simulator.toml: {e}")))?;

    for (lbl, udid) in &resolved {
        let sim_table = doc
            .get_mut("sim")
            .and_then(|s| s.as_table_mut())
            .ok_or_else(|| XcDevError::Other("simulator.toml has no [sim] table".to_string()))?;

        let entry = sim_table
            .get_mut(lbl.as_str())
            .and_then(|e| e.as_table_mut())
            .ok_or_else(|| {
                XcDevError::Other(format!("no such [sim.{lbl}] entry in simulator.toml"))
            })?;

        entry.insert("id", toml_edit::value(udid.clone()));
    }

    // 6. Write the modified file.
    std::fs::write(&sim_path, doc.to_string()).map_err(|source| XcDevError::Io {
        path: sim_path.clone(),
        source,
    })?;

    // 7. Report.
    for (lbl, udid) in &resolved {
        println!("[sim.{lbl}] → {udid}");
    }
    println!(
        "xc-dev: baked {} entr{}. simulator.toml updated.",
        resolved.len(),
        if resolved.len() == 1 { "y" } else { "ies" }
    );

    Ok(0)
}
