use std::fs;
use tempfile::tempdir;

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
