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
