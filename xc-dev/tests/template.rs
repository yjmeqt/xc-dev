use std::collections::BTreeMap;
use std::path::PathBuf;
use xc_dev::config::project::{Configuration, Project, ProjectFile};
use xc_dev::config::simulator::{Sim, SimulatorFile};
use xc_dev::template;
use xc_dev::vars::Vars;

fn pf() -> ProjectFile {
    let mut confs = BTreeMap::new();
    confs.insert(
        "Debug".to_string(),
        Configuration {
            bundle_id: "com.x".into(),
            product_name: "X".into(),
        },
    );
    ProjectFile {
        schema_version: 1,
        project: Project {
            path: "X.xcworkspace".into(),
            scheme: "X".into(),
            configuration: "Debug".into(),
            derived_data: None,
            configurations: confs,
        },
        vars: BTreeMap::from([("xcbeautify".to_string(), "xcb".to_string())]),
    }
}

fn sf() -> SimulatorFile {
    SimulatorFile {
        schema_version: 1,
        sim: BTreeMap::from([
            (
                "default".into(),
                Sim {
                    id: Some("ABC".into()),
                    name: "iPhone".into(),
                    runtime: "iOS 26".into(),
                    device: None,
                },
            ),
            (
                "ipad".into(),
                Sim {
                    id: Some("DEF".into()),
                    name: "iPad".into(),
                    runtime: "iOS 26".into(),
                    device: None,
                },
            ),
        ]),
    }
}

#[test]
fn resolves_project_field() {
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("scheme"), Some("X".into()));
    assert_eq!(v.resolve("bundle_id"), Some("com.x".into()));
}

#[test]
fn resolves_user_var() {
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("xcbeautify"), Some("xcb".into()));
}

#[test]
fn resolves_default_sim_shortcuts() {
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("sim_id"), Some("ABC".into()));
    assert_eq!(v.resolve("sim_name"), Some("iPhone".into()));
}

#[test]
fn resolves_qualified_sim() {
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("sim.ipad.id"), Some("DEF".into()));
}

#[test]
fn resolves_args_highest_priority() {
    // NOTE: reserved-name enforcement (Task 9) will eventually forbid using
    // a project field name as an arg, but until then args win on shadow.
    let args = BTreeMap::from([("config".into(), "Release".into())]);
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        args,
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("config"), Some("Release".into()));
}

#[test]
fn auto_vars_paths_and_derived_data() {
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("project_root"), Some("/tmp/p".into()));
    assert_eq!(v.resolve("xc_dev_dir"), Some("/tmp/p/.xc-dev".into()));
    assert_eq!(
        v.resolve("derived_data"),
        Some("/tmp/p/.xc-dev/DerivedData".into())
    );
    assert_eq!(
        v.resolve("build_products_dir"),
        Some("/tmp/p/.xc-dev/DerivedData/Build/Products/Debug-iphonesimulator".into())
    );
    assert_eq!(
        v.resolve("app_path"),
        Some("/tmp/p/.xc-dev/DerivedData/Build/Products/Debug-iphonesimulator/X.app".into())
    );
}

#[test]
fn auto_var_user_and_home() {
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("user"), std::env::var("USER").ok());
    assert_eq!(v.resolve("home"), std::env::var("HOME").ok());
}

#[test]
fn auto_var_custom_derived_data() {
    let mut p = pf();
    p.project.derived_data = Some("/custom/dd".into());
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        p,
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("derived_data"), Some("/custom/dd".into()));
    assert_eq!(
        v.resolve("build_products_dir"),
        Some("/custom/dd/Build/Products/Debug-iphonesimulator".into())
    );
}

fn make_resolver<'a>(map: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| {
        map.iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.to_string())
    }
}

#[test]
fn substitutes_simple_var() {
    let r = make_resolver(&[("scheme", "BedTerm")]);
    let out = template::render("xcodebuild -scheme {{scheme}}", "build", &r).unwrap();
    assert_eq!(out, "xcodebuild -scheme BedTerm");
}

#[test]
fn dotted_names_work() {
    let r = make_resolver(&[("sim.ipad.id", "DEF-456")]);
    let out = template::render("launch {{sim.ipad.id}}", "run", &r).unwrap();
    assert_eq!(out, "launch DEF-456");
}

#[test]
fn unknown_var_is_error() {
    let r = make_resolver(&[]);
    let err = template::render("{{foo}}", "x", &r).unwrap_err();
    match err {
        xc_dev::error::XcDevError::UnknownVar { name, task } => {
            assert_eq!(name, "foo");
            assert_eq!(task, "x");
        }
        e => panic!("wrong error: {e}"),
    }
}

#[test]
fn passes_through_non_template() {
    let r = make_resolver(&[]);
    let out = template::render("hello world { not a var }", "x", &r).unwrap();
    assert_eq!(out, "hello world { not a var }");
}

#[test]
fn multiple_vars_one_line() {
    let r = make_resolver(&[("a", "1"), ("b", "2")]);
    let out = template::render("{{a}}-{{b}}-{{a}}", "x", &r).unwrap();
    assert_eq!(out, "1-2-1");
}

