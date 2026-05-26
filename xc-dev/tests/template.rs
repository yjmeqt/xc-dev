use xc_dev::template;

fn make_resolver<'a>(map: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| map.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

#[test]
fn substitutes_simple_var() {
    let r = make_resolver(&[("scheme", "BedTerm")]);
    let out = template::render("xcodebuild -scheme {{scheme}}", "build", &r).unwrap();
    assert_eq!(out, "xcodebuild -scheme BedTerm");
}

#[test]
fn dotted_names_work() {
    let r = make_resolver(&[("sim.ipad.id", "DEF-456")]);
    let out = template::render("launch {{sim.ipad.id}}", "run", &r).unwrap();
    assert_eq!(out, "launch DEF-456");
}

#[test]
fn unknown_var_is_error() {
    let r = make_resolver(&[]);
    let err = template::render("{{foo}}", "x", &r).unwrap_err();
    match err {
        xc_dev::error::XcDevError::UnknownVar { name, task } => {
            assert_eq!(name, "foo"); assert_eq!(task, "x");
        }
        e => panic!("wrong error: {e}"),
    }
}

#[test]
fn passes_through_non_template() {
    let r = make_resolver(&[]);
    let out = template::render("hello world { not a var }", "x", &r).unwrap();
    assert_eq!(out, "hello world { not a var }");
}

#[test]
fn multiple_vars_one_line() {
    let r = make_resolver(&[("a", "1"), ("b", "2")]);
    let out = template::render("{{a}}-{{b}}-{{a}}", "x", &r).unwrap();
    assert_eq!(out, "1-2-1");
}

#[test]
fn trims_whitespace_inside_braces() {
    let r = make_resolver(&[("scheme", "X")]);
    let out = template::render("{{ scheme }}", "x", &r).unwrap();
    assert_eq!(out, "X");
}
