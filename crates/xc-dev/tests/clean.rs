//! Integration tests for `xc-dev :clean`.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn write_project(dir: &Path) {
    let dotdir = dir.join(".xc-dev");
    fs::create_dir_all(&dotdir).unwrap();
    fs::write(
        dotdir.join("project.toml"),
        r#"
schema_version = 1
[project]
path = "X.xcodeproj"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "com.x"
product_name = "X"
"#,
    )
    .unwrap();
    fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    fs::write(dotdir.join("tasks.toml"), "").unwrap();
}

fn write_xml_plist(path: &Path, workspace_path: &str) {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>WorkspacePath</key>
	<string>{workspace_path}</string>
</dict>
</plist>
"#
    );
    fs::write(path, xml).unwrap();
}

#[test]
fn clean_dry_run_lists_but_does_not_delete() {
    let tmp = tempdir().unwrap();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    write_project(&proj);

    let local = proj.join(".xc-dev/DerivedData");
    fs::create_dir_all(local.join("Build")).unwrap();

    let sys = tmp.path().join("SystemDD");
    fs::create_dir_all(&sys).unwrap();
    let root = proj.canonicalize().unwrap();
    let root_s = root.to_string_lossy().to_string();

    let hit = sys.join("App-aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    fs::create_dir_all(&hit).unwrap();
    write_xml_plist(&hit.join("info.plist"), &format!("{root_s}/Packages/Lib"));

    let miss = sys.join("App-bbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    fs::create_dir_all(&miss).unwrap();
    write_xml_plist(&miss.join("info.plist"), "/other/proj/App.xcodeproj");

    let assert = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(&proj)
        .env("XC_DEV_SYSTEM_DERIVED_DATA", &sys)
        .arg(":clean")
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("DRY-RUN"), "{out}");
    assert!(out.contains("would rm"), "{out}");
    assert!(
        out.contains("Packages/Lib") || out.contains(&root_s),
        "{out}"
    );
    assert!(local.exists(), "dry-run must not delete local DerivedData");
    assert!(hit.exists(), "dry-run must not delete system hit");
    assert!(miss.exists());
}

#[test]
fn clean_yes_removes_local_and_matching_system() {
    let tmp = tempdir().unwrap();
    let proj = tmp.path().join("proj");
    fs::create_dir_all(&proj).unwrap();
    write_project(&proj);

    let local = proj.join(".xc-dev/DerivedData");
    fs::create_dir_all(local.join("Build")).unwrap();

    let sys = tmp.path().join("SystemDD");
    fs::create_dir_all(&sys).unwrap();
    let root = proj.canonicalize().unwrap();
    let root_s = root.to_string_lossy().to_string();

    let hit_main = sys.join("App-aaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    fs::create_dir_all(&hit_main).unwrap();
    write_xml_plist(
        &hit_main.join("info.plist"),
        &format!("{root_s}/App.xcodeproj"),
    );

    let hit_pkg = sys.join("Lib-cccccccccccccccccccccccccccc");
    fs::create_dir_all(&hit_pkg).unwrap();
    write_xml_plist(
        &hit_pkg.join("info.plist"),
        &format!("{root_s}/Packages/Lib"),
    );

    let miss = sys.join("App-bbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    fs::create_dir_all(&miss).unwrap();
    write_xml_plist(&miss.join("info.plist"), "/other/proj/App.xcodeproj");

    let noindex = sys.join("ModuleCache.noindex");
    fs::create_dir_all(&noindex).unwrap();

    Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(&proj)
        .env("XC_DEV_SYSTEM_DERIVED_DATA", &sys)
        .args([":clean", "--yes"])
        .assert()
        .success();

    assert!(!local.exists(), "local DerivedData should be gone");
    assert!(!hit_main.exists(), "main project system DD should be gone");
    assert!(!hit_pkg.exists(), "package system DD should be gone");
    assert!(miss.exists(), "other project must remain");
    assert!(noindex.exists(), "noindex must remain");
}