#[test]
fn trims_whitespace_inside_braces() {
    let r = make_resolver(&[("scheme", "X")]);
    let out = template::render("{{ scheme }}", "x", &r).unwrap();
    assert_eq!(out, "X");
}

#[test]
fn git_branch_in_real_repo() {
    // Use the actual repo we're testing in (the worktree).
    let cwd = std::env::current_dir().unwrap();
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .unwrap();
    if !out.status.success() {
        return;
    } // not in a git repo; skip
    let project_root = std::path::PathBuf::from(String::from_utf8(out.stdout).unwrap().trim());
    let v = Vars::build(
        project_root.clone(),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    let b = v
        .try_resolve("git_branch")
        .unwrap()
        .expect("git_branch should resolve");
    assert!(!b.is_empty(), "git_branch was empty");
    let sha = v
        .try_resolve("git_sha")
        .unwrap()
        .expect("git_sha should resolve");
    assert!(
        sha.len() >= 7 && sha.len() <= 40,
        "git_sha length suspicious: {sha:?}"
    );
    let main = v
        .try_resolve("git_main_root")
        .unwrap()
        .expect("git_main_root should resolve");
    assert!(!main.is_empty());
}

#[test]
fn git_unavailable_in_non_git_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let v = Vars::build(
        tmp.path().to_path_buf(),
        pf(),
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    let r = v.try_resolve("git_branch");
    assert!(
        matches!(r, Err(xc_dev::error::XcDevError::VarUnavailable { .. })),
        "expected VarUnavailable, got {r:?}"
    );
    // resolve() (infallible) should silently return None:
    assert_eq!(v.resolve("git_branch"), None);
}

#[test]
fn git_unknown_name_returns_ok_none() {
    let cwd = std::env::current_dir().unwrap();
    let v = Vars::build(cwd, pf(), sf(), BTreeMap::new(), "t", "Debug".into()).unwrap();
    // "git_foo" isn't one of the supported names; try_resolve should return Ok(None)
    // (not VarUnavailable), so the template engine reports UnknownVar instead.
    let r = v.try_resolve("git_foo").unwrap();
    assert!(r.is_none());
}

fn pf_with_debug_and_release() -> ProjectFile {
    let mut confs = BTreeMap::new();
    confs.insert(
        "Debug".to_string(),
        Configuration {
            bundle_id: "com.x".into(),
            product_name: "X".into(),
        },
    );
    confs.insert(
        "Release".to_string(),
        Configuration {
            bundle_id: "com.x.release".into(),
            product_name: "X Release".into(),
        },
    );
    ProjectFile {
        schema_version: 1,
        project: Project {
            path: "X.xcworkspace".into(),
            scheme: "X".into(),
            configuration: "Debug".into(),
            derived_data: None,
            configurations: confs,
        },
        vars: BTreeMap::new(),
    }
}

#[test]
fn per_config_bundle_id_resolves_from_configuration_block() {
    let p = pf_with_debug_and_release();
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        p,
        sf(),
        BTreeMap::new(),
        "t",
        "Release".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("bundle_id"), Some("com.x.release".into()));
    assert_eq!(v.resolve("product_name"), Some("X Release".into()));
}

#[test]
fn per_config_debug_resolves_from_debug_block() {
    let p = pf_with_debug_and_release();
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        p,
        sf(),
        BTreeMap::new(),
        "t",
        "Debug".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("bundle_id"), Some("com.x".into()));
    assert_eq!(v.resolve("product_name"), Some("X".into()));
}

#[test]
fn missing_configuration_block_errors_at_build() {
    // effective_config = "Beta" but there is no [project.configurations.Beta] block.
    match Vars::build(
        PathBuf::from("/tmp/p"),
        pf(), // only has Debug
        sf(),
        BTreeMap::new(),
        "t",
        "Beta".into(),
    ) {
        Err(e) => {
            let s = e.to_string();
            assert!(
                s.contains("Beta") && s.contains("no configuration"),
                "expected error mentioning Beta, got: {s}"
            );
        }
        Ok(_) => panic!("expected error for missing Beta config block"),
    }
}

#[test]
fn configuration_resolves_to_effective_config() {
    let p = pf_with_debug_and_release();
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        p,
        sf(),
        BTreeMap::new(),
        "t",
        "Release".into(),
    )
    .unwrap();
    assert_eq!(v.resolve("configuration"), Some("Release".into()));
}

#[test]
fn app_path_uses_effective_config_and_product_name() {
    let p = pf_with_debug_and_release();
    let v = Vars::build(
        PathBuf::from("/tmp/p"),
        p,
        sf(),
        BTreeMap::new(),
        "t",
        "Release".into(),
    )
    .unwrap();
    let app_path = v.resolve("app_path").unwrap();
    assert!(
        app_path.contains("Release-iphonesimulator"),
        "expected Release-iphonesimulator in app_path, got: {app_path}"
    );
    assert!(
        app_path.ends_with("/X Release.app"),
        "expected effective product_name in app_path, got: {app_path}"
    );
}
