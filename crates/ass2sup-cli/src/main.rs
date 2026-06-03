use std::path::{Path, PathBuf};
use std::process;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use tracing::{error, info, warn};

use ass_parser::{AssFile, SubtitleFormat};
use color_quantizer::{quantize_with_palette, DitherMethod, Quantizer, Rgba};
use pgs_encoder::PgsEncoder;
use subtitle_renderer::{RenderConfig, Renderer};
use subtitle_validator::{OverlapConfig, OverlapSeverity, Validator};

/// ASS/SRT to SUP/PGS converter
#[derive(Parser, Debug)]
#[command(name = "ass2sup", version, about, long_about = None)]
struct Args {
    /// Input subtitle file(s) (ASS/SSA/SRT)
    #[arg(required = true)]
    input: Vec<PathBuf>,

    /// Output SUP file path (single file mode)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output directory (batch mode)
    #[arg(short = 'd', long)]
    output_dir: Option<PathBuf>,

    /// Display resolution (WIDTHxHEIGHT)
    #[arg(short, long, default_value = "1920x1080")]
    resolution: String,

    /// Frames per second
    #[arg(short, long, default_value = "23.976")]
    fps: f64,

    /// Run validation before conversion
    #[arg(long)]
    validate: bool,

    /// Enable overlap warning detection
    #[arg(long)]
    overlap_warn: bool,

    /// Overlap detection mode (strict/lenient)
    #[arg(long, default_value = "lenient")]
    overlap_mode: String,

    /// Quantizer algorithm (median-cut)
    #[arg(long, default_value = "median-cut")]
    quantizer: String,

    /// Maximum colors in palette (1-255)
    #[arg(long, default_value = "255")]
    max_colors: usize,

    /// Dithering method (none/floyd-steinberg/ordered)
    #[arg(long, default_value = "floyd-steinberg")]
    dither: String,

    /// Default font name for SRT input
    #[arg(long, default_value = "Arial")]
    font: String,

    /// Default font size for SRT input
    #[arg(long, default_value = "48.0")]
    font_size: f64,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Process files in parallel (batch mode)
    #[arg(short, long)]
    parallel: bool,

    /// Force conversion even if validation fails
    #[arg(long)]
    force: bool,

    /// Dry run: parse and validate only, don't write output
    #[arg(long)]
    dry_run: bool,

    /// Render frames in parallel using rayon (single-file mode)
    #[arg(long)]
    parallel_frames: bool,

    /// Suppress progress bar
    #[arg(long)]
    quiet: bool,
}

struct Resolution {
    width: u32,
    height: u32,
}

fn parse_resolution(s: &str) -> Result<Resolution, String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid resolution format '{}'. Expected WIDTHxHEIGHT", s));
    }
    let width = parts[0].parse::<u32>().map_err(|_| format!("Invalid width '{}'", parts[0]))?;
    let height = parts[1].parse::<u32>().map_err(|_| format!("Invalid height '{}'", parts[1]))?;
    if width == 0 || height == 0 {
        return Err("Resolution dimensions must be > 0".to_string());
    }
    Ok(Resolution { width, height })
}

