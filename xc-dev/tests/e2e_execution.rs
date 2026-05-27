//! End-to-end test: run a real user task against the autodetected HelloDemo fixture
//! and verify the full pipeline — TOML parse → variable resolution (every layer) →
//! template substitution → shell execution with merged env — produces the expected
//! expanded command and environment.

use assert_cmd::Command;
use std::collections::HashMap;
use std::path::Path;
use tempfile::tempdir;

fn fixture_root() -> std::path::PathBuf {
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

/// Initialize an empty git repo so {{git_branch}} resolves.
fn git_init(dir: &Path, branch: &str) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "ci")
            .env("GIT_AUTHOR_EMAIL", "ci@example.com")
            .env("GIT_COMMITTER_NAME", "ci")
            .env("GIT_COMMITTER_EMAIL", "ci@example.com")
            .output()
            .expect("git available")
    };
    let out = run(&["init", "-b", branch]);
    assert!(out.status.success(), "git init failed: {:?}", out);
    let out = run(&["commit", "--allow-empty", "-m", "init"]);
    assert!(out.status.success(), "git commit failed: {:?}", out);
}

#[test]
fn e2e_executes_task_with_full_variable_and_env_surface() {
    if !xcodebuild_available() {
        eprintln!("skipping: xcodebuild not on PATH");
        return;
    }
    let fixture = fixture_root();
    if !fixture.exists() {
        eprintln!("skipping: fixture not present at {}", fixture.display());
        return;
    }

    // 1. Copy fixture to tempdir and run :proj init (autodetect via xcodebuild).
    let tmp = tempdir().unwrap();
    copy_dir_recursive(&fixture, tmp.path()).unwrap();
    Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":proj", "init", "--yes"])
        .assert()
        .success();

    // 2. Init a git repo on a deterministic branch so {{git_branch}} resolves.
    git_init(tmp.path(), "e2e-test-branch");

    // 3. Provide a deterministic [sim.default] so {{sim_*}} resolves.
    std::fs::write(
        tmp.path().join(".xc-dev/simulator.toml"),
        r#"schema_version = 1

[sim.default]
id      = "ABC-123-E2E"
name    = "iPhone E2E"
runtime = "iOS 99.9"
"#,
    )
    .unwrap();

    // 4. Append a custom [vars] entry to the autodetected project.toml.
    //    The scaffolded [vars] is the last section, so plain append lands inside it.
    let mut proj_toml = std::fs::read_to_string(tmp.path().join(".xc-dev/project.toml")).unwrap();
    proj_toml.push_str("my_custom = \"vars-layer-value\"\n");
    std::fs::write(tmp.path().join(".xc-dev/project.toml"), proj_toml).unwrap();

    // 5. Replace tasks.toml with a probe task that echoes every interesting variable
    //    and an env-var, KEY=value per line. We assert against this output.
    std::fs::write(
        tmp.path().join(".xc-dev/tasks.toml"),
        r#"
[tasks.probe]
args = ["mode=default-arg"]
env = { TEST_ENV = "from-task-env" }
cmd = """
echo SCHEME={{scheme}}
echo CONFIG={{configuration}}
echo BUNDLE_ID={{bundle_id}}
echo PRODUCT_NAME={{product_name}}
echo DERIVED_DATA={{derived_data}}
echo APP_PATH={{app_path}}
echo PROJECT_ROOT={{project_root}}
echo XC_DEV_DIR={{xc_dev_dir}}
echo BUILD_PRODUCTS={{build_products_dir}}
echo MY_CUSTOM={{my_custom}}
echo ARG_MODE={{mode}}
echo SIM_ID={{sim_id}}
echo SIM_NAME={{sim_name}}
echo SIM_RUNTIME={{sim_runtime}}
echo GIT_BRANCH={{git_branch}}
echo TEST_ENV=$TEST_ENV
echo PATH_PRESENT=$([ -n "$PATH" ] && echo yes || echo no)
echo PWD=$(pwd -P)
"""
"#,
    )
    .unwrap();

    // 6. Run the task with default arg.
    let out = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args(["probe"])
        .output()
        .expect("cmd ran");
    assert!(
        out.status.success(),
        "probe failed (exit={:?})\nstderr:\n{}\nstdout:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    // 7. Parse `KEY=value` lines into a map.
    let stdout = String::from_utf8(out.stdout).unwrap();
    let kv: HashMap<&str, &str> = stdout.lines().filter_map(|l| l.split_once('=')).collect();

    let expect = |key: &str, want: &str| {
        let got = kv
            .get(key)
            .unwrap_or_else(|| panic!("missing key `{key}` in stdout:\n{stdout}"));
        assert_eq!(*got, want, "key `{key}` mismatch (stdout:\n{stdout})");
    };
    let expect_suffix = |key: &str, suffix: &str| {
        let got = kv
            .get(key)
            .unwrap_or_else(|| panic!("missing key `{key}` in stdout:\n{stdout}"));
        assert!(
            got.ends_with(suffix),
            "key `{key}` value {got:?} does not end with {suffix:?} (stdout:\n{stdout})"
        );
    };
    let expect_contains = |key: &str, needle: &str| {
        let got = kv
            .get(key)
            .unwrap_or_else(|| panic!("missing key `{key}` in stdout:\n{stdout}"));
        assert!(
            got.contains(needle),
            "key `{key}` value {got:?} does not contain {needle:?} (stdout:\n{stdout})"
        );
    };

    // --- [project] layer ---
    expect("SCHEME", "HelloDemo");
    expect("CONFIG", "Debug");
    expect("BUNDLE_ID", "com.example.HelloDemo");
    expect("PRODUCT_NAME", "HelloDemo");

    // --- auto vars (paths derived from project_root + config + product_name) ---
    expect_suffix("DERIVED_DATA", "/.xc-dev/DerivedData");
    expect_contains("BUILD_PRODUCTS", "Debug-iphonesimulator");
    expect_suffix("APP_PATH", "/HelloDemo.app");
    expect_contains("APP_PATH", "Debug-iphonesimulator");
    expect_suffix("XC_DEV_DIR", "/.xc-dev");

    // --- [vars] layer ---
    expect("MY_CUSTOM", "vars-layer-value");

    // --- args layer (default value applied since caller passed none) ---
    expect("ARG_MODE", "default-arg");

    // --- sim layer (default sim shortcuts) ---
    expect("SIM_ID", "ABC-123-E2E");
    expect("SIM_NAME", "iPhone E2E");
    expect("SIM_RUNTIME", "iOS 99.9");

    // --- git layer (lazy, surfaces VarUnavailable in non-git dir — but we initialized one) ---
    expect("GIT_BRANCH", "e2e-test-branch");

    // --- env propagation: task env merged onto parent env (PATH is preserved) ---
    expect("TEST_ENV", "from-task-env");
    expect("PATH_PRESENT", "yes");

    // --- cwd defaults to project root ---
    let project_root = kv.get("PROJECT_ROOT").expect("PROJECT_ROOT printed");
    let pwd = kv.get("PWD").expect("PWD printed");
    assert_eq!(
        pwd, project_root,
        "expected task cwd to equal project_root\nPROJECT_ROOT={project_root}\nPWD={pwd}"
    );
}

