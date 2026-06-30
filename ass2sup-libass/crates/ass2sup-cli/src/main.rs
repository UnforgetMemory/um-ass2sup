use std::collections::HashMap;
use std::path::PathBuf;

use clap::Parser;

use ass2sup_core::domain::pipeline::{Ass2Sup, ConversionConfig};

/// ASS/SSA/SRT to SUP/PGS subtitle converter (libass-based).
#[derive(Parser, Debug)]
#[command(name = "ass2sup", version, about)]
struct Args {
    /// Input ASS file
    input: PathBuf,

    /// Output SUP file path
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output BDN XML + PNG instead of SUP
    #[arg(long)]
    to_bdn: bool,

    /// Output directory for BDN mode
    #[arg(short = 'd', long)]
    output_dir: Option<PathBuf>,

    /// Display resolution (WIDTHxHEIGHT, defaults to ASS PlayRes or 1920x1080)
    #[arg(short, long)]
    resolution: Option<String>,

    /// Frames per second
    #[arg(short, long, default_value = "23.976")]
    fps: f64,

    /// Maximum palette colours (1–255)
    #[arg(long, default_value = "255")]
    max_colors: usize,

    /// Dither method: none, floyd-steinberg, ordered
    #[arg(long, default_value = "floyd-steinberg")]
    dither: String,

    /// Default font family for fontconfig
    #[arg(long)]
    font: Option<String>,

    /// Additional font directory
    #[arg(long)]
    font_dir: Option<String>,

    /// Per-style font fallback map. Each entry is "StyleName:fallback1,fallback2,...".
    /// Replaces the original style's Fontname before passing to libass.
    #[arg(long, value_name = "STYLE:FALLBACKS")]
    font_fallback_map: Vec<String>,

    /// Check font availability before rendering. Exits with error if any
    /// requested font is missing (no exact match via fc-match).
    #[arg(long)]
    check_fonts: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    let args = Args::parse();

    // Initialize tracing
    let filter = if args.verbose {
        "ass2sup_core=debug"
    } else {
        "ass2sup_core=info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    let (width, height) = parse_resolution(args.resolution.as_deref());

    let config = ConversionConfig {
        fps: args.fps,
        width,
        height,
        max_colors: args.max_colors.clamp(1, 255),
        dither: args.dither,
        default_font: args.font,
        fonts_dir: args.font_dir,
        font_fallback_map: parse_font_fallback_map(&args.font_fallback_map),
        check_fonts: args.check_fonts,
    };

    if args.to_bdn {
        let output_dir = args
            .output_dir
            .unwrap_or_else(|| PathBuf::from("bdn_output"));
        match Ass2Sup::convert_to_bdn(&args.input, &output_dir, &config) {
            Ok(stats) => {
                tracing::info!(
                    events = stats.events_processed,
                    frames = stats.frames_encoded,
                    "BDN output: {}",
                    output_dir.display()
                );
            }
            Err(e) => {
                tracing::error!("Conversion failed: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        let output = args.output.unwrap_or_else(|| {
            let mut p = args.input.clone();
            p.set_extension("sup");
            p
        });
        match Ass2Sup::convert_file(&args.input, &output, &config) {
            Ok(stats) => {
                tracing::info!(
                    events = stats.events_processed,
                    frames = stats.frames_encoded,
                    size = stats.output_size,
                    "[OK] Converted -> {} ({} bytes)",
                    output.display(),
                    stats.output_size
                );
            }
            Err(e) => {
                tracing::error!("Conversion failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Parse "Style:Fallback" entries from --font-fallback-map args.
fn parse_font_fallback_map(items: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for item in items {
        if let Some((style, fallback)) = item.split_once(':') {
            let style = style.trim().to_string();
            let fallback = fallback.trim().to_string();
            if !style.is_empty() && !fallback.is_empty() {
                map.insert(style, fallback);
            }
        }
    }
    map
}

/// Parse a "WIDTHxHEIGHT" resolution string. Falls back to 1920x1080.
fn parse_resolution(s: Option<&str>) -> (u32, u32) {
    const DEFAULT: (u32, u32) = (1920, 1080);
    let s = match s {
        Some(v) => v,
        None => return DEFAULT,
    };
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return DEFAULT;
    }
    let w: u32 = parts[0].parse().unwrap_or(DEFAULT.0);
    let h: u32 = parts[1].parse().unwrap_or(DEFAULT.1);
    (w, h)
}