fn setup_logging(verbose: bool) {
    let level = if verbose {
        tracing::level_filters::LevelFilter::DEBUG
    } else {
        tracing::level_filters::LevelFilter::INFO
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

fn create_progress_bar(len: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb.set_message(message.to_string());
    pb
}

fn convert_file(
    input: &Path,
    output: &Path,
    args: &Args,
    res: &Resolution,
) -> Result<ConversionStats, String> {
    info!("Processing: {}", input.display());

    let content = std::fs::read_to_string(input)
        .map_err(|e| format!("Failed to read input file: {}", e))?;

    let format = SubtitleFormat::detect(input);
    info!("Detected format: {:?}", format);

    let mut ass = AssFile::parse(&content)
        .map_err(|e| format!("Failed to parse subtitle: {}", e))?;

    info!(
        "Parsed: {} styles, {} events",
        ass.styles.len(),
        ass.events.len()
    );

    if args.validate || args.overlap_warn {
        let overlap_config = match args.overlap_mode.as_str() {
            "strict" => OverlapConfig::strict(),
            _ => OverlapConfig::lenient(),
        };

        let validator = if args.overlap_warn {
            Validator::new().with_overlap_config(overlap_config)
        } else {
            Validator::new()
        };

        let report = validator.validate(&ass);

        if !report.is_valid {
            for finding in report.errors() {
                error!("  [{}] {}", finding.rule_id, finding.message);
            }
        }

        for finding in report.warnings() {
            warn!("  [{}] {}", finding.rule_id, finding.message);
        }

        if args.overlap_warn && !report.overlaps.is_empty() {
            warn!("Detected {} overlap(s):", report.overlaps.len());
            for overlap in &report.overlaps {
                warn!(
                    "  Events {} & {} overlap by {}ms ({})",
                    overlap.event_a_idx,
                    overlap.event_b_idx,
                    overlap.overlap_duration,
                    match overlap.severity {
                        OverlapSeverity::Critical => "CRITICAL",
                        OverlapSeverity::High => "HIGH",
                        OverlapSeverity::Medium => "MEDIUM",
                        OverlapSeverity::Low => "LOW",
                    }
                );
            }
        }

        info!("{}", report.summary());

        if !report.is_valid && !args.force {
            return Err("Validation failed. Use --force to override.".to_string());
        }
    }

    if args.dry_run {
        info!("Dry run complete — skipping render/encode");
        return Ok(ConversionStats {
            events_processed: ass.events.len() as u64,
            frames_encoded: 0,
            output_size: 0,
        });
    }

    let render_config = RenderConfig {
        width: res.width,
        height: res.height,
        script_width: ass.script_info.play_res_x,
        script_height: ass.script_info.play_res_y,
        default_font: args.font.clone(),
        default_font_size: args.font_size as f32,
    };

    let mut renderer = Renderer::new(render_config);

    // Load embedded fonts from ASS [Fonts] section
    let font_data_list = ass.load_embedded_fonts(
        input.parent().unwrap_or(std::path::Path::new("."))
    );
    for (_font_name, font_data) in font_data_list {
        let _id = renderer.font_manager_mut().load_font_data(font_data);
    }

    let dither_method = match args.dither.as_str() {
        "none" => DitherMethod::None,
        "ordered" => DitherMethod::Ordered,
        _ => DitherMethod::FloydSteinberg,
    };

    let use_palette_reuse = args.quantizer == "median-cut";
    let quantizer = Quantizer::new(args.max_colors).with_dither(dither_method);

    let mut pgs_encoder = PgsEncoder::new(res.width as u16, res.height as u16, args.fps);

    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    let total = dialogues.len() as u64;

    if total == 0 {
        warn!("No dialogue events found");
        std::fs::write(output, Vec::<u8>::new())
            .map_err(|e| format!("Failed to write output: {}", e))?;
        return Ok(ConversionStats {
            events_processed: 0,
            frames_encoded: 0,
            output_size: 0,
        });
    }

    let quiet = args.quiet;
    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        create_progress_bar(total, "Converting")
    };

    let mut frames_encoded = 0u64;

    let frame_data: Vec<_> = if args.parallel_frames && dialogues.len() > 1 {
        dialogues
            .par_iter()
            .map(|event| {
                let pts_ms = event.start.as_ms();
                let duration_ms = event.duration_ms();

                let frame = renderer.render_ass(&ass, pts_ms);
                (event.clone(), frame, pts_ms, duration_ms)
            })
            .collect()
    } else {
        dialogues
            .iter()
            .map(|event| {
                let pts_ms = event.start.as_ms();
                let duration_ms = event.duration_ms();

                let frame = renderer.render_ass(&ass, pts_ms);
                (event.clone(), frame, pts_ms, duration_ms)
            })
            .collect()
    };

    let mut all_segments = Vec::new();
    let mut prev_palette: Option<Vec<Rgba>> = None;

    for (_event, frame_opt, pts_ms, duration_ms) in &frame_data {
        if let Some(frame) = frame_opt {
            let quantized = if use_palette_reuse {
                let prev = prev_palette.as_deref();
                let q = quantize_with_palette(
                    &frame.bitmap,
                    frame.width,
                    frame.height,
                    prev,
                    args.max_colors,
                    dither_method,
                );
                prev_palette = Some(q.palette.clone());
                q
            } else {
                let q = quantizer.quantize(&frame.bitmap, frame.width, frame.height);
                prev_palette = Some(q.palette.clone());
                q
            };

            let segments = pgs_encoder.encode_frame(&quantized, *pts_ms, *duration_ms);
            all_segments.extend(segments);
            frames_encoded += 1;
        }

        pb.inc(1);
    }

    pb.finish_with_message("Done");

    let sup_data = {
        let sup_file = pgs_encoder::types::SupFile {
            segments: all_segments,
        };
        sup_file.to_bytes()
    };

    let output_size = sup_data.len();
    std::fs::write(output, &sup_data)
        .map_err(|e| format!("Failed to write output: {}", e))?;

    info!(
        "Output: {} ({} bytes, {} frames)",
        output.display(),
        output_size,
        frames_encoded
    );

    Ok(ConversionStats {
        events_processed: total,
        frames_encoded,
        output_size,
    })
}

#[derive(Debug)]
struct ConversionStats {
    events_processed: u64,
    frames_encoded: u64,
    output_size: usize,
}

fn main() {
    let args = Args::parse();
    setup_logging(args.verbose);

    let res = match parse_resolution(&args.resolution) {
        Ok(r) => r,
        Err(e) => {
            error!("Invalid resolution: {}", e);
            process::exit(1);
        }
    };

    info!(
        "ass2sup v{} - ASS/SRT to SUP/PGS converter",
        env!("CARGO_PKG_VERSION")
    );
    info!("Resolution: {}x{}, FPS: {}", res.width, res.height, args.fps);

    if args.input.len() == 1 {
        let input = &args.input[0];
        let output = args.output.clone().unwrap_or_else(|| {
            let mut out = input.clone();
            out.set_extension("sup");
            out
        });

        match convert_file(input, &output, &args, &res) {
            Ok(stats) => {
                info!(
                    "✅ Converted {} events ({} frames) → {} ({} bytes)",
                    stats.events_processed,
                    stats.frames_encoded,
                    output.display(),
                    stats.output_size
                );
            }
            Err(e) => {
                error!("❌ Conversion failed: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    let output_dir = args.output_dir.clone().unwrap_or_else(|| PathBuf::from("."));
    if !output_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&output_dir) {
            error!("Failed to create output directory: {}", e);
            process::exit(1);
        }
    }

    let inputs: Vec<_> = args.input.iter().collect();

    let results: Vec<(usize, Result<ConversionStats, String>)> = if args.parallel {
        inputs
            .par_iter()
            .enumerate()
            .map(|(i, input)| {
                let mut output = output_dir.clone();
                output.push(input.file_stem().unwrap_or_default());
                output.set_extension("sup");
                (i, convert_file(input, &output, &args, &res))
            })
            .collect()
    } else {
        let pb = if args.quiet {
            ProgressBar::hidden()
        } else {
            create_progress_bar(inputs.len() as u64, "Batch converting")
        };
        let results: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let mut output = output_dir.clone();
                output.push(input.file_stem().unwrap_or_default());
                output.set_extension("sup");
                let result = convert_file(input, &output, &args, &res);
                pb.inc(1);
                (i, result)
            })
            .collect();
        pb.finish_with_message("Done");
        results
    };

    let mut successes = 0;
    let mut failures = 0;

    for (i, result) in results {
        match result {
            Ok(stats) => {
                info!(
                    "✅ [{}] {} events ({} frames) → {} bytes",
                    inputs[i].display(),
                    stats.events_processed,
                    stats.frames_encoded,
                    stats.output_size
                );
                successes += 1;
            }
            Err(e) => {
                error!("❌ [{}] {}", inputs[i].display(), e);
                failures += 1;
            }
        }
    }

    info!(
        "Batch complete: {} succeeded, {} failed",
        successes, failures
    );

    if failures > 0 {
        process::exit(1);
    }
}
