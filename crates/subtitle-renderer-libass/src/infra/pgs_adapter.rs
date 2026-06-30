//! Adapter layer between domain types and the existing PGS/BDN crates.

use std::path::Path;

use color_quantizer::color::ColorSpace;
use color_quantizer::pipeline::ColorPipeline;
use color_quantizer::QuantizedFrame;
use pgs_encoder::PgsEncoder;

use crate::domain::error::AssError;
use crate::domain::frame::CroppedFrame;

/// Create a color pipeline with the given configuration.
///
/// For HD content (>576 lines), automatically uses BT.709 color space
/// for PGS compliance (matches the original ass2sup's logic).
pub fn create_pipeline(max_colors: usize, dither: &str, height: u32) -> ColorPipeline {
    let dither_method = match dither {
        "none" => color_quantizer::DitherMethod::None,
        "ordered" => color_quantizer::DitherMethod::Ordered,
        _ => color_quantizer::DitherMethod::FloydSteinberg,
    };

    let mut pipeline = ColorPipeline::new()
        .with_max_colors(max_colors.clamp(1, 255))
        .with_dither(dither_method);

    // HD content uses BT.709 for Blu-ray PGS compliance.
    if height > 576 {
        pipeline = pipeline.with_color_space(ColorSpace::Bt709);
    }

    pipeline
}

/// Quantize a cropped RGBA frame into a `QuantizedFrame`.
pub fn quantize_frame(
    pipeline: &ColorPipeline,
    frame: &CroppedFrame,
    pts_ms: u64,
    duration_ms: u64,
) -> Result<QuantizedFrame, AssError> {
    let mut quantized = pipeline.quantize(&frame.data, frame.width, frame.height);

    quantized.x = frame.x as u16;
    quantized.y = frame.y as u16;
    quantized.pts_ms = pts_ms;
    quantized.duration_ms = duration_ms;

    Ok(quantized)
}

/// Convert an ms timestamp to a frame-accurate PTS value (90kHz ticks).
///
/// Eliminates the sub-frame drift that naive `ms × 90` conversion
/// accumulates at NTSC rates (23.976, 29.97). Matches the original
/// ass2sup's `frame_accurate_pts`.
pub fn frame_accurate_pts(ms: u64, fps: f64) -> u64 {
    if pgs_encoder::domain::timing::is_ntsc_fps(fps) {
        // 23.976 = 24000/1001 → 15015/4 ticks per frame
        let frame = (ms as f64 * 24.0 / 1001.0).round() as u64;
        frame * 15015 / 4
    } else {
        let ticks_per = 90000.0 / fps;
        let frame = (ms as f64 * fps / 1000.0).round() as u64;
        (frame as f64 * ticks_per).round() as u64
    }
}

/// Encode a list of quantized frames into a complete SUP binary.
///
/// Handles frame-accurate PTS, gap detection (clear between non-contiguous
/// groups), and final clear at end of stream — matching the original
/// ass2sup's `encode_sup` logic.
pub fn encode_sup(
    frames: &[QuantizedFrame],
    width: u16,
    height: u16,
    fps: f64,
) -> Result<Vec<u8>, AssError> {
    let mut encoder = PgsEncoder::new(width, height, fps);
    let mut sup_file = pgs_encoder::SupFile::new();

    for (i, frame) in frames.iter().enumerate() {
        // Gap detection: if current frame starts after previous ended,
        // emit a clear segment at the gap point.
        if i > 0 {
            let prev = &frames[i - 1];
            let gap_start = prev.pts_ms + prev.duration_ms;
            if frame.pts_ms > gap_start {
                let clear_pts = frame_accurate_pts(gap_start, fps);
                for seg in encoder.emit_clear(clear_pts) {
                    sup_file.add_segment(seg);
                }
            }
        }

        // Use frame-accurate PTS for NTSC precision.
        let pts = frame_accurate_pts(frame.pts_ms, fps);
        let segments = encoder.encode_frame_at_pts(frame, pts, frame.duration_ms);

        for seg in segments {
            sup_file.add_segment(seg);
        }
    }

    // Final clear at end of stream.
    if let Some(last) = frames.last() {
        let clear_pts = frame_accurate_pts(last.pts_ms + last.duration_ms, fps);
        for seg in encoder.emit_clear(clear_pts) {
            sup_file.add_segment(seg);
        }
    }

    Ok(sup_file.to_bytes())
}

/// Write quantized frames to BDN XML + PNG files.
pub fn encode_bdn(
    frames: &[QuantizedFrame],
    name: &str,
    width: u32,
    height: u32,
    fps: f64,
    output_dir: &Path,
) -> Result<usize, AssError> {
    use bdn_xml::{ms_to_timecode, BdnEvent, BdnXml};

    let mut bdn = BdnXml::new(name, width, height);
    let mut count = 0usize;

    for (i, frame) in frames.iter().enumerate() {
        // Convert QuantizedFrame palette (Vec<Rgba>) to BDN palette (&[[u8;4]])
        let palette: Vec<[u8; 4]> = frame.palette.iter().map(|c| [c.r, c.g, c.b, c.a]).collect();

        let event = BdnEvent {
            index: (i + 1) as u32,
            in_tc: ms_to_timecode(frame.pts_ms, fps),
            out_tc: ms_to_timecode(frame.pts_ms + frame.duration_ms, fps),
            graphic: format!("{:04}.png", count + 1),
            x: frame.x as u32,
            y: frame.y as u32,
            width: frame.width,
            height: frame.height,
            forced: false,
        };

        // Save PNG file
        let png_path = output_dir.join(&event.graphic);
        bdn_xml::save_frame_png(
            &png_path,
            &palette,
            &frame.indices,
            frame.width,
            frame.height,
        )
        .map_err(|e| AssError::Encode(e.to_string()))?;

        bdn.add_event(event);
        count += 1;
    }

    // Write BDN XML
    let xml = bdn_xml::generate_xml(&bdn).map_err(|e| AssError::Encode(e.to_string()))?;
    std::fs::write(output_dir.join("BDN.xml"), xml)?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_ms_to_timecode() {
        // 0ms at 24fps
        assert_eq!(bdn_xml::ms_to_timecode(0, 24.0), "00:00:00:00");
        // 1000ms at 24fps = 00:00:01:00
        assert_eq!(bdn_xml::ms_to_timecode(1000, 24.0), "00:00:01:00");
        // 1500ms at 24fps = 00:00:01:12
        assert_eq!(bdn_xml::ms_to_timecode(1500, 24.0), "00:00:01:12");
    }
}
