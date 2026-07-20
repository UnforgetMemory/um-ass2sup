//! Libass rendering backend (C library via FFI).
//!
//! Wraps the [`subtitle_renderer_libass`] crate to render ASS content
//! through libass, then crops, quantises, and returns [`QuantizedFrame`]s.

use std::collections::HashMap;

use ass_core::SubtitleDocument;
use color_quantizer::QuantizedFrame;
use tracing::debug;

use crate::cli::args::Args;
use crate::cli::progress;
use crate::config::Config;
use crate::error::CliError;

/// Render and quantize using the libass C library.
pub fn render_and_quantize(
    content: &str,
    _doc: &SubtitleDocument,
    config: &Config,
    args: &Args,
) -> Result<Vec<QuantizedFrame>, CliError> {
    let libass_config = build_libass_config(config);
    let frames = process_libass(content, libass_config, args)
        .map_err(|e| CliError::Conversion(format!("libass rendering failed: {e}")))?;
    Ok(frames)
}

/// Bridge config from the unified CLI format to libass-native format.
fn build_libass_config(config: &Config) -> subtitle_renderer_libass::ConversionConfig {
    use color_quantizer::DitherMethod;

    let dither = match config.dither {
        DitherMethod::None => "none".to_string(),
        DitherMethod::FloydSteinberg => "floyd-steinberg".to_string(),
        DitherMethod::Ordered => "ordered".to_string(),
    };

    subtitle_renderer_libass::ConversionConfig {
        fps: config.fps,
        width: config.resolution.width,
        height: config.resolution.height,
        max_colors: config.max_colors,
        dither,
        default_font: Some(config.font.default_font.clone()),
        fonts_dirs: config
            .font
            .font_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        font_fallback_map: HashMap::new(),
        check_fonts: false,
    }
}

/// Run the libass rendering pipeline.
///
/// This re-exports and calls the libass-core equivalent of the original
/// `Ass2Sup::process_events()` pipeline.
fn process_libass(
    content: &str,
    config: subtitle_renderer_libass::ConversionConfig,
    args: &Args,
) -> Result<Vec<QuantizedFrame>, subtitle_renderer_libass::AssError> {
    use subtitle_renderer_libass::AssRenderer;

    let needed_families = subtitle_renderer_libass::extract_font_families(content);
    tracing::info!(
        "Font families needed: {}",
        if needed_families.is_empty() {
            "all".to_string()
        } else {
            needed_families
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    let mut renderer = AssRenderer::new(config.width, config.height)?;
    renderer.load_ass(content)?;
    renderer.configure_fonts(
        config.default_font.as_deref(),
        &config.fonts_dirs,
        &needed_families,
    )?;

    let events = renderer.events();
    if events.is_empty() {
        return Err(subtitle_renderer_libass::AssError::NoEvents);
    }

    let timestamps = subtitle_renderer_libass::generate_timestamps(&events, config.fps);
    if timestamps.is_empty() {
        return Err(subtitle_renderer_libass::AssError::NoEvents);
    }

    let pipeline =
        subtitle_renderer_libass::create_pipeline(config.max_colors, &config.dither, config.height);
    let mut output_frames: Vec<QuantizedFrame> = Vec::new();
    let mut prev_data_hash: Option<u64> = None;

    let total_frames = timestamps.len() as u64;
    tracing::info!("Rendering {total_frames} frames...");

    let pb = if args.quiet {
        indicatif::ProgressBar::hidden()
    } else {
        progress::create(total_frames, "Rendering")
    };

    let last_event_end = events
        .iter()
        .map(|e| e.start_ms + e.duration_ms)
        .max()
        .unwrap_or(0) as u64;

    for window in timestamps.windows(2) {
        let ts = window[0];
        let next_ts = window[1];

        let has_active = events
            .iter()
            .any(|e| e.start_ms as u64 <= ts && ts < (e.start_ms + e.duration_ms) as u64);
        if !has_active {
            pb.inc(1);
            continue;
        }

        let images = match renderer.render_frame(ts as i64)? {
            Some(imgs) if !imgs.is_empty() => imgs,
            _ => {
                pb.inc(1);
                continue;
            }
        };

        let rgba = subtitle_renderer_libass::compose_frame(&images, config.width, config.height);

        let cropped = match subtitle_renderer_libass::crop_to_tight_bbox(
            &rgba.data,
            config.width,
            config.height,
        ) {
            Some(c) => c,
            None => {
                pb.inc(1);
                continue;
            }
        };

        let cropped_frame = subtitle_renderer_libass::CroppedFrame {
            data: cropped.0,
            x: cropped.1,
            y: cropped.2,
            width: cropped.3,
            height: cropped.4,
        };

        let prev_frame = output_frames.last();
        let mut q = pipeline.quantize_with_prev(
            &cropped_frame.data,
            cropped_frame.width,
            cropped_frame.height,
            prev_frame,
        );
        q.x = cropped_frame.x as u16;
        q.y = cropped_frame.y as u16;
        q.pts_ms = ts;
        q.duration_ms = next_ts.saturating_sub(ts).max(1);

        // Duplicate detection
        let hash = subtitle_renderer_libass::hash_quantized(&q);
        if prev_data_hash == Some(hash) {
            if let Some(last) = output_frames.last_mut() {
                last.duration_ms = ts + q.duration_ms - last.pts_ms;
            }
            pb.inc(1);
            continue;
        }

        prev_data_hash = Some(hash);
        output_frames.push(q);
        pb.inc(1);
    }

    pb.finish_and_clear();

    // Fix up last frame duration
    if let Some(last) = output_frames.last_mut() {
        if last.pts_ms + last.duration_ms < last_event_end {
            last.duration_ms = last_event_end.saturating_sub(last.pts_ms);
        }
    }

    debug!(rendered = output_frames.len(), "libass rendering complete");
    Ok(output_frames)
}
