use xc_dev::config::tasks;
use xc_dev::runner;

fn parse(toml: &str) -> tasks::Tasks {
    tasks::parse(toml).unwrap()
}

#[test]
fn linear_depends() {
    let t = parse(
        r#"
[tasks.a]
cmd = "echo a"

[tasks.b]
depends = ["a"]
cmd = "echo b"

[tasks.c]
depends = ["b"]
cmd = "echo c"
"#,
    );
    let order = runner::resolve_chain("c", &t).unwrap();
    assert_eq!(
        order,
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[test]
fn cycle_detected() {
    let t = parse(
        r#"
[tasks.a]
depends = ["b"]
cmd = "echo a"

[tasks.b]
depends = ["a"]
cmd = "echo b"
"#,
    );
    let err = runner::resolve_chain("a", &t).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("cyclic"),
        "expected cyclic error, got: {err}"
    );
}

#[test]
fn unknown_task_in_depends() {
    let t = parse(
        r#"
[tasks.a]
depends = ["nope"]
cmd = "echo a"
"#,
    );
    let err = runner::resolve_chain("a", &t).unwrap_err();
    assert!(
        err.to_string().contains("nope"),
        "expected error mentioning `nope`, got: {err}"
    );
}

#[test]
fn unknown_root_task() {
    let t = parse(
        r#"[tasks.a]
cmd = "echo a"
"#,
    );
    let err = runner::resolve_chain("zzz", &t).unwrap_err();
    assert!(
        matches!(err, xc_dev::error::XcDevError::UnknownTask(ref n) if n == "zzz"),
        "expected UnknownTask(\"zzz\"), got: {err:?}"
    );
}

#[test]
fn diamond_dependency_runs_once() {
    let t = parse(
        r#"
[tasks.a]
cmd = "echo a"

[tasks.b]
depends = ["a"]
cmd = "echo b"

[tasks.c]
depends = ["a"]
cmd = "echo c"

[tasks.d]
depends = ["b", "c"]
cmd = "echo d"
"#,
    );
    let order = runner::resolve_chain("d", &t).unwrap();
    assert_eq!(
        order,
        vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string()
        ]
    );
}

#[test]
fn single_task_no_depends() {
    let t = parse(
        r#"[tasks.solo]
cmd = "echo solo"
"#,
    );
    let order = runner::resolve_chain("solo", &t).unwrap();
    assert_eq!(order, vec!["solo".to_string()]);
}

use std::collections::BTreeMap;

