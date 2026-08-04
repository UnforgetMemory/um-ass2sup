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
    #[arg(short, long, default_value = "23.976", value_parser = parse_fps)]
    pub fps: f64,

    // ── VALIDATION ──
    /// Run validation before conversion
    #[arg(long)]
    pub validate: bool,

    /// Overlap detection mode (off/strict/lenient); enables detection
    #[arg(long, default_value = "off", value_parser = ["off", "strict", "lenient"])]
    pub overlap: String,

    // ── QUANTISATION ──
    /// Maximum colors in palette (1–255)
    #[arg(long, default_value = "255")]
    pub max_colors: usize,

    /// Dithering method (none/floyd-steinberg/ordered)
    #[arg(long, default_value = "floyd-steinberg", value_parser = ["none", "floyd-steinberg", "ordered"])]
    pub dither: String,

    // ── FONT ──
    /// Default font name for SRT input
    #[arg(long, default_value = "Arial")]
    pub font: String,

    /// Default font size for SRT input (native backend only)
    #[arg(long, default_value = "48.0")]
    pub font_size: f64,

    /// Per-style font fallback map (native backend only). Each entry is
    /// "StyleName:fallback1,fallback2". Can be repeated multiple times.
    #[arg(long, value_name = "STYLE:FALLBACKS")]
    pub font_map: Vec<String>,

    /// Additional directories to scan for font files (TTF/OTF/WOFF2).
    #[arg(long, value_name = "DIR")]
    pub font_dir: Vec<PathBuf>,

    /// Skip font availability check (native backend only).
    #[arg(long)]
    pub no_check_fonts: bool,

    // ── PARALLEL ──
    /// Process files in parallel (batch mode)
    #[arg(short, long)]
    pub parallel: bool,

    // ── COLOUR ──
    /// Output colour space (srgb/bt709/bt2020, native backend only).
    #[arg(long, default_value = "srgb", value_parser = ["srgb", "bt709", "bt2020"])]
    pub color_space: String,

    /// HDR-to-SDR tone mapping operator (hable/reinhard/aces, native backend
    /// only).
    #[arg(long, value_parser = ["hable", "reinhard", "aces"])]
    pub tonemap: Option<String>,

    /// Enable VSFilter compatibility mode (native backend only, experimental).
    ///
    /// Compensates for font advance-width differences between swash and
    /// GDI/VSFilter by scaling font_size by ~0.764×, matching easyavs2bdnxml
    /// output dimensions more closely. This is an approximation — exact
    /// results depend on the specific font.
    #[arg(long)]
    pub compat_vsfilter: bool,

    // -- BACKEND SELECTION --
    /// Render backend to use (native, libass).  Only available when both
    /// backends are compiled in (--features native-backend,libass-backend).
    #[arg(long, default_value = "native", value_parser = parse_backend)]
    pub backend: Option<String>,

    // ── FORMAT SELECTION ──
    /// Output format (sup/srt/bdn).
    #[arg(long, default_value = "sup", value_parser = ["sup", "srt", "bdn"])]
    pub format: String,

    // ── MODE ──
    /// Parse and validate only, don't convert (exit 0 if OK, 1 if errors).
    #[arg(long)]
    pub check: bool,

    /// Force conversion even if validation fails.
    #[arg(long)]
    pub force: bool,

    // ── LOGGING ──
    /// Log level (error/warn/info/debug/trace).
    #[arg(long, default_value = "info", value_parser = ["error", "warn", "info", "debug", "trace"])]
    pub log_level: String,

    /// Suppress progress bar.
    #[arg(long)]
    pub quiet: bool,

    /// Colour output mode (auto/always/never).
    #[arg(long, default_value = "auto", value_parser = ["auto", "always", "never"])]
    pub color: String,

    /// Write diagnostic logs (with timestamps) to this file in addition to stderr.
    #[arg(long, value_name = "PATH")]
    pub log_file: Option<String>,
}

/// Validate a `--fps` value: must be a finite number greater than zero.
///
/// clap's default `f64` parser accepts `inf` and `nan`; with those values the
/// frame-timeline loops in the conversion pipeline either spin forever
/// (ms_per_frame → 0) or never terminate on NaN.  Rejecting them at parse time
/// turns a hang into an immediate, actionable CLI error.
fn parse_fps(s: &str) -> Result<f64, String> {
    let value: f64 = s
        .parse()
        .map_err(|_| format!("invalid fps value '{s}': expected a number"))?;
    if !value.is_finite() || value <= 0.0 {
        return Err(format!(
            "invalid fps value '{s}': must be a finite number greater than 0"
        ));
    }
    Ok(value)
}

/// Validate a `--backend` value.
///
/// When both rendering backends are compiled in, only `native` and `libass`
/// are valid; anything else is rejected at parse time (clap-style error).
///
/// In single-backend builds the flag is a no-op — the compiled backend is
/// always used — so any value is accepted to preserve the historical
/// behaviour.
fn parse_backend(s: &str) -> Result<String, String> {
    #[cfg(all(feature = "native-backend", feature = "libass-backend"))]
    {
        match s {
            "native" | "libass" => Ok(s.to_string()),
            other => Err(format!(
                "invalid backend '{other}': expected 'native' or 'libass'"
            )),
        }
    }
    #[cfg(not(all(feature = "native-backend", feature = "libass-backend")))]
    {
        Ok(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fps_rejects_nan() {
        assert!(parse_fps("nan").is_err());
        assert!(parse_fps("NaN").is_err());
    }

    #[test]
    fn parse_fps_rejects_infinity() {
        assert!(parse_fps("inf").is_err());
        assert!(parse_fps("+inf").is_err());
        assert!(parse_fps("infinity").is_err());
    }

    #[test]
    fn parse_fps_rejects_non_positive() {
        assert!(parse_fps("0").is_err());
        assert!(parse_fps("0.0").is_err());
        assert!(parse_fps("-1").is_err());
    }

    #[test]
    fn parse_fps_accepts_valid_values() {
        assert_eq!(parse_fps("23.976").unwrap(), 23.976);
        assert_eq!(parse_fps("25").unwrap(), 25.0);
    }
}
