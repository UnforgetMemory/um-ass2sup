use clap::Parser;

fn main() {
    let args = ass2sup_cli::cli::args::Args::parse();
    if let Err(e) = ass2sup_cli::run(args) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
