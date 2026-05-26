use assert_cmd::Command;
use std::path::Path;
use tempfile::tempdir;

fn write_minimal(dir: &Path, tasks_toml: &str) {
    let dotdir = dir.join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();
    std::fs::write(dotdir.join("project.toml"), r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"
bundle_id = "com.x"
product_name = "X"
min_ios_version = "26.0"
"#).unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(dotdir.join("tasks.toml"), tasks_toml).unwrap();
}

#[test]
fn list_prints_tasks_alphabetically() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), r#"
[tasks.zeta]
desc = "last alphabetically"
cmd = "echo z"

[tasks.alpha]
cmd = "echo a"
"#);
    let assert = Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":list")
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let alpha_pos = out.find("alpha").expect("alpha should appear");
    let zeta_pos = out.find("zeta").expect("zeta should appear");
    assert!(alpha_pos < zeta_pos, "expected alpha before zeta in:\n{out}");
    assert!(out.contains("last alphabetically"), "desc should be printed:\n{out}");
}

#[test]
fn list_omits_desc_when_absent() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), r#"
[tasks.bare]
cmd = "echo b"
"#);
    let assert = Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":list")
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(out.contains("bare"), "expected `bare` in output:\n{out}");
}

#[test]
fn list_no_args_invokes_list() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), r#"
[tasks.hello]
cmd = "echo hi"
"#);
    let assert = Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(out.contains("hello"), "no-arg invocation should list tasks: {out}");
}

#[test]
fn list_errors_outside_project() {
    let tmp = tempdir().unwrap();
    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":list")
        .assert()
        .failure()
        .stderr(predicates::str::contains("no .xc-dev/"));
}
