//! BDN XML generation and PNG encoding utilities for Blu-ray subtitle authoring.
//!
//! This module provides functions to serialize [`BdnXml`] structures into
//! Blu-ray Disc Movie XML (BDN) format and to encode quantized subtitle
//! frames as palette-indexed PNG files suitable for Blu-ray authoring workflows.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::error::BdnError;
use crate::types::BdnXml;

/// Generates a BDN XML string from a [`BdnXml`] document structure.
///
/// Produces a UTF-8 encoded XML document conforming to the BDN (Blu-ray Disc
/// Movie) subtitle format. The output includes an XML declaration, a `<BDN>`
/// root element with a `Version` attribute, a `<Description>` block containing
/// metadata (`Name`, `Language`, `Format`, `Content`), and an `<Events>` block
/// with one `<Event>` per subtitle, each containing a `<Graphic>` reference.
///
/// # Arguments
///
/// * `bdn` - The BDN document to serialize.
///
/// # Errors
///
/// Returns [`BdnError::Xml`] if XML writing or UTF-8 conversion fails.
///
/// # Examples
///
/// ```no_run
/// use bdn_xml::{BdnXml, generate_xml};
///
/// let mut doc = BdnXml::new("My Movie", 1920, 1080);
/// // ... add events ...
/// let xml = generate_xml(&doc).expect("failed to generate XML");
/// assert!(xml.contains("<BDN"));
/// ```
pub fn generate_xml(bdn: &BdnXml) -> Result<String, BdnError> {
    let mut buf = Vec::new();
    let mut writer = Writer::new(&mut buf);

    writer
        .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| BdnError::Xml(e.to_string()))?;

    write_element(
        &mut writer,
        "BDN",
        None,
        Some(&[("Version", bdn.version.as_str())]),
    )?;
    write_element(&mut writer, "Description", None, None)?;
    write_text_element(&mut writer, "Name", &bdn.name)?;
    write_text_element(&mut writer, "Language", "eng")?;
    write_element(
        &mut writer,
        "Format",
        None,
        Some(&[("VideoFormat", bdn.format.as_str())]),
    )?;
    write_text_element(&mut writer, "Content", "")?;

    write_element(&mut writer, "Events", None, None)?;

    for event in &bdn.events {
        let forced_str = if event.forced { "true" } else { "false" };
        let area_attr = format!("{},{},{},{}", event.x, event.y, event.width, event.height);

        write_element(
            &mut writer,
            "Event",
            None,
            Some(&[
                ("InTC", event.in_tc.as_str()),
                ("OutTC", event.out_tc.as_str()),
                ("Forced", forced_str),
            ]),
        )?;
        write_element(
            &mut writer,
            "Graphic",
            None,
            Some(&[
                ("File", event.graphic.as_str()),
                ("Area", area_attr.as_str()),
            ]),
        )?;
        writer
            .write_event(Event::End(BytesEnd::new("Event")))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
    }

    writer
        .write_event(Event::End(BytesEnd::new("Events")))
        .map_err(|e| BdnError::Xml(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("Description")))
        .map_err(|e| BdnError::Xml(e.to_string()))?;
    writer
        .write_event(Event::End(BytesEnd::new("BDN")))
        .map_err(|e| BdnError::Xml(e.to_string()))?;

    String::from_utf8(buf).map_err(|e| BdnError::Xml(e.to_string()))
}

fn write_element(
    writer: &mut Writer<&mut Vec<u8>>,
    name: &str,
    text: Option<&str>,
    attrs: Option<&[(&str, &str)]>,
) -> Result<(), BdnError> {
    let mut elem = BytesStart::new(name);
    if let Some(attrs) = attrs {
        for (k, v) in attrs {
            elem.push_attribute((*k, *v));
        }
    }

    if let Some(text) = text {
        writer
            .write_event(Event::Start(elem))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
        writer
            .write_event(Event::Text(BytesText::new(text)))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
        writer
            .write_event(Event::End(BytesEnd::new(name)))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
    } else {
        writer
            .write_event(Event::Start(elem))
            .map_err(|e| BdnError::Xml(e.to_string()))?;
    }

    Ok(())
}

fn write_text_element(
    writer: &mut Writer<&mut Vec<u8>>,
    name: &str,
    text: &str,
) -> Result<(), BdnError> {
    write_element(writer, name, Some(text), None)
}

/// Converts a millisecond timestamp to a Blu-ray BDN timecode string.
///
/// The returned timecode uses the `HH:MM:SS:FF` format where `FF` is the
/// frame number within the current second, computed from the given frame rate.
/// Frame counts are rounded to the nearest integer.
///
/// # Arguments
///
/// * `ms` - Timestamp in milliseconds.
/// * `fps` - Frames per second (e.g. `24.0`, `23.976`, `29.97`).
///
/// # Examples
///
/// ```
/// use bdn_xml::ms_to_timecode;
///
/// assert_eq!(ms_to_timecode(0, 24.0), "00:00:00:00");
/// assert_eq!(ms_to_timecode(3661000, 24.0), "01:01:01:00");
/// ```
pub fn ms_to_timecode(ms: u64, fps: f64) -> String {
    let total_frames = (ms as f64 * fps / 1000.0).round() as u64;
    let frames = total_frames % fps as u64;
    let total_secs = total_frames / fps as u64;
    let secs = total_secs % 60;
    let total_mins = total_secs / 60;
    let mins = total_mins % 60;
    let hours = total_mins / 60;
    format!("{:02}:{:02}:{:02}:{:02}", hours, mins, secs, frames)
}

