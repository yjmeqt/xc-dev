use xc_dev::{builtins, cli, runner};

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let inv = match cli::parse(&argv) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("xc-dev: {e}");
            std::process::exit(2);
        }
    };
    let exit_code = run(inv);
    std::process::exit(exit_code);
}

fn run(inv: cli::Invocation) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let result: xc_dev::error::Result<i32> = match inv.command {
        cli::Command::List => builtins::list::run(&cwd),
        cli::Command::Get { key } => builtins::get::run(&cwd, &key),
        cli::Command::Doctor => builtins::doctor::run(&cwd),
        cli::Command::ProjInit {
            force,
            yes,
            path,
            scheme,
        } => builtins::init::run(&cwd, force, yes, path.as_deref(), scheme.as_deref()),
        cli::Command::SimBake { label } => builtins::sim::bake(&cwd, label.as_deref()),
        cli::Command::Task { name, args } => runner::run(&cwd, &name, &args, inv.verbose),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("xc-dev: {e}");
            1
        }
    }
}
