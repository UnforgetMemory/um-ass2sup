use clap::Parser;
use std::process;

fn main() {
    let args = ass2sup_cli::Args::parse();
    if let Err(e) = ass2sup_cli::run(args) {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
