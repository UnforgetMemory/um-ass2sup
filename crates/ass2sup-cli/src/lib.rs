//! CLI application wiring all ass2sup crates together.

#![warn(missing_docs)]
//!
//! This crate provides the command-line interface for converting ASS/SSA/SRT
//! subtitle files to Blu-ray PGS/SUP format. It orchestrates parsing,
//! validation, rendering, color quantization, and encoding in a single pass.
//!
//! # Usage
//!
//! ```text
//! ass2sup input.ass -o output.sup
//! ass2sup *.ass -d ./output/ --parallel
//! ass2sup input.ass -o output.sup -r 1920x1080 -f 29.97
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::Parser;
use glob::glob;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use tracing::{error, info, warn};
use walkdir::WalkDir;

use ass_parser::{AssFile, SubtitleFormat};
use bdn_xml::{BdnEvent, BdnXml};
use color_quantizer::{quantize_with_palette, DitherMethod, Quantizer, Rgba};
use pgs_encoder::PgsEncoder;
use subtitle_renderer::{FontManager, RenderConfig, Renderer};
use subtitle_validator::{OverlapConfig, OverlapSeverity, Validator};

pub mod ocr;

/// Maximum input file size in bytes (100 MiB).
///
/// Subtitle files are normally < 1 MiB. Anything over 100 MiB is almost
/// certainly a misuse (binary file, video, or attack). Refuse early with
/// [`CliError::InputTooLarge`] rather than allocating huge buffers.
pub const MAX_INPUT_SIZE_BYTES: u64 = 100 * 1024 * 1024;

/// ASS/SRT to SUP/PGS converter
#[derive(Parser, Debug)]
#[command(name = "ass2sup", version, about, long_about = None)]
pub struct Args {
    /// Input subtitle file(s) (ASS/SSA/SRT)
    #[arg(required_unless_present = "glob")]
    pub input: Vec<PathBuf>,

    /// Glob pattern for input files (alternative to positional args)
    #[arg(long)]
    pub glob: Option<String>,

    /// Traverse subdirectories when using --glob
    #[arg(long)]
    pub recursive: bool,

    /// Limit number of files processed when using --glob
    #[arg(long)]
    pub max_files: Option<usize>,

    /// Output SUP file path (single file mode)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output directory (batch mode)
    #[arg(short = 'd', long)]
    pub output_dir: Option<PathBuf>,

    /// Display resolution (WIDTHxHEIGHT).
    ///
    /// If not specified, uses PlayResX/PlayResY from [Script Info] section.
    /// Falls back to 1920x1080 if Script Info resolution is missing or zero.
    #[arg(short, long)]
    pub resolution: Option<String>,

    /// Frames per second
    #[arg(short, long, default_value = "23.976")]
    pub fps: f64,

    /// Run validation before conversion
    #[arg(long)]
    pub validate: bool,

    /// Enable overlap warning detection
    #[arg(long)]
    pub overlap_warn: bool,

    /// Overlap detection mode (strict/lenient)
    #[arg(long, default_value = "lenient")]
    pub overlap_mode: String,

    /// Quantizer algorithm (median-cut)
    #[arg(long, default_value = "median-cut")]
    pub quantizer: String,

    /// Maximum colors in palette (1-255)
    #[arg(long, default_value = "255")]
    pub max_colors: usize,

    /// Dithering method (none/floyd-steinberg/ordered)
    #[arg(long, default_value = "floyd-steinberg")]
    pub dither: String,

    /// Default font name for SRT input
    #[arg(long, default_value = "Arial")]
    pub font: String,

    /// Default font size for SRT input
    #[arg(long, default_value = "48.0")]
    pub font_size: f64,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Process files in parallel (batch mode)
    #[arg(short, long)]
    pub parallel: bool,

    /// Force conversion even if validation fails
    #[arg(long)]
    pub force: bool,

    /// Dry run: parse and validate only, don't write output
    #[arg(long)]
    pub dry_run: bool,

