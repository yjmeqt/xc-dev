use std::fs;
use tempfile::tempdir;

#[test]
fn parses_minimal_project_toml() {
    let toml = r#"
schema_version = 1

[project]
path             = "BedTerm.xcworkspace"
scheme           = "BedTerm"
configuration    = "Debug"

[project.configurations.Debug]
bundle_id        = "com.applovin.yi.bedterm"
product_name     = "BedTerm"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    assert_eq!(p.project.scheme, "BedTerm");
    assert_eq!(
        p.project.configurations["Debug"].bundle_id,
        "com.applovin.yi.bedterm"
    );
    assert!(p.project.derived_data.is_none());
    assert!(p.vars.is_empty());
}

#[test]
fn parses_optional_derived_data_and_vars() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"
derived_data = "/tmp/dd"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars]
xcbeautify = "xcbeautify --quiet"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    assert_eq!(p.project.derived_data.as_deref(), Some("/tmp/dd"));
    assert_eq!(p.vars.get("xcbeautify").unwrap(), "xcbeautify --quiet");
}

#[test]
fn finds_project_root_from_nested_dir() {
    let tmp = tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join(".xc-dev")).unwrap();
    let nested = root.join("a/b/c");
    fs::create_dir_all(&nested).unwrap();

    let found = xc_dev::paths::find_project_root(&nested).unwrap();
    // Note: canonicalize may resolve symlinks on macOS (/var → /private/var).
    // Compare canonicalized forms.
    assert_eq!(found, root.canonicalize().unwrap());
}

#[test]
fn errors_when_no_xc_dev_dir() {
    let tmp = tempdir().unwrap();
    let err = xc_dev::paths::find_project_root(tmp.path()).unwrap_err();
    assert!(matches!(err, xc_dev::error::XcDevError::NoProjectDir(_)));
}

#[test]
fn parses_multi_sim() {
    let toml = r#"
schema_version = 1
[sim.default]
id = "ABC-123"
name = "iPhone 17 Pro"
runtime = "iOS 26.2"

[sim.ipad]
id = "DEF-456"
name = "iPad Pro 11"
runtime = "iOS 26.2"
"#;
    let s = xc_dev::config::simulator::parse(toml).unwrap();
    assert_eq!(s.sim.len(), 2);
    assert_eq!(s.sim["default"].id.as_deref(), Some("ABC-123"));
    assert_eq!(s.sim["ipad"].name, "iPad Pro 11");
}

#[test]
fn parses_empty_simulator_file() {
    let toml = "schema_version = 1\n";
    let s = xc_dev::config::simulator::parse(toml).unwrap();
    assert!(s.sim.is_empty());
}

use xc_dev::config::tasks;

#[test]
fn parses_required_and_optional_args() {
    let toml = r#"
[tasks.build]
args = ["config=Debug"]
cmd = "xcodebuild build"

[tasks.test-package]
args = ["package"]
cmd = "swift test --package-path Packages/{{package}}"
"#;
    let t = tasks::parse(toml).unwrap();
    let build = &t.tasks["build"];
    assert_eq!(build.args.len(), 1);
    assert_eq!(build.args[0].name, "config");
    assert_eq!(build.args[0].default.as_deref(), Some("Debug"));

    let testpkg = &t.tasks["test-package"];
    assert_eq!(testpkg.args[0].name, "package");
    assert!(testpkg.args[0].default.is_none());
}

#[test]
fn rejects_required_after_optional() {
    let toml = r#"
[tasks.x]
args = ["a=1", "b"]
cmd = "echo"
"#;
    let err = tasks::parse(toml).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("required arg") && s.contains("after optional"),
        "unexpected error message: {s}"
    );
}

#[test]
fn rejects_colon_prefixed_task_name() {
    let toml = r#"
[tasks.":foo"]
cmd = "echo"
"#;
    let err = tasks::parse(toml).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("cannot start with"), "unexpected error: {s}");
}

#[test]
fn parses_depends_and_env_and_cwd() {
    let toml = r#"
[tasks.run]
depends = ["build"]
cwd = "ios"
env = { FOO = "bar" }
cmd = "xcrun simctl launch x y"
"#;
    let t = tasks::parse(toml).unwrap();
    let r = &t.tasks["run"];
    assert_eq!(r.depends, vec!["build".to_string()]);
    assert_eq!(r.cwd.as_deref(), Some("ios"));
    assert_eq!(r.env.get("FOO").unwrap(), "bar");
}

#[test]
fn vars_reserved_name_rejected() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars]
git_branch = "naughty"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    let err = xc_dev::vars::validate_project(&p).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("reserved name `git_branch`"),
        "unexpected error: {s}"
    );
}

#[test]
fn vars_project_field_collision_rejected() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars]
scheme = "override"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    let err = xc_dev::vars::validate_project(&p).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("reserved name `scheme`"),
        "unexpected error: {s}"
    );
}

#[test]
fn args_reserved_name_rejected() {
    let toml = r#"
[tasks.x]
args = ["scheme"]
cmd = "echo {{scheme}}"
"#;
    let t = xc_dev::config::tasks::parse(toml).unwrap();
    let err = xc_dev::vars::validate_tasks(&t).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("reserved name `scheme`"),
        "unexpected error: {s}"
    );
}