/// Encodes a quantized subtitle frame as a palette-indexed PNG image.
///
/// The PNG uses 8-bit indexed color with a PLTE (palette) and tRNS (alpha
/// transparency) chunk derived from the RGBA palette. Each entry in `indices`
/// is a byte index into the palette. This produces PNG files suitable for
/// embedding in Blu-ray BDN XML subtitle assets.
///
/// # Arguments
///
/// * `palette` - RGBA color palette with up to 256 entries.
/// * `indices` - Indexed pixel data (`width × height` bytes).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Errors
///
/// Returns [`BdnError::Png`] if PNG encoding fails.
///
/// # Examples
///
/// ```no_run
/// use bdn_xml::generate_png;
///
/// let palette = [[255u8; 4]; 256];
/// let indices = vec![0u8; 1920 * 1080];
/// let png = generate_png(&palette, &indices, 1920, 1080).unwrap();
/// ```
pub fn generate_png(
    palette: &[[u8; 4]],
    indices: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, BdnError> {
    use png::Encoder;

    let mut buf = Vec::new();
    {
        let mut encoder = Encoder::new(&mut buf, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);

        let mut plte = Vec::with_capacity(palette.len() * 3);
        let mut trns = Vec::with_capacity(palette.len());
        for color in palette {
            plte.extend_from_slice(&color[0..3]);
            trns.push(color[3]);
        }

        encoder.set_palette(&plte);
        encoder.set_trns(&trns);

        let mut writer = encoder
            .write_header()
            .map_err(|e| BdnError::Png(e.to_string()))?;
        writer
            .write_image_data(indices)
            .map_err(|e| BdnError::Png(e.to_string()))?;
    }

    Ok(buf)
}

/// Writes a quantized subtitle frame to a PNG file on disk.
///
/// Convenience wrapper around [`generate_png`] that serializes the palette-indexed
/// frame and writes the resulting bytes to the specified path. Used to save
/// individual subtitle graphic assets for Blu-ray BDN XML authoring.
///
/// # Arguments
///
/// * `path` - Destination file path for the PNG.
/// * `palette` - RGBA color palette with up to 256 entries.
/// * `indices` - Indexed pixel data (`width × height` bytes).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Errors
///
/// Returns [`BdnError::Png`] if PNG encoding fails, or an I/O error if the
/// file cannot be written.
pub fn save_frame_png(
    path: &std::path::Path,
    palette: &[[u8; 4]],
    indices: &[u8],
    width: u32,
    height: u32,
) -> Result<(), BdnError> {
    let data = generate_png(palette, indices, width, height)?;
    std::fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// (a) The output is always exactly 11 characters: `HH:MM:SS:FF`.
        /// fps is bounded to < 101 because `fps as u64` truncation produces
        /// frame values up to `fps as u64 - 1`; at fps >= 101 the frame
        /// component can become 3 digits, breaking the fixed-width format.
        #[test]
        fn format_invariant(ms in 0u64..86_400_000u64, fps in 1.0..101.0) {
            let tc = ms_to_timecode(ms, fps);
            assert_eq!(tc.len(), 11, "timecode ({ms}ms @ {fps}fps) = {tc:?}");
            let parts: Vec<&str> = tc.split(':').collect();
            assert_eq!(parts.len(), 4, "expected 4 colon-separated fields");
            for part in &parts {
                assert_eq!(part.len(), 2, "each field must be 2 digits, got {part:?}");
                assert!(part.chars().all(|c| c.is_ascii_digit()), "field {part:?} is not numeric");
            }
        }
    }

    proptest! {
        /// (b) Zero milliseconds always gives `00:00:00:00`.
        #[test]
        fn zero_invariant(fps in proptest::array::uniform2(24.0f64..=30.0f64)) {
            let fps = fps[0]; // just pick the first
            assert_eq!(ms_to_timecode(0, fps), "00:00:00:00");
        }
    }

    proptest! {
        /// (c) For integer fps, the last two characters (frames) are in `0..fps`.
        #[test]
        fn frame_component(ms in 1u64..86_400_000u64) {
            for fps in &[24.0f64, 25.0, 30.0] {
                let tc = ms_to_timecode(ms, *fps);
                let frames: u64 = tc[9..11].parse().unwrap();
                assert!(frames < *fps as u64,
                    "frame {frames} out of range [0,{}) for {ms}ms @ {fps}fps: {tc}",
                    fps);
            }
        }
    }

    /// (d) 24 hours at 24 fps gives `24:00:00:00` (hours can exceed 23).
    #[test]
    fn twenty_four_hour_boundary() {
        assert_eq!(ms_to_timecode(24 * 60 * 60 * 1000, 24.0), "24:00:00:00");
    }

    proptest! {
        /// (e) NTSC 23.976 fps — output is well-formed (format invariant).
        #[test]
        fn ntsc_handling(ms in 0u64..86_400_000u64) {
            let tc = ms_to_timecode(ms, 23.976);
            assert_eq!(tc.len(), 11, "NTSC timecode ({ms}ms) = {tc:?}");
            let parts: Vec<&str> = tc.split(':').collect();
            assert_eq!(parts.len(), 4);
            for part in &parts {
                assert_eq!(part.len(), 2);
                assert!(part.chars().all(|c| c.is_ascii_digit()));
            }
            // At 23.976 fps, `fps as u64` truncates to 23, so the frame
            // component should be in 0..23.
            let frames: u64 = tc[9..11].parse().unwrap();
            assert!(frames < 23, "frame {frames} should be < 23 for 23.976 fps");
        }
    }
}
