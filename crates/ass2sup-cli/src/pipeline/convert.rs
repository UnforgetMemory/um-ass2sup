//! Core conversion pipeline — the heart of ass2sup.
//!
//! [`ConversionPipeline`] is a stateless service that processes a parsed
//! [`SubtitleDocument`] through render → crop → quantise → encode → write.

use std::path::Path;

use ass_core::{SubtitleDocument, SubtitleFormat};
use color_quantizer::pipeline::ColorPipeline;
use color_quantizer::QuantizedFrame;
use pgs_encoder::PgsEncoder;
use subtitle_renderer::{RenderConfig, Renderer};
use tracing::{debug, error, info, trace, warn};

use crate::cli::args::Args;
use crate::cli::progress;
use crate::config::Config;
use crate::error::CliError;
use crate::util;

/// Per-file conversion statistics.
#[derive(Debug, Clone, Default)]
pub struct ConversionStats {
    /// Number of dialogue events processed from the input.
    pub events_processed: u64,
    /// Number of PGS / BDN frames successfully encoded.
    pub frames_encoded: u64,
    /// Size of the output in bytes.
    pub output_size: usize,
}

/// Stateless conversion pipeline.
pub struct ConversionPipeline;

impl ConversionPipeline {
    /// Parse a subtitle file from disk, auto-detecting the format.
    pub fn parse_input(input: &Path) -> Result<SubtitleDocument, CliError> {
        let content = std::fs::read_to_string(input)
            .map_err(|e| CliError::ReadError(input.display().to_string(), e.to_string()))?;

        let format = input
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| match e.to_lowercase().as_str() {
                "srt" => SubtitleFormat::Srt,
                "ssa" => SubtitleFormat::Ssa,
                _ => SubtitleFormat::Ass,
            })
            .unwrap_or(SubtitleFormat::Ass);
        info!("Detected format: {format:?}");

