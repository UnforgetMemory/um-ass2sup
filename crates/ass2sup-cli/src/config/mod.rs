//! Domain-level configuration for the conversion pipeline.
//!
//! [`Config`] is the central value object built from CLI arguments and
//! optional ASS script metadata.  Sub-modules handle resolution parsing,
//! colour-space selection, and font-map configuration.

pub mod color_space;
/// Font configuration — per-style fallback maps and availability checks.
pub mod font;
pub mod resolution;

pub use color_space::*;
pub use font::*;
pub use resolution::Resolution;

/// Complete pipeline configuration assembled from CLI args.
#[derive(Debug, Clone)]
pub struct Config {
    /// Display resolution (width × height).
    pub resolution: Resolution,
    /// Frames per second.
    pub fps: f64,
    /// Maximum palette colours (1–255).
    pub max_colors: usize,
    /// Dithering method.
    pub dither: color_quantizer::DitherMethod,
    /// Output colour space for PGS YCbCr conversion.
    pub color_space: color_quantizer::color::ColorSpace,
    /// Optional HDR-to-SDR tone-mapping operator.
    pub tonemap: Option<color_quantizer::color::tonemap::ToneMapOperator>,
    /// Font-related settings.
    pub font: FontConfig,
    /// Output path / format settings.
    pub output: OutputConfig,
    /// Parallelism settings.
    pub parallel: ParallelConfig,
}

/// Font-related settings.
#[derive(Debug, Clone)]
pub struct FontConfig {
    /// Default font family name.
    pub default_font: String,
    /// Default font size in points.
    pub default_font_size: f32,
    /// Per-style fallback map (style → fallback list).
    pub font_map: FontMap,
    /// Extra directories to scan for font files.
    pub font_dirs: Vec<std::path::PathBuf>,
    /// Skip the font availability check.
    pub no_check: bool,
}

/// Output path / format configuration.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Single output file path (SUP mode).
    pub output: Option<std::path::PathBuf>,
    /// Output directory (batch or BDN mode).
    pub output_dir: Option<std::path::PathBuf>,
    /// Dry-run mode: parse only, no output written.
    pub dry_run: bool,
}

/// Parallelism settings.
#[derive(Debug, Clone, Default)]
pub struct ParallelConfig {
    /// Enable parallel frame rendering (single-file mode).
    pub frames: bool,
    /// Enable parallel file processing (batch mode).
    pub files: bool,
}

impl Config {
    /// Build a [`Config`] from CLI arguments and an optional ASS metadata
    /// resolution fallback.
    pub fn from_args(args: &super::cli::args::Args) -> Self {
        let dither = match args.dither.as_str() {
            "none" => color_quantizer::DitherMethod::None,
            "ordered" => color_quantizer::DitherMethod::Ordered,
            _ => color_quantizer::DitherMethod::FloydSteinberg,
        };

        let color_space = match args.color_space.as_str() {
            "bt709" => color_quantizer::color::ColorSpace::Bt709,
            "bt2020" => color_quantizer::color::ColorSpace::Bt2020,
            _ => color_quantizer::color::ColorSpace::Srgb,
        };

        let tonemap = args.tonemap.as_ref().map(|op| match op.as_str() {
            "hable" => color_quantizer::color::tonemap::ToneMapOperator::Hable,
            "reinhard" => color_quantizer::color::tonemap::ToneMapOperator::Reinhard,
            "aces" => color_quantizer::color::tonemap::ToneMapOperator::Aces,
            _ => color_quantizer::color::tonemap::ToneMapOperator::Reinhard,
        });

        Self {
            resolution: Resolution::default(),
            fps: args.fps,
            max_colors: args.max_colors,
            dither,
            color_space,
            tonemap,
            font: FontConfig {
                default_font: args.font.clone(),
                default_font_size: args.font_size as f32,
                font_map: parse_font_map(&args.font_map).unwrap_or_default(),
                font_dirs: args.font_dir.clone(),
                no_check: args.no_check_fonts,
            },
            output: OutputConfig {
                output: args.output.clone(),
                output_dir: args.output_dir.clone(),
                dry_run: args.dry_run,
            },
            parallel: ParallelConfig {
                frames: false,
                files: args.parallel,
            },
        }
    }

    /// Resolve the effective display resolution, using `script_resolution` as
    /// fallback when no explicit `-r` flag was provided.
    pub fn resolve_resolution(&mut self, script_width: u32, script_height: u32) {
        self.resolution =
            Resolution::from_args_or_script(&self.resolution, script_width, script_height);
    }
}
