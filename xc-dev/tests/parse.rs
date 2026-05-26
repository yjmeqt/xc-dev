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
bundle_id        = "com.applovin.yi.bedterm"
product_name     = "BedTerm"
min_ios_version  = "26.0"
"#;
    let p = xc_dev::config::project::parse(toml).unwrap();
    assert_eq!(p.project.scheme, "BedTerm");
    assert_eq!(p.project.bundle_id, "com.applovin.yi.bedterm");
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
bundle_id = "x"
product_name = "X"
min_ios_version = "26.0"
derived_data = "/tmp/dd"

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