#[test]
fn exec_runs_zero_exit() {
    let code = xc_dev::exec::run_script("exit 0", None, &BTreeMap::new()).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn exec_propagates_nonzero_exit() {
    let code = xc_dev::exec::run_script("exit 42", None, &BTreeMap::new()).unwrap();
    assert_eq!(code, 42);
}

#[test]
fn exec_shell_state_persists_across_lines() {
    // Without single-shell semantics, line 2 wouldn't see VAR set in line 1.
    let code =
        xc_dev::exec::run_script("VAR=hi\ntest \"$VAR\" = hi", None, &BTreeMap::new()).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn exec_cwd_and_env_honored() {
    let tmp = tempfile::tempdir().unwrap();
    let mut env = BTreeMap::new();
    env.insert("MARKER".into(), "yes".into());
    // On macOS, /tmp -> /private/tmp; canonicalize so test compares apples to apples.
    let canon = tmp.path().canonicalize().unwrap();
    let cmd = format!(
        "test \"$MARKER\" = yes && test \"$(pwd -P)\" = \"{}\"",
        canon.display()
    );
    let code = xc_dev::exec::run_script(&cmd, Some(&canon), &env).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn exec_set_e_not_injected() {
    // Lines 1 (false) wouldn't halt without `set -e`; line 2 still runs and succeeds.
    // Final exit code should be 0 (from `true`).
    let code = xc_dev::exec::run_script("false\ntrue", None, &BTreeMap::new()).unwrap();
    assert_eq!(code, 0);
}

use xc_dev::config::tasks::Task;

fn task_with_args(args: &[&str]) -> Task {
    let toml_args = args
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let s = format!("[tasks.x]\nargs = [{toml_args}]\ncmd = \"echo\"\n");
    xc_dev::config::tasks::parse(&s)
        .unwrap()
        .tasks
        .remove("x")
        .unwrap()
}

#[test]
fn binds_defaults_when_no_user_args() {
    let t = task_with_args(&["config=Debug"]);
    let m = xc_dev::runner::bind_args(&t, &[]).unwrap();
    assert_eq!(m.get("config").map(String::as_str), Some("Debug"));
}

#[test]
fn binds_positional_override() {
    let t = task_with_args(&["config=Debug"]);
    let m = xc_dev::runner::bind_args(&t, &["Release".into()]).unwrap();
    assert_eq!(m.get("config").map(String::as_str), Some("Release"));
}

#[test]
fn binds_required_arg() {
    let t = task_with_args(&["package"]);
    let m = xc_dev::runner::bind_args(&t, &["Networking".into()]).unwrap();
    assert_eq!(m.get("package").map(String::as_str), Some("Networking"));
}

#[test]
fn missing_required_errors() {
    let t = task_with_args(&["package"]);
    let err = xc_dev::runner::bind_args(&t, &[]).unwrap_err();
    assert!(err.to_string().contains("expects"), "got: {err}");
    assert!(err.to_string().contains("0"), "got: {err}");
}

#[test]
fn extra_args_error() {
    let t = task_with_args(&["package"]);
    let err = xc_dev::runner::bind_args(&t, &["a".into(), "b".into()]).unwrap_err();
    assert!(err.to_string().contains("got 2"), "got: {err}");
}

#[test]
fn mixed_required_optional() {
    let t = task_with_args(&["pkg", "config=Debug"]);
    // Only required
    let m = xc_dev::runner::bind_args(&t, &["Net".into()]).unwrap();
    assert_eq!(m.get("pkg").map(String::as_str), Some("Net"));
    assert_eq!(m.get("config").map(String::as_str), Some("Debug"));
    // Both
    let m = xc_dev::runner::bind_args(&t, &["Net".into(), "Release".into()]).unwrap();
    assert_eq!(m.get("config").map(String::as_str), Some("Release"));
}

#[test]
fn no_args_no_user_args() {
    let t = task_with_args(&[]);
    let m = xc_dev::runner::bind_args(&t, &[]).unwrap();
    assert!(m.is_empty());
}

fn write_minimal_project(dir: &std::path::Path, tasks_toml: &str) {
    let dotdir = dir.join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();
    std::fs::write(
        dotdir.join("project.toml"),
        r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "Hello"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "com.x"
product_name = "X"
"#,
    )
    .unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(dotdir.join("tasks.toml"), tasks_toml).unwrap();
}

#[test]
fn runs_chain_with_template_substitution() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_project(
        tmp.path(),
        r#"
[tasks.greet]
args = ["who=world"]
cmd = "echo {{scheme}} says hello {{who}}"
"#,
    );
    let code = xc_dev::runner::run(tmp.path(), "greet", &[], false).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn chain_stops_on_failure() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_project(
        tmp.path(),
        r#"
[tasks.fail]
cmd = "exit 3"

[tasks.after]
depends = ["fail"]
cmd = "echo should-not-run"
"#,
    );
    let code = xc_dev::runner::run(tmp.path(), "after", &[], false).unwrap();
    assert_eq!(code, 3);
}

#[test]
fn unknown_task_reports_error() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_project(
        tmp.path(),
        r#"[tasks.a]
cmd = "echo a"
"#,
    );
    let err = xc_dev::runner::run(tmp.path(), "nope", &[], false).unwrap_err();
    assert!(matches!(err, xc_dev::error::XcDevError::UnknownTask(_)));
}

#[test]
fn root_args_passed_dep_uses_defaults() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_project(
        tmp.path(),
        r#"
[tasks.dep]
args = ["mode=quiet"]
cmd = "test \"{{mode}}\" = quiet"

[tasks.main]
args = ["x=ok"]
depends = ["dep"]
cmd = "test \"{{x}}\" = override"
"#,
    );
    let code = xc_dev::runner::run(tmp.path(), "main", &["override".into()], false).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn missing_xc_dev_dir_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let err = xc_dev::runner::run(tmp.path(), "build", &[], false).unwrap_err();
    assert!(matches!(err, xc_dev::error::XcDevError::NoProjectDir(_)));
}

#[test]
fn unknown_var_in_cmd_errors_at_runtime() {
    let tmp = tempfile::tempdir().unwrap();
    write_minimal_project(
        tmp.path(),
        r#"[tasks.bad]
cmd = "echo {{nope}}"
"#,
    );
    let err = xc_dev::runner::run(tmp.path(), "bad", &[], false).unwrap_err();
    assert!(matches!(err, xc_dev::error::XcDevError::UnknownVar { .. }));
}

