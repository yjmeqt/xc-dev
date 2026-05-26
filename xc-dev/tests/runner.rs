use xc_dev::config::tasks;
use xc_dev::runner;

fn parse(toml: &str) -> tasks::Tasks { tasks::parse(toml).unwrap() }

#[test]
fn linear_depends() {
    let t = parse(r#"
[tasks.a]
cmd = "echo a"

[tasks.b]
depends = ["a"]
cmd = "echo b"

[tasks.c]
depends = ["b"]
cmd = "echo c"
"#);
    let order = runner::resolve_chain("c", &t).unwrap();
    assert_eq!(order, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
}

#[test]
fn cycle_detected() {
    let t = parse(r#"
[tasks.a]
depends = ["b"]
cmd = "echo a"

[tasks.b]
depends = ["a"]
cmd = "echo b"
"#);
    let err = runner::resolve_chain("a", &t).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("cyclic"),
            "expected cyclic error, got: {err}");
}

#[test]
fn unknown_task_in_depends() {
    let t = parse(r#"
[tasks.a]
depends = ["nope"]
cmd = "echo a"
"#);
    let err = runner::resolve_chain("a", &t).unwrap_err();
    assert!(err.to_string().contains("nope"),
            "expected error mentioning `nope`, got: {err}");
}

#[test]
fn unknown_root_task() {
    let t = parse(r#"[tasks.a]
cmd = "echo a"
"#);
    let err = runner::resolve_chain("zzz", &t).unwrap_err();
    assert!(matches!(err, xc_dev::error::XcDevError::UnknownTask(ref n) if n == "zzz"),
            "expected UnknownTask(\"zzz\"), got: {err:?}");
}

#[test]
fn diamond_dependency_runs_once() {
    let t = parse(r#"
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
"#);
    let order = runner::resolve_chain("d", &t).unwrap();
    assert_eq!(order, vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()]);
}

#[test]
fn single_task_no_depends() {
    let t = parse(r#"[tasks.solo]
cmd = "echo solo"
"#);
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
    let code = xc_dev::exec::run_script("VAR=hi\ntest \"$VAR\" = hi", None, &BTreeMap::new()).unwrap();
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
    let toml_args = args.iter().map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ");
    let s = format!(
        "[tasks.x]\nargs = [{toml_args}]\ncmd = \"echo\"\n"
    );
    xc_dev::config::tasks::parse(&s).unwrap().tasks.remove("x").unwrap()
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
