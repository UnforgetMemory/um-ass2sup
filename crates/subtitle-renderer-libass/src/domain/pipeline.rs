//! Pipeline orchestration: ASS parse → render → quantize → encode.

use std::collections::hash_map::DefaultHasher;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::Path;

use color_quantizer::QuantizedFrame;

use crate::domain::composer::compose_frame;
use crate::domain::error::AssError;
use crate::domain::frame::AssEventInfo;
use crate::domain::renderer::{extract_font_families, AssRenderer};
use crate::domain::timeline::generate_timestamps;
use crate::infra::pgs_adapter::{create_pipeline, encode_bdn, encode_sup};
use crate::infra::vendor::crop_to_tight_bbox;

/// User-facing configuration for a conversion run.
#[derive(Debug, Clone)]
pub struct ConversionConfig {
    /// Output video framerate (e.g. 23.976).
    pub fps: f64,
    /// Output bitmap width.
    pub width: u32,
    /// Output bitmap height.
    pub height: u32,
    /// Maximum palette colours (1–255).
    pub max_colors: usize,
    /// Dither method: "none", "floyd-steinberg", or "ordered".
    pub dither: String,
    /// Default font family for fontconfig.
    pub default_font: Option<String>,
    /// User-provided font directories — each is scanned for font files
    /// and registered via `ass_add_font`, giving system + user two-level matching.
    pub fonts_dirs: Vec<String>,
    /// Per-style font fallback map (style_name → fallback family).
    pub font_fallback_map: std::collections::HashMap<String, String>,
    /// Check font availability before rendering.
    pub check_fonts: bool,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            fps: 23.976,
            width: 1920,
            height: 1080,
            max_colors: 255,
            dither: "floyd-steinberg".into(),
            default_font: None,
            fonts_dirs: Vec::new(),
            font_fallback_map: std::collections::HashMap::new(),
            check_fonts: false,
        }
    }
}

/// Statistics reported after a successful conversion.
#[derive(Debug, Clone)]
pub struct ConversionStats {
    /// Number of subtitle events processed.
    pub events_processed: usize,
    /// Number of unique frames encoded.
    pub frames_encoded: u64,
    /// Number of duplicates skipped (smart rendering).
    pub duplicates_skipped: u64,
    /// Number of empty frames skipped.
    pub empty_skipped: u64,
    /// Total output size in bytes.
    pub output_size: usize,
}

/// Hash the indices and palette of a QuantizedFrame for duplicate detection.
pub fn hash_quantized(frame: &QuantizedFrame) -> u64 {
    let mut hasher = DefaultHasher::new();
    frame.indices.hash(&mut hasher);
    // Hash palette as a flat byte slice
    for c in &frame.palette {
        c.r.hash(&mut hasher);
        c.g.hash(&mut hasher);
        c.b.hash(&mut hasher);
        c.a.hash(&mut hasher);
    }
    frame.transparent_index.hash(&mut hasher);
    hasher.finish()
}

/// Top-level orchestrator for ASS → SUP conversion.
pub struct Ass2Sup;

impl Ass2Sup {
    /// Convert an ASS file to SUP binary and write it to disk.
    pub fn convert_file(
        input: &Path,
        output: &Path,
        config: &ConversionConfig,
    ) -> Result<ConversionStats, AssError> {
        let content = std::fs::read_to_string(input)?;
        let frames = Self::process_events(&content, config)?;
        let sup_data = encode_sup(
            &frames,
            config.width as u16,
            config.height as u16,
            config.fps,
        )?;
        let output_size = sup_data.len();
        std::fs::write(output, &sup_data)?;
        Ok(ConversionStats {
            events_processed: frames.len(),
            frames_encoded: frames.len() as u64,
            duplicates_skipped: 0,
            empty_skipped: 0,
            output_size,
        })
    }

    /// Convert an ASS file to BDN XML + PNG sequence.
    pub fn convert_to_bdn(
        input: &Path,
        output_dir: &Path,
        config: &ConversionConfig,
    ) -> Result<ConversionStats, AssError> {
        std::fs::create_dir_all(output_dir)?;
        let content = std::fs::read_to_string(input)?;
        let frames = Self::process_events(&content, config)?;
        let name = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let count = encode_bdn(
            &frames,
            name,
            config.width,
            config.height,
            config.fps,
            output_dir,
        )?;
        Ok(ConversionStats {
            events_processed: frames.len(),
            frames_encoded: count as u64,
            duplicates_skipped: 0,
            empty_skipped: 0,
            output_size: 0,
        })
    }

