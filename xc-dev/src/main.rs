mod error;

fn main() {
    if let Err(e) = run() {
        eprintln!("xc-dev: {e}");
        std::process::exit(1);
    }
}

fn run() -> error::Result<()> {
    println!("xc-dev v0.1.0 (stub)");
    Ok(())
}
