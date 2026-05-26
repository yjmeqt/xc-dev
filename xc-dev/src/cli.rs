#[derive(Debug)]
pub struct Invocation {
    pub verbose: bool,
    pub command: Command,
}

#[derive(Debug)]
pub enum Command {
    List,
    Task { name: String, args: Vec<String> },
    Get { key: String },
    Doctor,
    ProjInit {
        force: bool,
        yes: bool,
        path: Option<String>,
        scheme: Option<String>,
    },
}

pub fn parse(argv: &[String]) -> Result<Invocation, String> {
    let mut args: Vec<String> = argv.iter().skip(1).cloned().collect();
    let mut verbose = false;
    args.retain(|a| {
        if a == "-v" || a == "--verbose" { verbose = true; false } else { true }
    });

    if args.is_empty() {
        return Ok(Invocation { verbose, command: Command::List });
    }

    let first = args.remove(0);
    let command = match first.as_str() {
        ":list" => Command::List,
        ":get" => {
            let key = args.first().cloned().ok_or("`:get` requires a key")?;
            Command::Get { key }
        }
        ":doctor" => Command::Doctor,
        ":proj" => {
            let sub = args.first().cloned().ok_or("`:proj` requires a subcommand")?;
            if sub != "init" { return Err(format!("unknown :proj subcommand `{sub}`")); }
            let rest = &args[1..];
            let mut force = false;
            let mut yes = false;
            let mut path: Option<String> = None;
            let mut scheme: Option<String> = None;
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--force" => { force = true; i += 1; }
                    "--yes" => { yes = true; i += 1; }
                    "--path" => {
                        path = Some(rest.get(i + 1).cloned()
                            .ok_or("--path requires a value")?);
                        i += 2;
                    }
                    "--scheme" => {
                        scheme = Some(rest.get(i + 1).cloned()
                            .ok_or("--scheme requires a value")?);
                        i += 2;
                    }
                    other => return Err(format!("unknown :proj init flag `{other}`")),
                }
            }
            Command::ProjInit { force, yes, path, scheme }
        }
        other if other.starts_with(':') => {
            return Err(format!("unknown built-in `{other}`"));
        }
        other => Command::Task { name: other.to_string(), args },
    };

    Ok(Invocation { verbose, command })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Invocation, String> {
        let v: Vec<String> = std::iter::once("xc-dev".to_string())
            .chain(args.iter().map(|s| s.to_string())).collect();
        parse(&v)
    }

    #[test]
    fn empty_argv_is_list() {
        let i = p(&[]).unwrap();
        assert!(matches!(i.command, Command::List));
    }

    #[test]
    fn task_with_args() {
        let i = p(&["build", "Release"]).unwrap();
        match i.command {
            Command::Task { name, args } => {
                assert_eq!(name, "build");
                assert_eq!(args, vec!["Release".to_string()]);
            }
            other => panic!("expected Task, got {other:?}"),
        }
    }

    #[test]
    fn verbose_is_stripped_anywhere() {
        let i = p(&["build", "-v", "Release"]).unwrap();
        assert!(i.verbose);
        match i.command {
            Command::Task { name, args } => {
                assert_eq!(name, "build");
                assert_eq!(args, vec!["Release".to_string()]);
            }
            other => panic!("expected Task, got {other:?}"),
        }
    }

    #[test]
    fn get_requires_key() {
        let err = p(&[":get"]).unwrap_err();
        assert!(err.contains("requires a key"));
    }

    #[test]
    fn unknown_builtin_errors() {
        let err = p(&[":wat"]).unwrap_err();
        assert!(err.contains("unknown built-in"));
    }

    #[test]
    fn proj_init_parses_flags() {
        let i = p(&[":proj", "init", "--path", "X.xcworkspace", "--scheme", "Hello", "--yes", "--force"]).unwrap();
        match i.command {
            Command::ProjInit { force, yes, path, scheme } => {
                assert!(force); assert!(yes);
                assert_eq!(path.as_deref(), Some("X.xcworkspace"));
                assert_eq!(scheme.as_deref(), Some("Hello"));
            }
            other => panic!("expected ProjInit, got {other:?}"),
        }
    }

    #[test]
    fn proj_subcommand_required() {
        let err = p(&[":proj"]).unwrap_err();
        assert!(err.contains(":proj"));
    }
}