#[test]
fn verbose_echoes_command_to_stderr() {
    use assert_cmd::Command;
    let tmp = tempfile::tempdir().unwrap();
    let dotdir = tmp.path().join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();
    std::fs::write(
        dotdir.join("project.toml"),
        r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "Hello"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "com.x"
product_name = "X"
"#,
    )
    .unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(
        dotdir.join("tasks.toml"),
        r#"
[tasks.greet]
cmd = "echo hello from {{scheme}}"
"#,
    )
    .unwrap();

    let assert = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args(["-v", "greet"])
        .assert()
        .success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("+ greet"),
        "expected `+ greet` line in stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("+ echo hello from Hello"),
        "expected the expanded cmd echoed to stderr:\n{stderr}"
    );
}

#[test]
fn git_var_outside_repo_reports_var_unavailable() {
    let tmp = tempfile::tempdir().unwrap();
    let dotdir = tmp.path().join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();
    std::fs::write(
        dotdir.join("project.toml"),
        r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "X"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "com.x"
product_name = "X"
"#,
    )
    .unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(
        dotdir.join("tasks.toml"),
        r#"
[tasks.show-branch]
cmd = "echo {{git_branch}}"
"#,
    )
    .unwrap();
    let err = xc_dev::runner::run(tmp.path(), "show-branch", &[], false).unwrap_err();
    match err {
        xc_dev::error::XcDevError::VarUnavailable { name, .. } => {
            assert_eq!(name, "git_branch");
        }
        other => panic!("expected VarUnavailable, got {other:?}"),
    }
}

#[test]
fn no_verbose_does_not_echo() {
    use assert_cmd::Command;
    let tmp = tempfile::tempdir().unwrap();
    let dotdir = tmp.path().join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();
    std::fs::write(
        dotdir.join("project.toml"),
        r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "Hello"
configuration = "Debug"

[project.configurations.Debug]
bundle_id = "com.x"
product_name = "X"
"#,
    )
    .unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(
        dotdir.join("tasks.toml"),
        r#"
[tasks.silent]
cmd = "true"
"#,
    )
    .unwrap();

    let assert = Command::cargo_bin("xc-dev")
        .unwrap()
        .current_dir(tmp.path())
        .args(["silent"])
        .assert()
        .success();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        !stderr.contains("+ "),
        "expected no `+ ` prefix on stderr without -v:\n{stderr}"
    );
}

fn write_project_with_config_overrides(dir: &std::path::Path, tasks_toml: &str) {
    let dotdir = dir.join(".xc-dev");
    std::fs::create_dir_all(&dotdir).unwrap();
    std::fs::write(
        dotdir.join("project.toml"),
        r#"
schema_version = 1
[project]
path = "X.xcworkspace"
scheme = "Hello"
configuration = "Debug"

[project.configurations.Debug]
bundle_id    = "com.x.debug"
product_name = "X Debug"

[project.configurations.Release]
bundle_id    = "com.x.release"
product_name = "X Release"
"#,
    )
    .unwrap();
    std::fs::write(dotdir.join("simulator.toml"), "schema_version = 1\n").unwrap();
    std::fs::write(dotdir.join("tasks.toml"), tasks_toml).unwrap();
}

#[test]
fn task_configuration_field_resolves_release_bundle_id() {
    let tmp = tempfile::tempdir().unwrap();
    write_project_with_config_overrides(
        tmp.path(),
        r#"
[tasks.show-id]
configuration = "Release"
cmd = "echo BUNDLE_ID={{bundle_id}}"
"#,
    );
    let code = xc_dev::runner::run(tmp.path(), "show-id", &[], false).unwrap();
    assert_eq!(code, 0);
}

#[test]
fn task_without_configuration_uses_project_default() {
    let tmp = tempfile::tempdir().unwrap();
    write_project_with_config_overrides(
        tmp.path(),
        r#"
[tasks.check-debug]
cmd = "test \"{{bundle_id}}\" = \"com.x.debug\""
"#,
    );
    let code = xc_dev::runner::run(tmp.path(), "check-debug", &[], false).unwrap();
    assert_eq!(
        code, 0,
        "expected Debug bundle_id when no task.configuration"
    );
}

#[test]
fn task_configuration_release_uses_release_bundle_id() {
    let tmp = tempfile::tempdir().unwrap();
    write_project_with_config_overrides(
        tmp.path(),
        r#"
[tasks.check-release]
configuration = "Release"
cmd = "test \"{{bundle_id}}\" = \"com.x.release\""
"#,
    );
    let code = xc_dev::runner::run(tmp.path(), "check-release", &[], false).unwrap();
    assert_eq!(
        code, 0,
        "expected Release bundle_id when task.configuration = Release"
    );
}
