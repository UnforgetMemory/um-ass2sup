//! ass2sup — ASS/SSA/SRT to Blu-ray SUP/PGS subtitle converter CLI.
//!
//! This crate provides the command-line interface for converting subtitle
//! formats.  It orchestrates parsing, validation, rendering, colour
//! quantisation, and encoding into a single pass.

#![warn(missing_docs)]

pub mod cli;
pub mod config;
pub mod error;
/// PaddleOCR integration for verification testing.
pub mod ocr;
pub mod pipeline;
pub mod telemetry;
pub mod util;

pub use error::CliError;

use std::path::{Path, PathBuf};

use tracing::{error, info, warn};
use walkdir::WalkDir;

use cli::args::Args;
use config::Config;

/// Maximum input file size in bytes (100 MiB).
pub const MAX_INPUT_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// Colour output mode detection.
pub fn should_use_color(color: &str) -> bool {
    match color {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stdout()),
    }
}

/// Return the current process RSS in bytes (Linux only).
pub fn current_rss_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    let kb = rest.trim().trim_end_matches("kB").trim();
                    if let Ok(value) = kb.parse::<u64>() {
                        return value * 1024;
                    }
                }
            }
        }
    }
    0
}

/// Collect input files from positional args and/or --glob pattern.
fn collect_input_files(args: &Args) -> Vec<PathBuf> {
    let mut inputs = args.input.clone();

    if let Some(ref pattern) = args.glob {
        let mut globbed = if args.recursive {
            collect_recursive_glob(pattern)
        } else {
            collect_flat_glob(pattern)
        };
        globbed.sort();
        if let Some(max) = args.max_files {
            globbed.truncate(max);
        }
        inputs.extend(globbed);
    }

    inputs
}

fn collect_flat_glob(pattern: &str) -> Vec<PathBuf> {
    match glob::glob(pattern) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.is_file())
            .collect(),
        Err(e) => {
            warn!("Invalid glob pattern '{pattern}': {e}");
            Vec::new()
        }
    }
}

fn collect_recursive_glob(pattern: &str) -> Vec<PathBuf> {
    let p = Path::new(pattern);
    let base_dir = p.parent().unwrap_or(Path::new("."));
    let file_pattern = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| pattern.to_string());

    let globber = match glob::Pattern::new(&file_pattern) {
        Ok(p) => p,
        Err(e) => {
            warn!("Invalid glob pattern '{file_pattern}': {e}");
            return Vec::new();
        }
    };

    WalkDir::new(base_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| globber.matches(&e.file_name().to_string_lossy()))
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Run the CLI conversion with parsed arguments.
pub fn run(args: Args) -> Result<(), CliError> {
    telemetry::init(args.verbose, args.quiet, args.debug, &args.color);
    let use_color = should_use_color(&args.color);
    let inputs = collect_input_files(&args);

    if inputs.is_empty() {
        error!("No input files found. Provide positional args or use --glob.");
        return Err(CliError::NoInputFiles);
    }

    // Size check
    for input in &inputs {
        let size = std::fs::metadata(input)
            .map_err(|e| CliError::ReadError(input.display().to_string(), e.to_string()))?
            .len();
        if size > MAX_INPUT_SIZE_BYTES {
            return Err(CliError::InputTooLarge {
                path: input.display().to_string(),
                size,
                max: MAX_INPUT_SIZE_BYTES,
            });
        }
    }

    let config = Config::from_args(&args);

    // Validate resolution early so bad -r values fail immediately
    if let Some(ref res_str) = args.resolution {
        config::resolution::Resolution::parse(res_str).map_err(|e| {
            CliError::InvalidResolution {
                input: res_str.clone(),
                message: e,
            }
        })?;
    }

    // --check mode
    if args.check {
        return pipeline::check::run_check(&inputs, &args);
    }

    // --to-srt mode
    if args.to_srt {
        for input in &inputs {
            let output = args.output.clone().unwrap_or_else(|| {
                let mut o = input.clone();
                o.set_extension("srt");
                o
            });
            pipeline::srt::convert_to_srt(input, &output, &args, &config)?;
        }
        return Ok(());
    }

    // --to-bdn mode
    if args.to_bdn {
        for input in &inputs {
            let stem = input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("subtitle");
            let output_dir = if let Some(ref dir) = args.output_dir {
                dir.join(stem)
            } else {
                PathBuf::from(stem)
            };
            pipeline::convert::convert_to_bdn(input, &output_dir, &args, &config)?;
        }
        return Ok(());
    }

    info!(
        "ass2sup v{} — ASS/SRT to SUP/PGS converter",
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "ass2sup v{} — ASS/SRT to SUP/PGS converter",
        env!("CARGO_PKG_VERSION")
    );

    // Warn about deprecated --parallel-frames flag
    #[allow(deprecated)]
    if args.parallel_frames {
        warn!("--parallel-frames is deprecated and ignored in frame-driven mode");
    }

    // Single file mode
    if inputs.len() == 1 {
        let input = &inputs[0];
        let output = args.output.clone().unwrap_or_else(|| {
            let mut o = input.clone();
            o.set_extension("sup");
            o
        });

        match pipeline::convert::convert_file(input, &output, &args, &config) {
            Ok(stats) => {
                let msg = format!(
                    "{} Converted {} events ({} frames) → {} ({} bytes)",
                    if use_color { "✅" } else { "[OK]" },
                    stats.events_processed,
                    stats.frames_encoded,
                    output.display(),
                    stats.output_size,
                );
                info!("{msg}");
                println!("{msg}");
            }
            Err(e) => {
                let msg = format!(
                    "{}Conversion failed: {}",
                    if use_color { "❌ " } else { "[FAIL] " },
                    e
                );
                error!("{msg}");
                eprintln!("{msg}");
                return Err(e);
            }
        }
        return Ok(());
    }

    // Batch mode
    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            CliError::CreateDirError(output_dir.display().to_string(), e.to_string())
        })?;
    }

    pipeline::batch::convert_batch(&inputs, &args, &config, &output_dir)
}
