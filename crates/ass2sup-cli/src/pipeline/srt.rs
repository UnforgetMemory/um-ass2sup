//! ASS/SSA → SRT format downgrade.

use std::path::Path;
use tracing::info;

use crate::cli::args::Args;
use crate::config::Config;
use crate::error::CliError;

use super::convert::ConversionStats;

/// Convert ASS/SSA document to SRT and write to disk.
pub fn convert_to_srt(
    input: &Path,
    output: &Path,
    _args: &Args,
    _config: &Config,
) -> Result<ConversionStats, CliError> {
    let doc = super::convert::ConversionPipeline::parse_input(input)?;

    let srt_content = ass_core::srt::to_srt(&doc);
    std::fs::write(output, &srt_content)
        .map_err(|e| CliError::Conversion(format!("Failed to write SRT: {e}")))?;

    info!("{} → {}", input.display(), output.display());

    Ok(ConversionStats {
        events_processed: doc.events.len() as u64,
        ..Default::default()
    })
}
