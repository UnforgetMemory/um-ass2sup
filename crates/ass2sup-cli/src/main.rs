use clap::Parser;
use std::process;

fn main() {
    let args = ass2sup_cli::Args::parse();
    if ass2sup_cli::run(args).is_err() {
        process::exit(1);
    }
}