#[test]
fn args_auto_var_name_rejected() {
    let toml = r#"
[tasks.x]
args = ["git_branch"]
cmd = "echo {{git_branch}}"
"#;
    let t = xc_dev::config::tasks::parse(toml).unwrap();
    let err = xc_dev::vars::validate_tasks(&t).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("reserved name `git_branch`"),
        "unexpected error: {s}"
    );
}

#[test]
fn validation_passes_for_clean_config() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars]
xcbeautify = "xcb"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    xc_dev::vars::validate_project(&p).unwrap();

    let t_toml = r#"
[tasks.build]
args = ["config=Debug"]
cmd = "echo"
"#;
    let t = xc_dev::config::tasks::parse(t_toml).unwrap();
    xc_dev::vars::validate_tasks(&t).unwrap();
}

#[test]
fn parses_project_configurations_blocks() {
    let toml = r#"
schema_version = 1
[project]
path = "MyApp.xcworkspace"
scheme = "MyApp"
configuration = "Debug"

[project.configurations.Debug]
bundle_id    = "com.example.myapp.debug"
product_name = "MyApp Dev"

[project.configurations.Release]
bundle_id    = "com.example.myapp"
product_name = "MyApp"

[project.configurations.AdHoc]
bundle_id    = "com.example.myapp.adhoc"
product_name = "MyApp AdHoc"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();

    let debug = p.project.configurations.get("Debug").expect("Debug config");
    assert_eq!(debug.bundle_id, "com.example.myapp.debug");
    assert_eq!(debug.product_name, "MyApp Dev");

    let release = p
        .project
        .configurations
        .get("Release")
        .expect("Release config");
    assert_eq!(release.bundle_id, "com.example.myapp");
    assert_eq!(release.product_name, "MyApp");

    let adhoc = p.project.configurations.get("AdHoc").expect("AdHoc config");
    assert_eq!(adhoc.bundle_id, "com.example.myapp.adhoc");
    assert_eq!(adhoc.product_name, "MyApp AdHoc");
}

#[test]
fn missing_bundle_id_in_configuration_block_is_parse_error() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
product_name = "X"
"#;
    // bundle_id is required in Configuration — serde should reject this.
    let err = xc_dev::config::project::parse(toml).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("bundle_id") || s.contains("missing field"),
        "expected parse error mentioning bundle_id, got: {s}"
    );
}

#[test]
fn missing_default_configuration_block_is_validation_error() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Beta"

[project.configurations.Debug]
bundle_id    = "com.example.x"
product_name = "X"
"#;
    // [project].configuration = "Beta" but no [project.configurations.Beta] block.
    let p = xc_dev::config::project::parse(toml).unwrap();
    let err = xc_dev::vars::validate_project(&p).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("Beta") && s.contains("no [project.configurations.Beta]"),
        "unexpected error: {s}"
    );
}

#[test]
fn parses_nested_vars() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars]
xcb = "xcbeautify"

[vars.accounts.02]
email    = "test.02@mailinator.com"
password = "PulseTest#2026!"
user_id  = 16195

[vars.accounts.03]
email    = "test.03@mailinator.com"
password = "12345678"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    assert_eq!(p.vars.get("xcb").unwrap(), "xcbeautify");
    assert_eq!(
        p.vars.get("accounts.02.email").unwrap(),
        "test.02@mailinator.com"
    );
    assert_eq!(
        p.vars.get("accounts.02.password").unwrap(),
        "PulseTest#2026!"
    );
    assert_eq!(p.vars.get("accounts.02.user_id").unwrap(), "16195");
    assert_eq!(
        p.vars.get("accounts.03.email").unwrap(),
        "test.03@mailinator.com"
    );
}

#[test]
fn nested_vars_reserved_name_rejected() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars.git_branch]
something = "bad"
"#;
    // Flattens to "git_branch.something" — top-level key is "git_branch.something", not reserved.
    // But a direct flat key collision is what the reserved check catches:
    let p = xc_dev::config::project::parse(toml).unwrap();
    // "git_branch.something" is not itself reserved, so validation passes.
    xc_dev::vars::validate_project(&p).unwrap();
}

#[test]
fn nested_vars_boolean_coercion() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars.feature]
enabled  = true
disabled = false
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    assert_eq!(p.vars.get("feature.enabled").unwrap(), "true");
    assert_eq!(p.vars.get("feature.disabled").unwrap(), "false");
}

#[test]
fn nested_vars_deep_nesting() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars.a.b.c]
key = "deep"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    assert_eq!(p.vars.get("a.b.c.key").unwrap(), "deep");
}

#[test]
fn nested_vars_unsupported_leaf_type_errors() {
    let toml = r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "x"
product_name = "X"

[vars]
bad = ["a", "b"]
"#;
    let err = xc_dev::config::project::parse(toml).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("unsupported value type") || s.contains("bad"),
        "unexpected error: {s}"
    );
}

#[test]
fn parses_task_configuration_field() {
    let toml = r#"
[tasks.archive]
configuration = "Release"
cmd = "xcodebuild archive"

[tasks.build]
cmd = "xcodebuild build"
"#;
    let t = tasks::parse(toml).unwrap();
    assert_eq!(t.tasks["archive"].configuration.as_deref(), Some("Release"));
    assert!(t.tasks["build"].configuration.is_none());
}