        let doc = match format {
            SubtitleFormat::Srt => ass_core::srt::parse_srt(&content)
                .map_err(|e| CliError::ParseError(input.display().to_string(), e.to_string()))?,
            _ => SubtitleDocument::parse(&content)
                .map_err(|e| CliError::ParseError(input.display().to_string(), e.to_string()))?,
        };
        Ok(doc)
    }

    /// Validate a subtitle document if the CLI flags are set.
    pub fn validate(doc: &SubtitleDocument, args: &Args) -> Result<(), CliError> {
        if !args.validate && !args.overlap_warn {
            return Ok(());
        }

        let overlap_config = match args.overlap_mode.as_str() {
            "strict" => subtitle_validator::OverlapConfig::strict(),
            _ => subtitle_validator::OverlapConfig::lenient(),
        };

        let validator = if args.overlap_warn {
            subtitle_validator::Validator::new().with_overlap_config(overlap_config)
        } else {
            subtitle_validator::Validator::new()
        };

        let report = validator.validate(doc);

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
                        subtitle_validator::OverlapSeverity::Critical => "CRITICAL",
                        subtitle_validator::OverlapSeverity::High => "HIGH",
                        subtitle_validator::OverlapSeverity::Medium => "MEDIUM",
                        subtitle_validator::OverlapSeverity::Low => "LOW",
                    },
                );
            }
        }

        info!("{}", report.summary());

        if !report.is_valid && !args.force {
            return Err(CliError::Conversion(
                "Validation failed. Use --force to override.".into(),
            ));
        }
        Ok(())
    }

    /// Build a [`Renderer`] and load fonts for the given document and config.
    pub fn create_renderer(
        doc: &SubtitleDocument,
        config: &Config,
        input: &Path,
        _args: &Args,
    ) -> Result<Renderer, CliError> {
        debug!(
            width = config.resolution.width,
            height = config.resolution.height,
            "creating Renderer"
        );

        let render_cfg = RenderConfig {
            width: config.resolution.width,
            height: config.resolution.height,
            script_width: doc.metadata.play_res_x,
            script_height: doc.metadata.play_res_y,
            default_font: config.font.default_font.clone(),
            default_font_size: config.font.default_font_size,
        };

        let renderer = Renderer::new(render_cfg);

        // Load system fonts
        let system_count = renderer.load_system_fonts();
        debug!(system_count, "loaded system fonts");

        // Load extra font directories
        for dir in &config.font.font_dirs {
            let added = renderer.load_user_fonts_dir(dir);
            if added > 0 {
                info!("Loaded {added} font face(s) from {}", dir.display());
            } else {
                warn!("No font files found in --font-dir: {}", dir.display());
            }
        }

        // Load embedded fonts from ASS [Fonts] section
        let font_data_list: Vec<(String, Vec<u8>)> = doc
            .fonts
            .iter()
            .filter_map(|ef| {
                if ef.filename.is_empty() {
                    return None;
                }
                let path = input.parent().unwrap_or(Path::new(".")).join(&ef.filename);
                std::fs::read(&path)
                    .ok()
                    .map(|data| (ef.font_name.clone(), data))
            })
            .collect();
        let embedded_count = font_data_list.len();
        for (_font_name, font_data) in font_data_list {
            let _ = renderer.load_user_font_data(font_data);
        }
        debug!(embedded_count, "loaded ASS embedded fonts");

        Ok(renderer)
    }

    /// Shared render + quantise step, used by both SUP and BDN paths.
    pub fn render_and_quantize(
        doc: &SubtitleDocument,
        renderer: &mut Renderer,
        config: &Config,
        args: &Args,
    ) -> Vec<QuantizedFrame> {
        use rayon::prelude::*;

        let dialogues: Vec<_> = doc.events.iter().collect();
        let total = dialogues.len() as u64;

        let pb = if args.quiet {
            indicatif::ProgressBar::hidden()
        } else {
            progress::create(total, "Rendering")
        };

        let dither = super::super::config::color_space::parse_dither(&args.dither);
        let mut pipeline = ColorPipeline::new()
            .with_max_colors(config.max_colors)
            .with_dither(dither);

        // Auto-select BT.709 for HD content (1080p) per Blu-ray spec.
        let effective_cs = if config.color_space == color_quantizer::color::ColorSpace::Srgb
            && config.resolution.height > 576
        {
            color_quantizer::color::ColorSpace::Bt709
        } else {
            config.color_space
        };
        if effective_cs != color_quantizer::color::ColorSpace::Srgb {
            pipeline = pipeline.with_color_space(effective_cs);
        }
        if let Some(op) = &config.tonemap {
            pipeline = pipeline.with_tonemap(*op);
        }

        let use_palette_reuse = args.quantizer == "median-cut";

        let quantized: Vec<QuantizedFrame> = if use_palette_reuse {
            // Sequential path with palette reuse
            let mut prev_frame: Option<QuantizedFrame> = None;
            doc.events
                .iter()
                .enumerate()
                .filter_map(|(i, event)| {
                    let render_pts = util::compute_render_pts(event);
                    let frame = match renderer.render_ass(doc, render_pts) {
                        Some(f) => f,
                        None => {
                            warn!(
                                "Event {}: render_ass returned no frame (start={}ms, end={}ms, text=\"{}\")",
                                i,
                                event.start_ms,
                                event.end_ms,
                                event.text_raw.chars().take(60).collect::<String>(),
                            );
                            return None;
                        }
                    };
                    let (bmp, x, y, w, h) = match util::crop_to_tight_bbox(
                        &frame.bitmap,
                        frame.width,
                        frame.height,
                    ) {
                        Some(c) => c,
                        None => {
                            warn!(
                                "Event {}: crop_to_tight_bbox found no visible pixels (frame {}x{})",
                                i, frame.width, frame.height,
                            );
                            return None;
                        }
                    };
                    let mut q = pipeline.quantize_with_prev(&bmp, w, h, prev_frame.as_ref());
                    q.x = x as u16;
                    q.y = y as u16;
                    q.pts_ms = event.start_ms;
                    q.duration_ms = event.end_ms.saturating_sub(event.start_ms);
                    prev_frame = Some(q.clone());
                    pb.inc(1);
                    Some(q)
                })
                .collect()
        } else {
            // Parallel path (no palette reuse)
            let frames: Vec<Option<QuantizedFrame>> = doc
                .events
                .par_iter()
                .enumerate()
                .map(|(i, event)| {
                    let render_pts = util::compute_render_pts(event);
                    let frame = match renderer.render_ass(doc, render_pts) {
                        Some(f) => f,
                        None => {
                            warn!(
                                "Event {}: render_ass returned no frame (start={}ms, end={}ms, text=\"{}\")",
                                i,
                                event.start_ms,
                                event.end_ms,
                                event.text_raw.chars().take(60).collect::<String>(),
                            );
                            return None;
                        }
                    };
                    let (bmp, x, y, w, h) = match util::crop_to_tight_bbox(
                        &frame.bitmap,
                        frame.width,
                        frame.height,
                    ) {
                        Some(c) => c,
                        None => {
                            warn!(
                                "Event {}: crop_to_tight_bbox found no visible pixels (frame {}x{})",
                                i, frame.width, frame.height,
                            );
                            return None;
                        }
                    };
                    let mut q = pipeline.quantize(&bmp, w, h);
                    q.x = x as u16;
                    q.y = y as u16;
                    q.pts_ms = event.start_ms;
                    q.duration_ms = event.end_ms.saturating_sub(event.start_ms);
                    Some(q)
                })
                .collect();
            pb.finish_and_clear();
            frames.into_iter().flatten().collect()
        };

        pb.finish_and_clear();
        quantized
    }

    /// Encode quantised frames into SUP binary data.
    pub fn encode_sup(
        frames: &[QuantizedFrame],
        config: &Config,
    ) -> Vec<pgs_encoder::types::Segment> {
        let mut encoder = PgsEncoder::new(
            config.resolution.width as u16,
            config.resolution.height as u16,
            config.fps,
        );

        let mut all_segments = Vec::new();
        for (i, q) in frames.iter().enumerate() {
            let segments = encoder.encode_frame(q, q.pts_ms, q.duration_ms);
            all_segments.extend(segments);
            if i % 100 == 0 {
                trace!("encode_sup progress: {}/{} frames", i, frames.len());
            }
        }
        all_segments
    }

    /// Write a SUP file from PGS segments.
    pub fn write_sup(
        segments: Vec<pgs_encoder::types::Segment>,
        output: &Path,
    ) -> Result<usize, CliError> {
        let sup_file = pgs_encoder::types::SupFile { segments };
        let bytes = sup_file.to_bytes();
        let len = bytes.len();
        std::fs::write(output, &bytes)
            .map_err(|e| CliError::Conversion(format!("Failed to write SUP: {e}")))?;
        Ok(len)
    }

    /// Write BDN XML + per-frame PNG files.
    pub fn write_bdn(
        frames: &[QuantizedFrame],
        doc: &SubtitleDocument,
        config: &Config,
        input: &Path,
        output_dir: &Path,
    ) -> Result<ConversionStats, CliError> {
        std::fs::create_dir_all(output_dir)
            .map_err(|e| CliError::Conversion(format!("Failed to create dir: {e}")))?;

        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("subtitle");
        let mut bdn = bdn_xml::BdnXml::new(stem, config.resolution.width, config.resolution.height);
        bdn.frame_rate = format!("{}", config.fps);

        let mut total_png_bytes: usize = 0;
        let mut frames_encoded = 0u64;

        for (i, event) in doc.events.iter().enumerate() {
            if i >= frames.len() {
                break;
            }
            let q = &frames[i];
            let palette: Vec<[u8; 4]> = q.palette.iter().map(|c| [c.r, c.g, c.b, c.a]).collect();

            let png_filename = format!("{:04}.png", i + 1);
            let png_path = output_dir.join(&png_filename);
            bdn_xml::save_frame_png(&png_path, &palette, &q.indices, q.width, q.height)
                .map_err(|e| CliError::Conversion(format!("PNG save failed: {e}")))?;

            total_png_bytes += q.indices.len() + palette.len() * 4;

            let display_pts = event.start_ms;
            let duration_ms = event.end_ms.saturating_sub(event.start_ms);
            let in_tc = bdn_xml::ms_to_timecode(display_pts, config.fps);
            let out_tc = bdn_xml::ms_to_timecode(display_pts + duration_ms, config.fps);

            bdn.add_event(bdn_xml::BdnEvent {
                index: (i + 1) as u32,
                in_tc,
                out_tc,
                graphic: png_filename,
                x: q.x as u32,
                y: q.y as u32,
                width: q.width,
                height: q.height,
                forced: false,
            });
            frames_encoded += 1;
        }

        let xml = bdn_xml::generate_xml(&bdn)
            .map_err(|e| CliError::Conversion(format!("XML generation failed: {e}")))?;
        let xml_path = output_dir.join("BDN.xml");
        std::fs::write(&xml_path, &xml)
            .map_err(|e| CliError::Conversion(format!("XML write failed: {e}")))?;

        Ok(ConversionStats {
            events_processed: doc.events.len() as u64,
            frames_encoded,
            output_size: total_png_bytes + xml.len(),
        })
    }
}

