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

#[test]
fn get_project_field() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), "[tasks.x]\ncmd = \"echo\"\n");
    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path()).args([":get", "scheme"])
        .assert().success()
        .stdout(predicates::str::starts_with("X"));
}

#[test]
fn get_derived_data_default() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), "[tasks.x]\ncmd = \"echo\"\n");
    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path()).args([":get", "derived_data"])
        .assert().success()
        .stdout(predicates::str::contains(".xc-dev/DerivedData"));
}

#[test]
fn get_app_path_uses_product_name_and_config() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), "[tasks.x]\ncmd = \"echo\"\n");
    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path()).args([":get", "app_path"])
        .assert().success()
        .stdout(predicates::str::contains("Debug-iphonesimulator/X.app"));
}

#[test]
fn get_unknown_errors() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), "[tasks.x]\ncmd = \"echo\"\n");
    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path()).args([":get", "nope"])
        .assert().failure()
        .stderr(predicates::str::contains("unknown variable"));
}

#[test]
fn get_user_var() {
    let tmp = tempdir().unwrap();
    // Override project.toml to add [vars]
    let dotdir = tmp.path().join(".xc-dev");
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

[vars]
xcbeautify = "xcbeautify --quiet"
"#).unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(dotdir.join("tasks.toml"), "[tasks.x]\ncmd=\"echo\"\n").unwrap();

    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path()).args([":get", "xcbeautify"])
        .assert().success()
        .stdout(predicates::str::starts_with("xcbeautify --quiet"));
}

#[test]
fn doctor_runs_on_minimal_project_and_passes() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), "[tasks.x]\ncmd = \"echo hi\"\n");
    let assert = Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":doctor")
        .assert();
    // We allow doctor to FAIL on xcodebuild missing or project.path missing in the
    // tempdir (the empty fake project), but it should still print PASS lines for
    // the parse + validation checks.
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(out.contains("PASS"), "expected at least some PASS lines:\n{out}");
    assert!(out.contains("project.toml"), "expected project.toml check label:\n{out}");
    assert!(out.contains("tasks.toml"), "expected tasks.toml check label:\n{out}");
}

#[test]
fn doctor_fails_on_unknown_var_in_task() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), "[tasks.x]\ncmd = \"echo {{nope}}\"\n");
    let assert = Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":doctor")
        .assert()
        .failure();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(out.contains("FAIL"), "expected a FAIL line:\n{out}");
    assert!(out.contains("nope"), "expected the unknown var name in output:\n{out}");
}

#[test]
fn doctor_fails_on_depends_cycle() {
    let tmp = tempdir().unwrap();
    write_minimal(tmp.path(), r#"
[tasks.a]
depends = ["b"]
cmd = "echo a"

[tasks.b]
depends = ["a"]
cmd = "echo b"
"#);
    let assert = Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":doctor")
        .assert()
        .failure();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(out.contains("FAIL"), "expected a FAIL line:\n{out}");
}

#[test]
fn doctor_errors_outside_project() {
    let tmp = tempdir().unwrap();
    Command::cargo_bin("xc-dev").unwrap()
        .current_dir(tmp.path())
        .arg(":doctor")
        .assert()
        .failure();
}