#[test]
fn e2e_arg_override_replaces_default() {
    if !xcodebuild_available() {
        return;
    }
    let fixture = fixture_root();
    if !fixture.exists() {
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

    std::fs::write(
        tmp.path().join(".xc-dev/tasks.toml"),
        r#"
[tasks.echo-mode]
args = ["mode=default-value"]
cmd = "echo MODE={{mode}}"
"#,
    )
    .unwrap();

    let out = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args(["echo-mode", "override-value"])
        .output()
        .expect("cmd ran");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("MODE=override-value"),
        "expected positional override to win, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("MODE=default-value"),
        "default value should be replaced, got:\n{stdout}"
    );
}

#[test]
fn e2e_depends_chain_passes_state_via_files() {
    if !xcodebuild_available() {
        return;
    }
    let fixture = fixture_root();
    if !fixture.exists() {
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

    // upstream writes a marker file into {{xc_dev_dir}}; downstream reads it back.
    std::fs::write(
        tmp.path().join(".xc-dev/tasks.toml"),
        r#"
[tasks.upstream]
cmd = "echo upstream-ran > {{xc_dev_dir}}/marker"

[tasks.downstream]
depends = ["upstream"]
cmd = "cat {{xc_dev_dir}}/marker"
"#,
    )
    .unwrap();

    let out = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args(["downstream"])
        .output()
        .expect("cmd ran");
    assert!(
        out.status.success(),
        "downstream failed (exit={:?})\nstderr:\n{}\nstdout:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("upstream-ran"),
        "depends chain should have run upstream first; got:\n{stdout}"
    );
}

#[test]
fn e2e_task_configuration_resolves_release_bundle_id() {
    if !xcodebuild_available() {
        eprintln!("skipping: xcodebuild not on PATH");
        return;
    }
    let fixture = fixture_root();
    if !fixture.exists() {
        eprintln!("skipping: fixture not present at {}", fixture.display());
        return;
    }

    // 1. Copy fixture to tempdir and run :proj init (autodetects multi-config).
    let tmp = tempdir().unwrap();
    copy_dir_recursive(&fixture, tmp.path()).unwrap();
    Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args([":proj", "init", "--yes"])
        .assert()
        .success();

    // Verify the init produced the Release override block.
    let project_toml = std::fs::read_to_string(tmp.path().join(".xc-dev/project.toml")).unwrap();
    assert!(
        project_toml.contains("[project.configurations.Release]"),
        "expected Release config override block in project.toml:\n{project_toml}"
    );

    // 2. Write a probe task that uses configuration = "Release".
    std::fs::write(
        tmp.path().join(".xc-dev/tasks.toml"),
        r#"
[tasks.probe-release]
configuration = "Release"
cmd = "echo BUNDLE_ID={{bundle_id}} CONFIGURATION={{configuration}}"
"#,
    )
    .unwrap();

    // 3. Run the task and verify output.
    let out = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args(["probe-release"])
        .output()
        .expect("cmd ran");
    assert!(
        out.status.success(),
        "probe-release failed (exit={:?})\nstderr:\n{}\nstdout:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("BUNDLE_ID=com.example.HelloDemo.release"),
        "expected Release bundle_id in output:\n{stdout}"
    );
    assert!(
        stdout.contains("CONFIGURATION=Release"),
        "expected CONFIGURATION=Release in output:\n{stdout}"
    );
}