    /// Internal: process events through the full render → quantize → deduplicate pipeline.
    /// Mirrors the original ass2sup's `render_and_quantize` logic.
    fn process_events(
        content: &str,
        config: &ConversionConfig,
    ) -> Result<Vec<QuantizedFrame>, AssError> {
        // Apply font-fallback-map (rewrite Fontname in style definitions)
        let content = Self::apply_font_fallback_map(content, &config.font_fallback_map);

        // Optional font availability check
        if config.check_fonts {
            let issues = Self::check_font_availability(&content, &config.font_fallback_map);
            if issues
                .iter()
                .any(|(_, _, resolved)| resolved == "NOT FOUND")
            {
                let missing: Vec<_> = issues
                    .iter()
                    .filter(|(_, _, r)| r == "NOT FOUND")
                    .map(|(s, f, _)| format!("{}:{}", s, f))
                    .collect();
                return Err(AssError::Config(format!(
                    "Fonts not found: {}",
                    missing.join(", ")
                )));
            }
        }

        let needed_families = extract_font_families(&content);

        let mut renderer = AssRenderer::new(config.width, config.height)?;
        renderer.load_ass(&content)?;
        renderer.configure_fonts(
            config.default_font.as_deref(),
            &config.fonts_dirs,
            &needed_families,
        )?;

        let events = renderer.events();
        if events.is_empty() {
            return Err(AssError::NoEvents);
        }

        let timestamps = generate_timestamps(&events, config.fps);
        if timestamps.is_empty() {
            return Err(AssError::NoEvents);
        }

        let pipeline = create_pipeline(config.max_colors, &config.dither, config.height);
        let mut output_frames: Vec<QuantizedFrame> = Vec::new();
        let mut prev_data_hash: Option<u64> = None;
        let mut empty_skipped = 0u64;
        let mut dup_skipped = 0u64;

        let mut sorted_events: Vec<&AssEventInfo> = events.iter().collect();
        sorted_events.sort_by_key(|e| e.start_ms);

        let last_event_end = sorted_events
            .iter()
            .map(|e| e.start_ms + e.duration_ms)
            .max()
            .unwrap_or(0) as u64;

        let mut event_cursor = 0usize;
        let mut active_ends: BinaryHeap<i64> = BinaryHeap::new();

        for window in timestamps.windows(2) {
            let ts = window[0];
            let next_ts = window[1];

            while event_cursor < sorted_events.len() {
                let e = sorted_events[event_cursor];
                if (e.start_ms as u64) <= ts {
                    let end = e.start_ms + e.duration_ms;
                    active_ends.push(-end);
                    event_cursor += 1;
                } else {
                    break;
                }
            }

            while let Some(&neg_end) = active_ends.peek() {
                let end = (-neg_end) as u64;
                if end <= ts {
                    active_ends.pop();
                } else {
                    break;
                }
            }

            if active_ends.is_empty() {
                continue;
            }

            // Render via libass
            let images = match renderer.render_frame(ts as i64)? {
                Some(imgs) if !imgs.is_empty() => imgs,
                _ => {
                    empty_skipped += 1;
                    continue;
                }
            };

            // Compose RGBA frame
            let rgba = compose_frame(&images, config.width, config.height);

            // Crop to tight bbox — skip fully transparent
            let cropped = match crop_to_tight_bbox(&rgba.data, config.width, config.height) {
                Some(c) => c,
                None => {
                    empty_skipped += 1;
                    continue;
                }
            };

            let cropped_frame = crate::domain::frame::CroppedFrame {
                data: cropped.0,
                x: cropped.1,
                y: cropped.2,
                width: cropped.3,
                height: cropped.4,
            };

            // Quantize with palette reuse (mirrors original's quantize_with_prev)
            let prev_frame = output_frames.last();
            let qf = if config.dither != "none" && prev_frame.is_some() {
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
                q
            } else {
                let mut q = pipeline.quantize(
                    &cropped_frame.data,
                    cropped_frame.width,
                    cropped_frame.height,
                );
                q.x = cropped_frame.x as u16;
                q.y = cropped_frame.y as u16;
                q.pts_ms = ts;
                q.duration_ms = next_ts.saturating_sub(ts).max(1);
                q
            };

            // Duplicate detection via hash (indices + palette + x + y)
            let hash = hash_quantized(&qf);
            if prev_data_hash == Some(hash) {
                // Duplicate: extend previous frame's duration
                if let Some(last) = output_frames.last_mut() {
                    last.duration_ms = ts + qf.duration_ms - last.pts_ms;
                }
                dup_skipped += 1;
                continue;
            }

            prev_data_hash = Some(hash);
            output_frames.push(qf);
        }

        // Fix up last frame duration to cover until last_event_end.
        if let Some(last) = output_frames.last_mut() {
            if last.pts_ms + last.duration_ms < last_event_end {
                last.duration_ms = last_event_end.saturating_sub(last.pts_ms);
            }
        }

        tracing::info!(
            rendered = output_frames.len(),
            empty_skipped,
            dup_skipped,
            "rendering complete"
        );

        Ok(output_frames)
    }

