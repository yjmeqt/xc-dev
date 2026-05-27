use assert_cmd::Command;
use std::path::Path;
use tempfile::tempdir;

fn fixture_root() -> std::path::PathBuf {
    // examples/HelloDemo/ relative to the workspace root (two levels up from xc-dev/Cargo.toml)
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap().join("examples/HelloDemo")
}

fn xcodebuild_available() -> bool {
    std::process::Command::new("xcodebuild")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[test]
fn init_autodetects_against_real_xcodeproj_fixture() {
    if !xcodebuild_available() {
        eprintln!("skipping: xcodebuild not on PATH");
        return;
    }
    let fixture = fixture_root();
    if !fixture.exists() {
        eprintln!("skipping: fixture not present at {}", fixture.display());
        return;
    }
    let tmp = tempdir().unwrap();
    copy_dir_recursive(&fixture, tmp.path()).unwrap();

    Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":proj", "init", "--yes"])
        .assert()
        .success();

    let toml = std::fs::read_to_string(tmp.path().join(".xc-dev/project.toml")).unwrap();

    // Both .xcodeproj and .xcworkspace are present in the fixture; workspace must win.
    assert!(
        toml.contains(r#"path             = "HelloDemo.xcworkspace""#),
        "expected workspace to be preferred over project in:\n{toml}"
    );
    assert!(
        !toml.contains(r#"path             = "HelloDemo.xcodeproj""#),
        "expected .xcodeproj NOT to be selected when .xcworkspace is present:\n{toml}"
    );
    assert!(
        toml.contains(r#"scheme           = "HelloDemo""#),
        "expected detected scheme HelloDemo:\n{toml}"
    );
    // Every configuration gets its own [project.configurations.<name>] block.
    assert!(
        toml.contains("[project.configurations.Debug]"),
        "expected [project.configurations.Debug] block:\n{toml}"
    );
    assert!(
        toml.contains(r#"bundle_id        = "com.example.HelloDemo""#),
        "expected Debug bundle_id detection:\n{toml}"
    );
    assert!(
        toml.contains(r#"product_name     = "HelloDemo""#),
        "expected product_name detection:\n{toml}"
    );
    // Release configuration has a different bundle_id → gets its own block.
    assert!(
        toml.contains("[project.configurations.Release]"),
        "expected [project.configurations.Release] block:\n{toml}"
    );
    assert!(
        toml.contains(r#"bundle_id        = "com.example.HelloDemo.release""#),
        "expected Release bundle_id in its own block:\n{toml}"
    );
    // bundle_id must NOT appear at the [project] level anymore.
    // The [project] section only has path/scheme/configuration.
    let project_section_end = toml.find("\n[project.configurations").unwrap_or(toml.len());
    let project_section = &toml[..project_section_end];
    assert!(
        !project_section.contains("bundle_id"),
        "bundle_id must not appear at [project] level:\n{project_section}"
    );
}
