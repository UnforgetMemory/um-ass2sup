//! Check mode — parse and validate only.

use tracing::info;

use crate::cli::args::Args;
use crate::error::CliError;

/// Parse subtitle files without writing output. Exits 0 if all OK, 1 on error.
pub fn run_check(inputs: &[PathBuf], _args: &Args) -> Result<(), CliError> {
    for input in inputs {
        super::convert::ConversionPipeline::parse_input(input)?;
        info!("{} — OK", input.display());
    }
    Ok(())
}

use std::path::PathBuf;