    /// Apply font-fallback-map by rewriting Fontname in ASS style definitions.
    pub fn apply_font_fallback_map(content: &str, map: &HashMap<String, String>) -> String {
        if map.is_empty() {
            return content.to_string();
        }
        let mut result = String::with_capacity(content.len());
        let mut in_styles = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[V4+ Styles]")
                || trimmed.starts_with("[V4 Styles]")
                || trimmed.starts_with("[Styles]")
            {
                in_styles = true;
                result.push_str(line);
                result.push('\n');
                continue;
            }
            if in_styles && trimmed.starts_with('[') {
                in_styles = false; // next section
            }
            if in_styles && trimmed.starts_with("Style:") {
                let mut rewritten = line.to_string();
                for (style_name, fallback_font) in map {
                    if let Some(header_end) = rewritten.find("Style:") {
                        let after_style = &rewritten[header_end + 6..];
                        let parts: Vec<&str> = after_style.splitn(3, ',').collect();
                        if parts.len() >= 2 && parts[0].trim() == style_name.as_str() {
                            let rest = parts.get(2).copied().unwrap_or("");
                            let new_line =
                                format!("Style: {}, {},{}", style_name, fallback_font, rest);
                            tracing::info!(
                                style = %style_name,
                                original = %parts[1].trim(),
                                fallback = %fallback_font,
                                "font fallback map applied"
                            );
                            rewritten = new_line;
                            break;
                        }
                    }
                }
                result.push_str(&rewritten);
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
        result
    }

    /// Check font availability using fc-match.
    /// Returns a list of (style_name, requested_font, resolved_font) for
    /// cases where fc-match returns a different font family.
    pub fn check_font_availability(
        content: &str,
        map: &HashMap<String, String>,
    ) -> Vec<(String, String, String)> {
        let mut issues = Vec::new();
        let mut in_styles = false;
        let mut in_events = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("[V4+ Styles]") || trimmed.starts_with("[V4 Styles]") {
                in_styles = true;
                in_events = false;
                continue;
            }
            if trimmed.starts_with("[Events]") {
                in_styles = false;
                in_events = true;
                continue;
            }
            if (in_styles && trimmed.starts_with('[')) || (in_events && trimmed.starts_with('[')) {
                in_styles = false;
                in_events = false;
            }

            // ── Style section: check Fontname ──────────────────────────
            if in_styles && trimmed.starts_with("Style:") {
                let after_style = trimmed.strip_prefix("Style:").unwrap_or("").trim();
                let parts: Vec<&str> = after_style.splitn(3, ',').collect();
                if parts.len() < 2 {
                    continue;
                }
                let style_name = parts[0].trim().to_string();
                let font_name = parts[1].trim();

                let check_font = map
                    .get(style_name.as_str())
                    .map(|s| s.as_str())
                    .unwrap_or(font_name);

                Self::check_single_font(&mut issues, &style_name, check_font);
            }

            // ── Events section: scan inline \fn{FontName} ─────────────
            if in_events && trimmed.starts_with("Dialogue:") {
                // Text is after the 10th comma (0-indexed: field 9)
                let text = trimmed.split(',').skip(9).collect::<Vec<_>>().join(",");
                // Find all \fnFontName occurrences (not just \fn{...})
                let mut pos = 0;
                let bytes = text.as_bytes();
                while pos < bytes.len() {
                    if bytes[pos] == b'\\'
                        && pos + 2 < bytes.len()
                        && bytes[pos + 1] == b'f'
                        && bytes[pos + 2] == b'n'
                    {
                        // Check for \fn followed by non-'{' (font name) or '{' ... '}'
                        let start = pos + 3;
                        if start < bytes.len() && bytes[start] == b'{' {
                            // \fn{...} — skip
                            pos = start + 1;
                            continue;
                        }
                        // \fnFontName — collect chars until \ or }
                        let mut font_chars = Vec::new();
                        let mut p = start;
                        while p < bytes.len() && bytes[p] != b'\\' && bytes[p] != b'}' {
                            if !bytes[p].is_ascii_whitespace() {
                                font_chars.push(bytes[p]);
                            }
                            p += 1;
                        }
                        if !font_chars.is_empty() {
                            let inline_font = String::from_utf8_lossy(&font_chars);
                            if !inline_font.is_empty() {
                                Self::check_single_font(&mut issues, "(inline)", &inline_font);
                            }
                        }
                        pos = p;
                    } else {
                        pos += 1;
                    }
                }
            }
        }
        issues
    }

    /// Check a single font name via fc-match, record any issue.
    fn check_single_font(
        issues: &mut Vec<(String, String, String)>,
        source: &str,
        check_font: &str,
    ) {
        let output = std::process::Command::new("fc-match")
            .arg("--format=%{family[0]}")
            .arg(check_font)
            .output();
        let resolved = match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
            _ => String::new(),
        };

        if resolved.is_empty() {
            issues.push((
                source.to_string(),
                check_font.to_string(),
                "NOT FOUND".into(),
            ));
            tracing::warn!(
                source = %source,
                requested = %check_font,
                "FONT MISSING — no match found via fontconfig"
            );
        } else if resolved.to_lowercase() != check_font.to_lowercase() {
            issues.push((source.to_string(), check_font.to_string(), resolved.clone()));
            tracing::warn!(
                source = %source,
                requested = %check_font,
                resolved = %resolved,
                "font fallback — exact match not found, using fallback"
            );
        } else {
            tracing::info!(
                source = %source,
                font = %check_font,
                "font OK — exact match found"
            );
        }
    }
}
