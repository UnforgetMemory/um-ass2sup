//! CLI argument definitions via clap derive.
//!
//! [`Args`] mirrors every flag and positional argument accepted by the
//! `ass2sup` binary.  Helper types such as [`FontMap`] and [`Resolution`]
//! are re-exported from their respective domain modules.

use std::path::PathBuf;

use clap::Parser;

/// ASS/SRT to SUP/PGS converter
#[derive(Parser, Debug)]
#[command(name = "ass2sup", version, about, long_about = None)]
pub struct Args {
    // ── INPUT ──
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

    // ── OUTPUT ──
    /// Output SUP file path (single file mode)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output directory (batch mode)
    #[arg(short = 'd', long)]
    pub output_dir: Option<PathBuf>,

    // ── VIDEO ──
    /// Display resolution (WIDTHxHEIGHT).
    ///
    /// If not specified, uses PlayResX/PlayResY from [Script Info] section.
    /// Falls back to 1920×1080 if Script Info resolution is missing or zero.
    #[arg(short, long)]
    pub resolution: Option<String>,

    /// Frames per second
    #[arg(short, long, default_value = "23.976")]
    pub fps: f64,

    // ── VALIDATION ──
    /// Run validation before conversion
    #[arg(long)]
    pub validate: bool,

    /// Enable overlap warning detection
    #[arg(long)]
    pub overlap_warn: bool,

    /// Overlap detection mode (strict/lenient)
    #[arg(long, default_value = "lenient")]
    pub overlap_mode: String,

    // ── QUANTISATION ──
    /// Quantizer algorithm (median-cut)
    #[arg(long, default_value = "median-cut")]
    pub quantizer: String,

    /// Maximum colors in palette (1–255)
    #[arg(long, default_value = "255")]
    pub max_colors: usize,

    /// Dithering method (none/floyd-steinberg/ordered)
    #[arg(long, default_value = "floyd-steinberg")]
    pub dither: String,

    // ── FONT ──
    /// Default font name for SRT input
    #[arg(long, default_value = "Arial")]
    pub font: String,

    /// Default font size for SRT input
    #[arg(long, default_value = "48.0")]
    pub font_size: f64,

    /// Per-style font fallback map. Each entry is "StyleName:fallback1,fallback2".
    /// Can be repeated multiple times.
    #[arg(long, value_name = "STYLE:FALLBACKS")]
    pub font_map: Vec<String>,

    /// Additional directories to scan for font files (TTF/OTF/WOFF2).
    #[arg(long, value_name = "DIR")]
    pub font_dir: Vec<PathBuf>,

    /// Skip font availability check.
    #[arg(long)]
    pub no_check_fonts: bool,

    // ── PARALLEL ──
    /// Process files in parallel (batch mode)
    #[arg(short, long)]
    pub parallel: bool,

    /// Render frames in parallel using rayon (single-file mode)
    #[arg(long)]
    pub parallel_frames: bool,

    // ── COLOUR ──
    /// Output colour space (srgb/bt709/bt2020).
    #[arg(long, default_value = "srgb")]
    pub color_space: String,

    /// HDR-to-SDR tone mapping operator (hable/reinhard/aces).
    #[arg(long)]
    pub tonemap: Option<String>,

    // ── FORMAT SELECTION ──
    /// Convert to SRT format instead of SUP/PGS.
    #[arg(long)]
    pub to_srt: bool,

    /// Convert to BDN XML + PNG format (Blu-ray authoring).
    #[arg(long, conflicts_with = "to_srt")]
    pub to_bdn: bool,

    // ── MODE ──
    /// Parse and validate only, don't convert (exit 0 if OK, 1 if errors).
    #[arg(long)]
    pub check: bool,

    /// Dry run: parse and validate only, don't write output.
    #[arg(long)]
    pub dry_run: bool,

    /// Force conversion even if validation fails.
    #[arg(long)]
    pub force: bool,

    // ── LOGGING ──
    /// Enable verbose logging.
    #[arg(short, long)]
    pub verbose: bool,

    /// Enable trace-level debug output for pipeline diagnosis.
    #[arg(long)]
    pub debug: bool,

    /// Suppress progress bar.
    #[arg(long)]
    pub quiet: bool,

    /// Colour output mode (auto/always/never).
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,
}