    /// Render frames in parallel using rayon (single-file mode)
    #[arg(long)]
    pub parallel_frames: bool,

    /// Suppress progress bar
    #[arg(long)]
    pub quiet: bool,

    /// Parse and validate only, don't convert (exit 0 if OK, 1 if errors)
    #[arg(long)]
    pub check: bool,

    /// Color output mode (auto/always/never)
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// Convert to SRT format instead of SUP/PGS
    #[arg(long)]
    pub to_srt: bool,

    /// Convert to BDN XML + PNG format (Blu-ray authoring)
    #[arg(long, conflicts_with = "to_srt")]
    pub to_bdn: bool,

    /// Skip font availability check (fonts missing from the system will silently
    /// fall back to a substitute, potentially producing blank subtitle output)
    #[arg(long)]
    pub no_check_fonts: bool,

    /// Per-style font fallback map. Each entry is "StyleName:fallback1,fallback2".
    /// Can be repeated multiple times.
    #[arg(long, value_name = "STYLE:FALLBACKS")]
    pub font_map: Vec<String>,
}

/// Output display resolution parsed from `WIDTHxHEIGHT` strings.
#[derive(Debug)]
pub struct Resolution {
    /// Display width in pixels.
    pub width: u32,
    /// Display height in pixels.
    pub height: u32,
}

/// Per-file conversion statistics returned by [`convert_file`].
#[derive(Debug)]
pub struct ConversionStats {
    /// Number of dialogue events processed from the input.
    pub events_processed: u64,
    /// Number of PGS frames successfully encoded.
    pub frames_encoded: u64,
    /// Size of the output SUP file in bytes.
    pub output_size: usize,
}

/// Errors that can occur during CLI execution.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Invalid resolution format or dimensions.
    #[error("Invalid resolution '{input}': {message}")]
    InvalidResolution {
        /// The malformed input string the user provided.
        input: String,
        /// Human-readable explanation of why the value is invalid.
        message: String,
    },

    /// Input file exceeds the size limit.
    #[error("Input '{path}' is {size} bytes which exceeds the {max} byte limit")]
    InputTooLarge {
        /// Path of the oversized input.
        path: String,
        /// Actual file size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        max: u64,
    },

    /// Conversion failed for a file.
    #[error("Conversion failed: {0}")]
    Conversion(String),

    /// Failed to read an input file.
    #[error("Cannot read '{0}': {1}")]
    ReadError(String, String),

    /// Failed to parse a subtitle file.
    #[error("Parse error in '{0}': {1}")]
    ParseError(String, String),

    /// Failed to create the output directory.
    #[error("Failed to create output directory '{0}': {1}")]
    CreateDirError(String, String),

    /// No input files found.
    #[error("No input files found. Provide positional args or use --glob.")]
    NoInputFiles,

    /// Batch conversion completed with some failures.
    #[error("Batch conversion: {successes} succeeded, {failures} failed")]
    BatchFailed {
        /// Number of files that converted successfully.
        successes: usize,
        /// Number of files that failed to convert.
        failures: usize,
    },
}

/// Parses a `WIDTHxHEIGHT` resolution string into a [`Resolution`].
///
/// Both width and height must be non-zero unsigned 32-bit integers.
///
/// # Errors
///
/// Returns `Err` with a human-readable message if the input is malformed,
/// contains non-numeric components, or has a zero dimension.
pub fn parse_resolution(s: &str) -> Result<Resolution, String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid resolution format '{s}'. Expected WIDTHxHEIGHT"
        ));
    }
    let width = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("Invalid width '{}'", parts[0]))?;
    let height = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("Invalid height '{}'", parts[1]))?;
    if width == 0 || height == 0 {
        return Err("Resolution dimensions must be > 0".to_string());
    }
    Ok(Resolution { width, height })
}

