use xc_dev::template;
use xc_dev::config::project::{ProjectFile, Project};
use xc_dev::config::simulator::{SimulatorFile, Sim};
use xc_dev::vars::Vars;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn pf() -> ProjectFile {
    ProjectFile {
        schema_version: 1,
        project: Project {
            path: "X.xcworkspace".into(),
            scheme: "X".into(),
            configuration: "Debug".into(),
            bundle_id: "com.x".into(),
            product_name: "X".into(),
            min_ios_version: "26.0".into(),
            derived_data: None,
        },
        vars: BTreeMap::from([("xcbeautify".to_string(), "xcb".to_string())]),
    }
}

fn sf() -> SimulatorFile {
    SimulatorFile {
        schema_version: 1,
        sim: BTreeMap::from([
            ("default".into(), Sim { id: "ABC".into(), name: "iPhone".into(), runtime: "iOS 26".into() }),
            ("ipad".into(),    Sim { id: "DEF".into(), name: "iPad".into(),   runtime: "iOS 26".into() }),
        ]),
    }
}

#[test]
fn resolves_project_field() {
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("scheme"), Some("X".into()));
    assert_eq!(v.resolve("bundle_id"), Some("com.x".into()));
}

#[test]
fn resolves_user_var() {
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("xcbeautify"), Some("xcb".into()));
}

#[test]
fn resolves_default_sim_shortcuts() {
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("sim_id"), Some("ABC".into()));
    assert_eq!(v.resolve("sim_name"), Some("iPhone".into()));
}

#[test]
fn resolves_qualified_sim() {
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("sim.ipad.id"), Some("DEF".into()));
}

#[test]
fn resolves_args_highest_priority() {
    // NOTE: reserved-name enforcement (Task 9) will eventually forbid using
    // a project field name as an arg, but until then args win on shadow.
    let args = BTreeMap::from([("config".into(), "Release".into())]);
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), args, "t").unwrap();
    assert_eq!(v.resolve("config"), Some("Release".into()));
}

#[test]
fn auto_vars_paths_and_derived_data() {
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("project_root"), Some("/tmp/p".into()));
    assert_eq!(v.resolve("xc_dev_dir"), Some("/tmp/p/.xc-dev".into()));
    assert_eq!(v.resolve("derived_data"), Some("/tmp/p/.xc-dev/DerivedData".into()));
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
    let v = Vars::build(PathBuf::from("/tmp/p"), pf(), sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("user"), std::env::var("USER").ok());
    assert_eq!(v.resolve("home"), std::env::var("HOME").ok());
}

#[test]
fn auto_var_custom_derived_data() {
    let mut p = pf();
    p.project.derived_data = Some("/custom/dd".into());
    let v = Vars::build(PathBuf::from("/tmp/p"), p, sf(), BTreeMap::new(), "t").unwrap();
    assert_eq!(v.resolve("derived_data"), Some("/custom/dd".into()));
    assert_eq!(v.resolve("build_products_dir"),
               Some("/custom/dd/Build/Products/Debug-iphonesimulator".into()));
}

fn make_resolver<'a>(map: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| map.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
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
            assert_eq!(name, "foo"); assert_eq!(task, "x");
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
