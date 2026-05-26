use std::collections::BTreeMap;
use std::path::Path;
use crate::error::{Result, XcDevError};
use crate::{paths, config, vars::Vars};

pub fn run(cwd: &Path, key: &str) -> Result<i32> {
    let root = paths::find_project_root(cwd)?;
    let dotdir = paths::xc_dev_dir(&root);
    let pf = config::project::load(&dotdir.join("project.toml"))?;
    let sf = config::simulator::load(&dotdir.join("simulator.toml"))?;
    let v = Vars::build(root.clone(), pf, sf, BTreeMap::new(), ":get")?;
    match v.try_resolve(key)? {
        Some(value) => {
            println!("{value}");
            Ok(0)
        }
        None => Err(XcDevError::Other(format!("unknown variable `{key}`"))),
    }
}