/// Resolves the effective output resolution from CLI args and ASS script info.
///
/// If the user specified an explicit `-r` resolution it is parsed and returned.
/// Otherwise the `PlayResX`/`PlayResY` from `[Script Info]` is used, falling back
/// to 1920x1080 when those values are missing, zero, or unreasonably large.
fn resolve_resolution(args: &Args, ass: &AssFile) -> Result<Resolution, String> {
    if let Some(ref res_str) = args.resolution {
        parse_resolution(res_str)
    } else {
        let (w, h) = ass.resolution();
        if w > 0 && h > 0 && w <= 7680 && h <= 4320 {
            Ok(Resolution {
                width: w,
                height: h,
            })
        } else {
            info!(
                "Script Info resolution invalid or missing ({}x{}), falling back to 1920x1080",
                w, h
            );
            Ok(Resolution {
                width: 1920,
                height: 1080,
            })
        }
    }
}

/// Crop a rendered subtitle bitmap to its tight bounding box of non-transparent pixels.
///
/// Returns the cropped RGBA bitmap and its (x, y) offset on the original canvas.
/// PGS/BD-ROM requires the ODS object bitmap to contain only the actual subtitle
/// pixels (with the WDS/PCS position fields placing it on the video frame).
/// Using the full 1920x1080 canvas as the ODS bitmap makes every pixel part of
/// the "subtitle" — PotPlayer then renders the full video area as the subtitle
/// region, producing the vertical-line / white-block artifacts we observed.
///
/// Returns `None` if the bitmap is entirely transparent (skip the frame).
pub fn crop_to_tight_bbox(
    bitmap: &[u8],
    width: u32,
    height: u32,
) -> Option<(Vec<u8>, u32, u32, u32, u32)> {
    if bitmap.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;
    for y in 0..height {
        for x in 0..width {
            let off = ((y * width + x) * 4) as usize;
            if bitmap[off + 3] > 0 {
                any = true;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if !any {
        return None;
    }
    let w = max_x - min_x + 1;
    let h = max_y - min_y + 1;
    let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for y in min_y..=max_y {
        let row_start = ((y * width + min_x) * 4) as usize;
        let row_end = row_start + (w as usize) * 4;
        out.extend_from_slice(&bitmap[row_start..row_end]);
    }
    Some((out, min_x, min_y, w, h))
}

/// Initialises the global `tracing` subscriber with the appropriate level filter.
///
/// Level selection:
/// - `quiet`: `ERROR` only
/// - `verbose`: `DEBUG`
/// - otherwise: `INFO`
pub fn setup_logging(verbose: bool, quiet: bool) {
    let level = if quiet {
        tracing::level_filters::LevelFilter::ERROR
    } else if verbose {
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

/// Resolves the user's color preference string into a boolean.
///
/// `always`/`never` force the decision; any other value (typically `"auto"`)
/// defers to whether stdout is a TTY.
pub fn should_use_color(color: &str) -> bool {
    match color {
        "always" => true,
        "never" => false,
        _ => std::io::IsTerminal::is_terminal(&std::io::stdout()),
    }
}

/// Creates a styled `indicatif` progress bar with the cyan/blue theme used
/// throughout the CLI. `len` is the total unit count; `message` is shown
/// alongside the bar.
pub fn create_progress_bar(len: u64, message: &str) -> ProgressBar {
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

/// Collect input files from positional args and/or --glob pattern.
pub fn collect_input_files(args: &Args) -> Vec<PathBuf> {
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

/// Use glob crate to match files in current directory (non-recursive).
fn collect_flat_glob(pattern: &str) -> Vec<PathBuf> {
    match glob(pattern) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.is_file())
            .collect(),
        Err(e) => {
            warn!("Invalid glob pattern '{}': {}", pattern, e);
            Vec::new()
        }
    }
}

/// Use walkdir to traverse directories, filtering filenames with the glob pattern.
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
            warn!("Invalid glob pattern '{}': {}", file_pattern, e);
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

/// Font map: style name -> ordered list of fallback font names.
type FontMap = HashMap<String, Vec<String>>;

/// Parses "StyleName:fallback1,fallback2" entries into a FontMap.
/// Returns an error with the offending entry if any line is malformed.
fn parse_font_map(entries: &[String]) -> Result<FontMap, String> {
    let mut map = FontMap::new();
    for entry in entries {
        let Some((style, fallbacks)) = entry.split_once(':') else {
            return Err(format!(
                "Invalid font-map entry '{}': expected 'StyleName:fallback1,fallback2'",
                entry
            ));
        };
        let style = style.trim();
        let fb_list: Vec<String> = fallbacks
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if style.is_empty() {
            return Err(format!("Empty style name in font-map entry '{}'", entry));
        }
        map.insert(style.to_string(), fb_list);
    }
    Ok(map)
}

/// Checks all font families used in an ASS file and returns an error listing
/// every font that is missing from the system. The `font_map` provides per-style
/// fallback chains; the `global_fallback` is the --font CLI argument value.
fn check_ass_fonts(
    ass: &AssFile,
    font_manager: &FontManager,
    font_map: &FontMap,
    global_fallback: &str,
    no_check: bool,
) -> Result<(), String> {
    if no_check {
        return Ok(());
    }

    let mut missing: Vec<String> = Vec::new();

    for style in &ass.styles {
        let primary = if style.font_name.is_empty() {
            global_fallback
        } else {
            &style.font_name
        };

        if font_manager
            .query_with_score(primary, false, false)
            .is_some()
        {
            continue;
        }

        // Try per-style fallback chain from --font-map
        if let Some(fallbacks) = font_map.get(&style.name) {
            let all_missing = fallbacks
                .iter()
                .all(|fb| font_manager.query_with_score(fb, false, false).is_none());
            if !all_missing {
                continue;
            }
        }

        // Try global fallback (--font)
        if global_fallback != primary
            && !global_fallback.is_empty()
            && global_fallback != "Arial"
            && font_manager
                .query_with_score(global_fallback, false, false)
                .is_some()
        {
            continue;
        }

        // Build the failure description
        let fb_chain: Vec<&str> = font_map
            .get(&style.name)
            .map(|v| v.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();
        let desc = if fb_chain.is_empty() {
            format!("'{}' (no fallback configured)", primary)
        } else {
            let fb_str = fb_chain.join(", ");
            format!("'{}' (fallbacks: {}) not installed", primary, fb_str)
        };
        missing.push(desc);
    }

    if missing.is_empty() {
        Ok(())
    } else {
        let mut msg = String::from("Font check failed — missing font(s):\n");
        for m in &missing {
            msg.push_str(&format!("  • {}\n", m));
        }
        msg.push_str(
            "Install the fonts above or re-run with --no-check-fonts to skip this check.\n",
        );
        msg.push_str("Hint: for CJK subtitles install fonts-noto-cjk (Debian/Ubuntu) or embed fonts via the ASS [Fonts] section.");
        Err(msg)
    }
}

/// Converts a single subtitle file to the configured output format.
///
/// Handles format detection, validation (when enabled), render, quantize, and
/// encode in a single pass. Returns [`ConversionStats`] describing what was
/// processed, or an error string on failure.
pub fn convert_file(input: &Path, output: &Path, args: &Args) -> Result<ConversionStats, String> {
    info!("Processing: {}", input.display());

    let content =
        std::fs::read_to_string(input).map_err(|e| format!("Failed to read input file: {e}"))?;

    let format = SubtitleFormat::detect(input).unwrap_or(SubtitleFormat::Ass);
    info!("Detected format: {:?}", format);

    let mut ass = match format {
        SubtitleFormat::Srt => ass_parser::srt::parse_srt(&content)
            .map_err(|e| format!("Failed to parse SRT subtitle: {e}"))?,
        _ => AssFile::parse(&content).map_err(|e| format!("Failed to parse subtitle: {e}"))?,
    };

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

    let res = resolve_resolution(args, &ass)?;
    info!("Output resolution: {}x{}", res.width, res.height);

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
    let font_data_list =
        ass.load_embedded_fonts(input.parent().unwrap_or(std::path::Path::new(".")));
    for (_font_name, font_data) in font_data_list {
        let _id = renderer.font_manager_mut().load_font_data(font_data);
    }

    let font_map = parse_font_map(&args.font_map)?;
    check_ass_fonts(
        &ass,
        renderer.font_manager(),
        &font_map,
        &args.font,
        args.no_check_fonts,
    )?;

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
            .map_err(|e| format!("Failed to write output: {e}"))?;
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

    let quantized_frames: Vec<Option<color_quantizer::QuantizedFrame>> = if !use_palette_reuse
        && args.parallel_frames
        && frame_data.len() > 1
    {
        frame_data
            .par_iter()
            .map(|(_event, frame_opt, _pts, _dur)| {
                frame_opt.as_ref().and_then(|frame| {
                    let (bmp, _x, _y, w, h) =
                        crop_to_tight_bbox(&frame.bitmap, frame.width, frame.height)?;
                    Some(quantizer.quantize(&bmp, w, h))
                })
            })
            .collect()
    } else {
        let mut prev_palette: Option<Vec<Rgba>> = None;
        frame_data
            .iter()
            .map(|(_event, frame_opt, _pts, _dur)| {
                frame_opt.as_ref().and_then(|frame| {
                    let (bmp, _x, _y, w, h) =
                        crop_to_tight_bbox(&frame.bitmap, frame.width, frame.height)?;
                    if use_palette_reuse {
                        let prev = prev_palette.as_deref();
                        let q =
                            quantize_with_palette(&bmp, w, h, prev, args.max_colors, dither_method);
                        prev_palette = Some(q.palette.clone());
                        Some(q)
                    } else {
                        let q = quantizer.quantize(&bmp, w, h);
                        prev_palette = Some(q.palette.clone());
                        Some(q)
                    }
                })
            })
            .collect()
    };

    for ((_event, _frame_opt, pts_ms, duration_ms), q_opt) in
        frame_data.iter().zip(quantized_frames.iter())
    {
        if let Some(q) = q_opt {
            let segments = pgs_encoder.encode_frame(q, *pts_ms, *duration_ms);
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
    std::fs::write(output, &sup_data).map_err(|e| format!("Failed to write output: {e}"))?;

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

/// Convert ASS to BDN XML + per-frame PNGs in an output directory.
///
/// Mirrors the structure of `convert_file` but produces:
/// - `{stem}/BDN.xml` (BDN XML manifest referencing PNGs)
/// - `{stem}/0001.png`, `{stem}/0002.png`, ... (one indexed PNG per dialogue event)
pub fn convert_to_bdn(
    input: &Path,
    output_dir: &Path,
    args: &Args,
) -> Result<ConversionStats, String> {
    info!("Processing for BDN: {}", input.display());

    let content =
        std::fs::read_to_string(input).map_err(|e| format!("Failed to read input file: {e}"))?;

    let format = SubtitleFormat::detect(input).unwrap_or(SubtitleFormat::Ass);
    info!("Detected format: {:?}", format);

    let mut ass = match format {
        SubtitleFormat::Srt => ass_parser::srt::parse_srt(&content)
            .map_err(|e| format!("Failed to parse SRT subtitle: {e}"))?,
        _ => AssFile::parse(&content).map_err(|e| format!("Failed to parse subtitle: {e}"))?,
    };

    info!(
        "Parsed: {} styles, {} events",
        ass.styles.len(),
        ass.events.len()
    );

    std::fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output dir: {e}"))?;

    let res = resolve_resolution(args, &ass)?;
    info!("Output resolution: {}x{}", res.width, res.height);

    let render_config = RenderConfig {
        width: res.width,
        height: res.height,
        script_width: ass.script_info.play_res_x,
        script_height: ass.script_info.play_res_y,
        default_font: args.font.clone(),
        default_font_size: args.font_size as f32,
    };
    let mut renderer = Renderer::new(render_config);

    let font_data_list =
        ass.load_embedded_fonts(input.parent().unwrap_or(std::path::Path::new(".")));
    for (_font_name, font_data) in font_data_list {
        let _id = renderer.font_manager_mut().load_font_data(font_data);
    }

    let font_map = parse_font_map(&args.font_map)?;
    check_ass_fonts(
        &ass,
        renderer.font_manager(),
        &font_map,
        &args.font,
        args.no_check_fonts,
    )?;

    let dither_method = match args.dither.as_str() {
        "none" => DitherMethod::None,
        "ordered" => DitherMethod::Ordered,
        _ => DitherMethod::FloydSteinberg,
    };

    let quantizer = Quantizer::new(args.max_colors).with_dither(dither_method);

    let dialogues: Vec<_> = ass.dialogue_events().cloned().collect();
    let total = dialogues.len() as u64;

    if total == 0 {
        warn!("No dialogue events found; emitting empty BDN XML");
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("subtitle");
    let mut bdn = BdnXml::new(stem, res.width, res.height);
    bdn.frame_rate = format!("{}", args.fps);

    let mut events_processed = 0u64;
    let mut frames_encoded = 0u64;
    let mut total_png_bytes: usize = 0;

    for (i, event) in dialogues.iter().enumerate() {
        let pts_ms = event.start.as_ms();
        let duration_ms = event.duration_ms();
        let out_ms = pts_ms + duration_ms;

        let frame_opt = renderer.render_ass(&ass, pts_ms);
        if let Some(frame) = frame_opt {
            let quantized = quantizer.quantize(&frame.bitmap, frame.width, frame.height);

            // Convert quantizer Vec<Rgba> to Vec<[u8; 4]> for bdn-xml
            let palette: Vec<[u8; 4]> = quantized
                .palette
                .iter()
                .map(|c| [c.r, c.g, c.b, c.a])
                .collect();

            let png_filename = format!("{:04}.png", i + 1);
            let png_path = output_dir.join(&png_filename);
            bdn_xml::save_frame_png(
                &png_path,
                &palette,
                &quantized.indices,
                quantized.width,
                quantized.height,
            )
            .map_err(|e| format!("Failed to save PNG {}: {e}", png_filename))?;

            total_png_bytes += quantized.indices.len() + palette.len() * 4;

            let in_tc = bdn_xml::ms_to_timecode(pts_ms, args.fps);
            let out_tc = bdn_xml::ms_to_timecode(out_ms, args.fps);

            bdn.add_event(BdnEvent {
                index: (i + 1) as u32,
                in_tc,
                out_tc,
                graphic: png_filename,
                x: 0,
                y: 0,
                width: quantized.width,
                height: quantized.height,
                forced: false,
            });

            frames_encoded += 1;
        }
        events_processed += 1;
    }

    let xml =
        bdn_xml::generate_xml(&bdn).map_err(|e| format!("Failed to generate BDN XML: {e}"))?;
    let xml_path = output_dir.join("BDN.xml");
    std::fs::write(&xml_path, &xml).map_err(|e| format!("Failed to write BDN XML: {e}"))?;

    info!(
        "BDN output: {} ({} events, {} PNGs, ~{} bytes total)",
        output_dir.display(),
        events_processed,
        frames_encoded,
        total_png_bytes + xml.len()
    );

    Ok(ConversionStats {
        events_processed,
        frames_encoded,
        output_size: total_png_bytes + xml.len(),
    })
}

/// Run the CLI conversion with parsed arguments.
///
/// This function performs the full workflow:
/// 1. Sets up logging
/// 2. Collects input files (positional args + --glob)
/// 3. Parses the display resolution
/// 4. Runs --check mode if requested (parse + validate only)
/// 5. Converts a single file or batch of files
pub fn run(args: Args) -> Result<(), CliError> {
    setup_logging(args.verbose, args.quiet);

    let use_color = should_use_color(&args.color);

    let inputs = collect_input_files(&args);

    if inputs.is_empty() {
        error!("No input files found. Provide positional args or use --glob.");
        return Err(CliError::NoInputFiles);
    }

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

    // --check mode: parse + validate only
    if args.check {
        for input in &inputs {
            AssFile::parse_file(input)
                .map_err(|e| CliError::ParseError(input.display().to_string(), e.to_string()))?;
        }
        return Ok(());
    }

    // --to-srt mode: convert ASS/SSA/SRT to SRT format
    //
    // Also serves as a parse+reserialize self-check: `ass2sup in.srt --to-srt -o out.srt`
    // followed by `diff in.srt out.srt` validates that the SRT parser/serializer
    // roundtrips correctly.
    if args.to_srt {
        for input in &inputs {
            let ass = AssFile::parse_file(input)
                .map_err(|e| CliError::ParseError(input.display().to_string(), e.to_string()))?;
            let srt_content = ass.to_srt();

            let output = if let Some(ref out) = args.output {
                out.clone()
            } else {
                let mut out = input.clone();
                out.set_extension("srt");
                out
            };

            std::fs::write(&output, &srt_content)
                .map_err(|e| CliError::Conversion(format!("Failed to write SRT: {e}")))?;

            info!("{} → {}", input.display(), output.display());
        }
        return Ok(());
    }

    // --to-bdn mode: convert ASS to BDN XML + per-frame PNGs
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
            convert_to_bdn(input, &output_dir, &args).map_err(CliError::Conversion)?;
            info!("{} → {}/", input.display(), output_dir.display());
        }
        return Ok(());
    }

    info!(
        "ass2sup v{} - ASS/SRT to SUP/PGS converter",
        env!("CARGO_PKG_VERSION")
    );
    info!("FPS: {}", args.fps);

    if inputs.len() == 1 {
        let input = &inputs[0];
        let output = args.output.clone().unwrap_or_else(|| {
            let mut out = input.clone();
            out.set_extension("sup");
            out
        });

        match convert_file(input, &output, &args) {
            Ok(stats) => {
                info!(
                    "{} Converted {} events ({} frames) → {} ({} bytes)",
                    if use_color { "✅" } else { "[OK]" },
                    stats.events_processed,
                    stats.frames_encoded,
                    output.display(),
                    stats.output_size
                );
            }
            Err(e) => {
                error!(
                    "{}Conversion failed: {}",
                    if use_color { "❌ " } else { "[FAIL] " },
                    e
                );
                return Err(CliError::Conversion(e));
            }
        }
        return Ok(());
    }

    if args.dry_run && inputs.len() > 1 {
        info!("Dry run: {} file(s) found", inputs.len());
        for (i, input) in inputs.iter().enumerate() {
            info!("  {}. {}", i + 1, input.display());
        }
    }

    let output_dir = args
        .output_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    if !output_dir.exists() {
        std::fs::create_dir_all(&output_dir).map_err(|e| {
            CliError::CreateDirError(output_dir.display().to_string(), e.to_string())
        })?;
    }

    let results: Vec<(usize, Result<ConversionStats, String>)> = if args.parallel {
        inputs
            .par_iter()
            .enumerate()
            .map(|(i, input)| {
                let mut output = output_dir.clone();
                output.push(input.file_stem().unwrap_or_default());
                output.set_extension("sup");
                (i, convert_file(input, &output, &args))
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
                let filename = input
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                pb.set_message(filename.clone());
                let result = convert_file(input, &output, &args);
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
                    "{} [{}] {} events ({} frames) → {} bytes",
                    if use_color { "✅" } else { "[OK]" },
                    inputs[i].display(),
                    stats.events_processed,
                    stats.frames_encoded,
                    stats.output_size
                );
                successes += 1;
            }
            Err(e) => {
                error!(
                    "{} [{}] {}",
                    if use_color { "❌" } else { "[FAIL]" },
                    inputs[i].display(),
                    e
                );
                failures += 1;
            }
        }
    }

    info!(
        "Batch complete: {} succeeded, {} failed",
        successes, failures
    );

    if failures > 0 {
        return Err(CliError::BatchFailed {
            successes,
            failures,
        });
    }

    Ok(())
}