/// Convenience wrapper: parse, render, quantize, and write SUP in one call.
pub fn convert_file(
    input: &Path,
    output: &Path,
    args: &Args,
    config: &Config,
) -> Result<ConversionStats, CliError> {
    info!("Processing: {}", input.display());
    trace!(input = %input.display(), "convert_file entry");

    let doc = ConversionPipeline::parse_input(input)?;
    ConversionPipeline::validate(&doc, args)?;

    if config.output.dry_run {
        info!("Dry run complete — skipping render/encode");
        return Ok(ConversionStats {
            events_processed: doc.events.len() as u64,
            ..Default::default()
        });
    }

    let mut renderer = ConversionPipeline::create_renderer(&doc, config, input, args)?;
    let frames = ConversionPipeline::render_and_quantize(&doc, &mut renderer, config, args);
    let segments = ConversionPipeline::encode_sup(&frames, config);
    let output_size = ConversionPipeline::write_sup(segments, output)?;

    info!(
        "Output: {} ({} bytes, {} frames)",
        output.display(),
        output_size,
        frames.len()
    );

    Ok(ConversionStats {
        events_processed: doc.events.len() as u64,
        frames_encoded: frames.len() as u64,
        output_size,
    })
}

/// Convert to BDN XML + per-frame PNGs.
pub fn convert_to_bdn(
    input: &Path,
    output_dir: &Path,
    args: &Args,
    config: &Config,
) -> Result<ConversionStats, CliError> {
    info!("Processing for BDN: {}", input.display());

    let doc = ConversionPipeline::parse_input(input)?;
    let mut renderer = ConversionPipeline::create_renderer(&doc, config, input, args)?;
    let frames = ConversionPipeline::render_and_quantize(&doc, &mut renderer, config, args);
    let stats = ConversionPipeline::write_bdn(&frames, &doc, config, input, output_dir)?;

    info!(
        "BDN output: {} ({} events, {} PNGs)",
        output_dir.display(),
        stats.events_processed,
        stats.frames_encoded,
    );

    Ok(stats)
}
