//! Batch conversion — process multiple files.

use std::path::{Path, PathBuf};
use tracing::info;

use crate::cli::args::Args;
use crate::config::Config;
use crate::error::CliError;

use super::convert::ConversionStats;

/// Convert multiple files, optionally in parallel.
pub fn convert_batch(
    inputs: &[PathBuf],
    args: &Args,
    config: &Config,
    output_dir: &Path,
) -> Result<(), CliError> {
    use rayon::prelude::*;

    if inputs.is_empty() {
        return Ok(());
    }

    let results: Vec<(usize, Result<ConversionStats, String>)> = if config.parallel.files {
        inputs
            .par_iter()
            .enumerate()
            .map(|(i, input)| {
                let mut output = output_dir.to_path_buf();
                output.push(input.file_stem().unwrap_or_default());
                output.set_extension("sup");
                (
                    i,
                    super::convert::convert_file(input, &output, args, config)
                        .map_err(|e| e.to_string()),
                )
            })
            .collect()
    } else {
        let pb = if args.quiet {
            indicatif::ProgressBar::hidden()
        } else {
            crate::cli::progress::create(inputs.len() as u64, "Batch converting")
        };
        let results: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let mut output = output_dir.to_path_buf();
                output.push(input.file_stem().unwrap_or_default());
                output.set_extension("sup");
                pb.inc(1);
                (
                    i,
                    super::convert::convert_file(input, &output, args, config)
                        .map_err(|e| e.to_string()),
                )
            })
            .collect();
        pb.finish_and_clear();
        results
    };

    let mut successes = 0;
    let mut failures = 0;
    for (i, result) in &results {
        match result {
            Ok(stats) => {
                info!(
                    "[{i}] {} events → {} bytes",
                    stats.events_processed, stats.output_size
                );
                successes += 1;
            }
            Err(e) => {
                info!("[{i}] FAILED: {e}");
                failures += 1;
            }
        }
    }

    info!("Batch complete: {successes} succeeded, {failures} failed");

    if failures > 0 {
        Err(CliError::BatchFailed {
            successes,
            failures,
        })
    } else {
        Ok(())
    }
}
