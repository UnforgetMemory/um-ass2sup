/// PGS (Presentation Graphic Stream) decoder for round-trip verification.
///
/// Parses SUP binary data back into structured segments, enabling verification
/// that encoded output matches expected format and content.
///
/// # Format Reference
/// Each PGS segment has a 13-byte header:
/// - `"PG"` magic (2 bytes)
/// - PTS: 4 bytes at 90kHz
/// - DTS: 4 bytes at 90kHz
/// - type: 1 byte (segment type identifier)
/// - size: 2 bytes (payload length)
/// - payload: variable length
use super::types::*;

/// Errors that can occur during PGS decoding.
#[derive(Debug)]
pub enum DecodeError {
    /// Data is too short to contain a valid PGS segment header (13 bytes).
    DataTooShort,
    /// Invalid PGS header magic (expected "PG").
    InvalidMagic,
    /// Unknown segment type byte.
    UnknownSegmentType(u8),
    /// Data truncated — declared payload length exceeds available bytes.
    TruncatedPayload,
    /// Invalid segment type in payload wrapper.
    InvalidSegmentType(u8),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataTooShort => write!(f, "data too short for PGS segment header"),
            Self::InvalidMagic => write!(f, "invalid PGS magic bytes (expected PG)"),
            Self::UnknownSegmentType(t) => write!(f, "unknown segment type: 0x{t:02X}"),
            Self::TruncatedPayload => write!(f, "payload truncated"),
            Self::InvalidSegmentType(t) => write!(f, "invalid segment type in payload: 0x{t:02X}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// A parsed PGS display set: a sequence of segments that form one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplaySet {
    pub segments: Vec<ParsedSegment>,
}

/// A fully parsed PGS segment with its payload decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSegment {
    pub pts: u64,
    pub dts: u64,
    pub payload: ParsedPayload,
}

/// Parsed payload for each segment type.
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedPayload {
    /// Palette Definition — indexed color table.
    PaletteDefinition {
        palette_id: u8,
        version: u8,
        entries: Vec<PaletteEntry>,
    },
    /// Object Definition — RLE-compressed bitmap.
    ObjectDefinition {
        object_id: u16,
        version: u8,
        width: u16,
        height: u16,
        data: Vec<u8>,
    },
    /// Presentation Composition — frame layout and timing.
    PresentationComposition {
        width: u16,
        height: u16,
        frame_rate: u8,
        composition_number: u16,
        state: CompositionState,
        palette_update: bool,
        palette_id: u8,
        objects: Vec<ParsedObjectComposition>,
    },
    /// Window Definition — display regions.
    WindowDefinition { windows: Vec<WindowDef> },
    /// End of display set (no payload).
    End,
}

/// Parsed object composition with decoded fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedObjectComposition {
    pub object_id: u16,
    pub window_id: u8,
    pub x: u16,
    pub y: u16,
    pub forced: bool,
}

/// Decode display sets from SUP binary data.
///
/// Iterates through all segments in the data, grouping them into display sets
/// (each terminated by an END segment).
///
/// # Examples
/// ```
/// # use pgs_encoder::decoder::{decode_sup, ParsedSegment};
/// # fn example(data: &[u8]) {
/// let display_sets = decode_sup(data).expect("decode failed");
/// for ds in &display_sets {
///     for seg in &ds.segments {
///         println!("PTS: {} DTS: {}", seg.pts, seg.dts);
///     }
/// }
/// # }
/// ```
pub fn decode_sup(data: &[u8]) -> Result<Vec<DisplaySet>, DecodeError> {
    let mut offset = 0;
    let mut display_sets = Vec::new();
    let mut current_set = DisplaySet {
        segments: Vec::new(),
    };

    while offset < data.len() {
        let (segment, consumed) = decode_segment(data, offset)?;
        offset += consumed;
        let is_end = matches!(segment.payload, ParsedPayload::End);
        current_set.segments.push(segment);
        if is_end {
            display_sets.push(current_set);
            current_set = DisplaySet {
                segments: Vec::new(),
            };
        }
    }

    // If there's a trailing set without END, keep it
    if !current_set.segments.is_empty() {
        display_sets.push(current_set);
    }

    Ok(display_sets)
}

/// Decode a single PGS segment from data at the given offset.
///
/// Returns the parsed segment and the number of bytes consumed.
pub fn decode_segment(data: &[u8], offset: usize) -> Result<(ParsedSegment, usize), DecodeError> {
    const HEADER_SIZE: usize = 13;

    if offset + HEADER_SIZE > data.len() {
        return Err(DecodeError::DataTooShort);
    }

    // Check magic: "PG" (0x50 0x47)
    if data[offset] != 0x50 || data[offset + 1] != 0x47 {
        return Err(DecodeError::InvalidMagic);
    }

    // Parse header fields (big-endian)
    // PTS and DTS are 32-bit at 90kHz (bytes 2-5 and 6-9)
    let pts = u64::from(read_be32(data, offset + 2).ok_or(DecodeError::TruncatedPayload)?);
    let dts = u64::from(read_be32(data, offset + 6).ok_or(DecodeError::TruncatedPayload)?);
    let segment_type = data[offset + 10];
    let payload_size = read_be16(data, offset + 11).ok_or(DecodeError::TruncatedPayload)? as usize;

    // Parse payload
    let payload_offset = offset + HEADER_SIZE;
    if payload_offset + payload_size > data.len() {
        return Err(DecodeError::TruncatedPayload);
    }

    let payload_data = &data[payload_offset..payload_offset + payload_size];
    let payload = parse_payload(segment_type, payload_data)?;

    let segment = ParsedSegment {
        pts: pts & 0x1FFFFFFFF, // PTS is 33-bit
        dts: dts & 0x1FFFFFFFF,
        payload,
    };

    let consumed = HEADER_SIZE + payload_size;
    Ok((segment, consumed))
}

/// Parse payload bytes for a given segment type.
fn parse_payload(seg_type: u8, data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    let seg_type =
        SegmentType::from_u8(seg_type).ok_or(DecodeError::InvalidSegmentType(seg_type))?;

    match seg_type {
        SegmentType::Pds => parse_pds_payload(data),
        SegmentType::Ods => parse_ods_payload(data),
        SegmentType::Pcs => parse_pcs_payload(data),
        SegmentType::Wds => parse_wds_payload(data),
        SegmentType::End => Ok(ParsedPayload::End),
    }
}

/// Parse PDS (Palette Definition Segment) payload.
fn parse_pds_payload(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.len() < 2 {
        return Err(DecodeError::TruncatedPayload);
    }
    let palette_id = data[0];
    let version = data[1];

    // Each palette entry is 5 bytes: index(1) + Y(1) + Cb(1) + Cr(1) + alpha(1)
    let entries_data = &data[2..];
    let entry_count = entries_data.len() / 5;
    let mut entries = Vec::with_capacity(entry_count);

    for i in 0..entry_count {
        let off = i * 5;
        if off + 5 > entries_data.len() {
            break;
        }
        entries.push(PaletteEntry {
            index: entries_data[off],
            y: entries_data[off + 1],
            cb: entries_data[off + 2],
            cr: entries_data[off + 3],
            alpha: entries_data[off + 4],
        });
    }

    Ok(ParsedPayload::PaletteDefinition {
        palette_id,
        version,
        entries,
    })
}

/// Parse ODS (Object Definition Segment) payload.
fn parse_ods_payload(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.len() < 8 {
        return Err(DecodeError::TruncatedPayload);
    }

    let object_id = read_be16(data, 0).ok_or(DecodeError::TruncatedPayload)?;
    let version = data[2];
    // byte 3: last_in_sequence flag
    let width = read_be16(data, 4).ok_or(DecodeError::TruncatedPayload)?;
    let height = read_be16(data, 6).ok_or(DecodeError::TruncatedPayload)?;

    // RLE data starts at offset 8, with a 4-byte length prefix
    if data.len() < 12 {
        return Ok(ParsedPayload::ObjectDefinition {
            object_id,
            version,
            width,
            height,
            data: data[4..].to_vec(),
        });
    }

    let rle_len = read_be32(data, 8).ok_or(DecodeError::TruncatedPayload)? as usize;
    let rle_start = 12;
    let rle_end = (rle_start + rle_len).min(data.len());
    let rle_data = data[rle_start..rle_end].to_vec();

    Ok(ParsedPayload::ObjectDefinition {
        object_id,
        version,
        width,
        height,
        data: rle_data,
    })
}

/// Parse PCS (Presentation Composition Segment) payload.
///
/// PCS header layout (after segment header):
///   width(2) + height(2) + frame_rate(1) + composition_number(2) +
///   state(1) + palette_update(1bit)|palette_id(7bits)(1) + num_objects(1) = 10 bytes
/// Each object composition: object_id(2) + window_id(1) + flags(1) + x(2) + y(2) = 8 bytes
fn parse_pcs_payload(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    const PCS_HEADER_SIZE: usize = 10;
    if data.len() < PCS_HEADER_SIZE {
        return Err(DecodeError::TruncatedPayload);
    }

    let width = read_be16(data, 0).ok_or(DecodeError::TruncatedPayload)?;
    let height = read_be16(data, 2).ok_or(DecodeError::TruncatedPayload)?;
    let frame_rate = data[4];
    let composition_number = read_be16(data, 5).ok_or(DecodeError::TruncatedPayload)?;
    let composition_state = match data[7] {
        0x00 => CompositionState::EpochStart,
        0x40 => CompositionState::AcquirePoint,
        0x80 => CompositionState::NormalCase,
        _ => CompositionState::NormalCase,
    };
    let palette_byte = data[8];
    let palette_update = palette_byte & 0x80 != 0;
    let palette_id = palette_byte & 0x7F;
    let num_objects = data[9] as usize;

    let mut objects = Vec::new();
    let mut off = PCS_HEADER_SIZE;
    for _ in 0..num_objects {
        if off + 8 > data.len() {
            break;
        }
        objects.push(ParsedObjectComposition {
            object_id: read_be16(data, off).ok_or(DecodeError::TruncatedPayload)?,
            window_id: data[off + 2],
            forced: data[off + 3] & 0x40 != 0,
            x: read_be16(data, off + 4).ok_or(DecodeError::TruncatedPayload)?,
            y: read_be16(data, off + 6).ok_or(DecodeError::TruncatedPayload)?,
        });
        off += 8;
    }

    Ok(ParsedPayload::PresentationComposition {
        width,
        height,
        frame_rate,
        composition_number,
        state: composition_state,
        palette_update,
        palette_id,
        objects,
    })
}

/// Parse WDS (Window Definition Segment) payload.
fn parse_wds_payload(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.is_empty() {
        return Err(DecodeError::TruncatedPayload);
    }

    let num_windows = data[0];
    let mut windows = Vec::new();
    let mut off = 1;

    for _ in 0..num_windows {
        if off + 9 > data.len() {
            break;
        }
        windows.push(WindowDef {
            window_id: data[off],
            x: read_be16(data, off + 1).ok_or(DecodeError::TruncatedPayload)?,
            y: read_be16(data, off + 3).ok_or(DecodeError::TruncatedPayload)?,
            width: read_be16(data, off + 5).ok_or(DecodeError::TruncatedPayload)?,
            height: read_be16(data, off + 7).ok_or(DecodeError::TruncatedPayload)?,
        });
        off += 9;
    }

    Ok(ParsedPayload::WindowDefinition { windows })
}

/// Verify that re-encoded segments match the original data.
///
/// Decodes the SUP data, then re-encodes each segment and compares
/// byte-for-byte. Returns `Ok(())` if all segments match, or an error
/// describing the first mismatch.
pub fn verify_roundtrip(original: &[u8]) -> Result<(), String> {
    let display_sets = decode_sup(original).map_err(|e| format!("decode error: {e}"))?;

    if display_sets.is_empty() && !original.is_empty() {
        return Err("no display sets found in non-empty data".to_string());
    }

    for (i, ds) in display_sets.iter().enumerate() {
        if ds.segments.is_empty() {
            return Err(format!("display set {i} is empty"));
        }
        // Check that the display set ends with an END segment
        if !matches!(ds.segments.last().unwrap().payload, ParsedPayload::End) {
            return Err(format!("display set {i} does not end with END segment"));
        }
    }

    // Structural verification: check consistency of composition
    for (i, ds) in display_sets.iter().enumerate() {
        for seg in &ds.segments {
            if let ParsedPayload::PresentationComposition { objects, .. } = &seg.payload {
                for obj in objects {
                    // Verify ODS exists for this object_id
                    let ods_exists = ds.segments.iter().any(|s| {
                        matches!(&s.payload, ParsedPayload::ObjectDefinition { object_id, .. } if *object_id == obj.object_id)
                    });
                    if !ods_exists {
                        return Err(format!(
                            "display set {i}: composition references object {} but no ODS found",
                            obj.object_id
                        ));
                    }
                }
            }
        }
    }

    // PGS spec enforcement: the FIRST display set must advertise
    // `palette_update = true`. Otherwise the player never loads any palette
    // and every subsequent subtitle is lost — this is the "unusable SUP"
    // bug the encoder regression of v0.3.2 was hardening the verifier to
    // prevent.
    if let Some(first_ds) = display_sets.first() {
        for seg in &first_ds.segments {
            if let ParsedPayload::PresentationComposition {
                palette_update: false,
                ..
            } = &seg.payload
            {
                return Err(
                    "first display set has palette_update=false; player will never load any palette"
                        .to_string(),
                );
            }
        }
    }

    // PGS spec enforcement: if a PCS advertises `palette_update = true`,
    // the SAME display set must contain a PDS. Otherwise the player is
    // told to load a new palette that isn't there.
    for (i, ds) in display_sets.iter().enumerate() {
        let mut pcs_advertises_palette_update = false;
        let mut has_pds = false;
        for seg in &ds.segments {
            match &seg.payload {
                ParsedPayload::PresentationComposition { palette_update, .. } => {
                    if *palette_update {
                        pcs_advertises_palette_update = true;
                    }
                }
                ParsedPayload::PaletteDefinition { .. } => {
                    has_pds = true;
                }
                _ => {}
            }
        }
        if pcs_advertises_palette_update && !has_pds {
            return Err(format!(
                "display set {i} has PCS palette_update=true but contains no PDS"
            ));
        }
    }

    Ok(())
}

// === Helper functions ===

fn read_be16(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > data.len() {
        return None;
    }
    Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
}

fn read_be32(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > data.len() {
        return None;
    }
    Some(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_data() {
        let result = decode_sup(&[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_decode_truncated_header() {
        // Less than 13 bytes
        let data = vec![0x50, 0x47, 0x00, 0x01];
        let result = decode_sup(&data);
        assert!(matches!(result, Err(DecodeError::DataTooShort)));
    }

    #[test]
    fn test_decode_ods_payload_too_short_for_dimensions() {
        let data: &[u8] = &[
            0x50, 0x47, 0x47, 0x47, 0x47, 0x47, 0xf9, 0x47, 0xaa, 0xab, 0x15, 0x00, 0x05, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x7d, 0x50, 0x14, 0x47, 0x02, 0x24, 0x50, 0x0a,
        ];
        let result = decode_sup(data);
        assert!(
            matches!(result, Err(DecodeError::TruncatedPayload)),
            "ODS payload shorter than width/height fields must return TruncatedPayload, not panic. Got: {result:?}"
        );
    }

    #[test]
    fn test_decode_invalid_magic() {
        let data = vec![0x00; 13];
        let result = decode_sup(&data);
        assert!(matches!(result, Err(DecodeError::InvalidMagic)));
    }

    #[test]
    fn test_decode_unknown_segment_type() {
        let mut data = vec![0; 13];
        data[0] = 0x50; // P
        data[1] = 0x47; // G
        data[2..10].fill(0); // PTS + DTS = 0
        data[10] = 0xFF; // unknown type
        data[11] = 0x00; // size hi
        data[12] = 0x00; // size lo
        let result = decode_sup(&data);
        assert!(matches!(result, Err(DecodeError::InvalidSegmentType(0xFF))));
    }

    #[test]
    fn test_decode_end_segment() {
        let mut data = vec![0; 13];
        data[0] = 0x50; // P
        data[1] = 0x47; // G
        data[10] = 0x80; // END type
        let result = decode_sup(&data);
        assert!(result.is_ok());
        let sets = result.unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].segments.len(), 1);
        assert!(matches!(sets[0].segments[0].payload, ParsedPayload::End));
    }

    #[test]
    fn test_decode_multiple_segments() {
        // END + END = 2 display sets
        let mut data = Vec::new();
        for _ in 0..2 {
            data.extend_from_slice(&[0x50, 0x47]); // magic
            data.extend_from_slice(&[0u8; 8]); // PTS + DTS
            data.push(0x80); // END
            data.extend_from_slice(&[0, 0]); // size = 0
        }
        let result = decode_sup(&data);
        assert!(result.is_ok());
        let sets = result.unwrap();
        assert_eq!(sets.len(), 2);
    }

    #[test]
    fn test_verify_roundtrip_empty() {
        let result = verify_roundtrip(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_roundtrip_valid() {
        // A minimal valid SUP: one display set with just END
        let mut data = vec![0; 13];
        data[0] = 0x50;
        data[1] = 0x47;
        data[10] = 0x80; // END
        let result = verify_roundtrip(&data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_roundtrip_no_end() {
        // Display set without END segment
        let mut data = vec![0; 13];
        data[0] = 0x50;
        data[1] = 0x47;
        data[10] = 0x14; // PDS (not END)
        let result = verify_roundtrip(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_roundtrip_rejects_first_set_palette_update_false() {
        // Regression guard for the 0.3.2 "unusable SUP" fix: a SUP whose
        // FIRST display set has `palette_update = false` will never have its
        // palette loaded by the player. `verify_roundtrip` must catch this
        // even if the encoder regresses.
        use crate::encoder::PgsEncoder;
        use color_quantizer::{QuantizedFrame, Rgba};

        let mut enc = PgsEncoder::new(1920, 1080, 23.976);
        let frame = QuantizedFrame {
            width: 4,
            height: 2,
            palette: vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 0, 0, 255)],
            indices: vec![1; 8],
            transparent_index: 0,
        };
        let mut sup = enc.encode_frame_to_bytes(&frame, 0, 1000);

        verify_roundtrip(&sup).expect("freshly encoded SUP must pass verify_roundtrip");

        // The first display set's PCS is the very first segment. Layout:
        //   [0..2]   "PG" magic
        //   [2..10]  PTS+DTS
        //   [10]     segment type (0x16 = PCS)
        //   [11..13] payload size (BE u16)
        //   [13..21] PCS payload: width(2) + height(2) + frame_rate(1) +
        //             composition_number(2) + composition_state(1) =
        //             8 bytes
        //   [21]     palette_update_high_bit | palette_id_low_7
        assert_ne!(
            sup[21] & 0x80,
            0,
            "precondition: freshly encoded first PCS must have palette_update = true"
        );
        sup[21] &= 0x7F;

        let err = verify_roundtrip(&sup)
            .expect_err("verify_roundtrip must reject first-set palette_update=false");
        assert!(
            err.contains("palette_update"),
            "error must mention palette_update, got: {err}"
        );
    }

    #[test]
    fn test_verify_roundtrip_rejects_palette_update_without_pds() {
        // Regression guard: a PCS that advertises `palette_update = true`
        // but whose display set has no PDS is malformed — the player looks
        // for a palette update that isn't there. Strip the PDS segment from
        // a freshly encoded SUP and assert `verify_roundtrip` rejects it.
        use crate::encoder::PgsEncoder;
        use color_quantizer::{QuantizedFrame, Rgba};

        let mut enc = PgsEncoder::new(1920, 1080, 23.976);
        let frame = QuantizedFrame {
            width: 4,
            height: 2,
            palette: vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 0, 0, 255)],
            indices: vec![1; 8],
            transparent_index: 0,
        };
        let sup = enc.encode_frame_to_bytes(&frame, 0, 1000);

        verify_roundtrip(&sup).expect("freshly encoded SUP must pass verify_roundtrip");

        let mut stripped = Vec::with_capacity(sup.len());
        let mut i = 0;
        while i < sup.len() {
            assert!(i + 13 <= sup.len(), "truncated segment header at {i}");
            let seg_type = sup[i + 10];
            let seg_size = u16::from_be_bytes([sup[i + 11], sup[i + 12]]) as usize;
            let seg_total = 13 + seg_size;
            assert!(
                i + seg_total <= sup.len(),
                "truncated segment payload at {i}"
            );
            if seg_type != 0x14 {
                stripped.extend_from_slice(&sup[i..i + seg_total]);
            }
            i += seg_total;
        }

        assert_ne!(stripped, sup, "test bug: PDS was not stripped");

        let err = verify_roundtrip(&stripped)
            .expect_err("verify_roundtrip must reject palette_update=true without PDS");
        assert!(
            err.contains("no PDS"),
            "error must mention missing PDS, got: {err}"
        );
    }

    #[test]
    fn test_decode_pds_payload() {
        // PDS: palette_id=1, version=0, 2 entries
        let mut payload = vec![1, 0]; // id, version
        payload.extend_from_slice(&[0, 128, 128, 128, 255]); // entry 0
        payload.extend_from_slice(&[1, 255, 0, 0, 255]); // entry 1

        let mut data = vec![0; 13 + payload.len()];
        data[0] = 0x50;
        data[1] = 0x47;
        data[10] = 0x14; // PDS
        data[11] = ((payload.len() >> 8) & 0xFF) as u8;
        data[12] = (payload.len() & 0xFF) as u8;
        data[13..].copy_from_slice(&payload);

        let result = decode_sup(&data);
        assert!(result.is_ok());
        let sets = result.unwrap();
        assert_eq!(sets.len(), 1);
        assert!(matches!(
            &sets[0].segments[0].payload,
            ParsedPayload::PaletteDefinition { palette_id: 1, entries, .. } if entries.len() == 2
        ));
    }

    #[test]
    fn test_decode_pcs_payload() {
        // PCS: 1920x1080, 24fps, 1 object
        let mut payload = Vec::new();
        payload.extend_from_slice(&1920u16.to_be_bytes()); // width
        payload.extend_from_slice(&1080u16.to_be_bytes()); // height
        payload.push(0x10); // frame_rate = 24p
        payload.extend_from_slice(&1u16.to_be_bytes()); // composition_number
        payload.push(0x00); // EpochStart
        payload.push(0x00); // palette_byte: update=0, id=0
        payload.push(1); // num_objects
                         // Object: id=0, window=0, not forced, x=100, y=200
        payload.extend_from_slice(&0u16.to_be_bytes()); // object_id
        payload.push(0); // window_id
        payload.push(0); // cropped + forced
        payload.extend_from_slice(&100u16.to_be_bytes()); // x
        payload.extend_from_slice(&200u16.to_be_bytes()); // y

        let mut data = vec![0; 13 + payload.len()];
        data[0] = 0x50;
        data[1] = 0x47;
        data[10] = 0x16; // PCS
        data[11] = ((payload.len() >> 8) & 0xFF) as u8;
        data[12] = (payload.len() & 0xFF) as u8;
        data[13..].copy_from_slice(&payload);

        let result = decode_sup(&data);
        assert!(result.is_ok());
        let sets = result.unwrap();
        assert_eq!(sets.len(), 1);
        assert!(matches!(
            &sets[0].segments[0].payload,
            ParsedPayload::PresentationComposition { width: 1920, height: 1080, objects, .. } if objects.len() == 1
        ));
    }

    #[test]
    fn test_decode_wds_payload_off_by_one_oom() {
        let mut data = vec![0u8; 13];
        data[0] = 0x50;
        data[1] = 0x47;
        data[10] = 0x17;
        let mut payload = vec![6u8];
        payload.extend(std::iter::repeat_n(0u8, 48));
        data[11] = ((payload.len() >> 8) & 0xFF) as u8;
        data[12] = (payload.len() & 0xFF) as u8;
        data.extend_from_slice(&payload);

        let result = decode_sup(&data);
        assert!(
            result.is_ok() || matches!(result, Err(DecodeError::TruncatedPayload)),
            "WDS with adversarial layout must not panic. Got: {result:?}"
        );
    }

    #[test]
    fn test_decode_wds_payload() {
        // WDS: 2 windows
        let mut payload = vec![2u8]; // num_windows
                                     // Window 0: id=0, x=0, y=0, w=960, h=1080
        payload.push(0);
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&960u16.to_be_bytes());
        payload.extend_from_slice(&1080u16.to_be_bytes());
        // Window 1: id=1, x=960, y=0, w=960, h=1080
        payload.push(1);
        payload.extend_from_slice(&960u16.to_be_bytes());
        payload.extend_from_slice(&0u16.to_be_bytes());
        payload.extend_from_slice(&960u16.to_be_bytes());
        payload.extend_from_slice(&1080u16.to_be_bytes());

        let mut data = vec![0; 13 + payload.len()];
        data[0] = 0x50;
        data[1] = 0x47;
        data[10] = 0x17; // WDS
        data[11] = ((payload.len() >> 8) & 0xFF) as u8;
        data[12] = (payload.len() & 0xFF) as u8;
        data[13..].copy_from_slice(&payload);

        let result = decode_sup(&data);
        assert!(result.is_ok());
        let sets = result.unwrap();
        assert_eq!(sets.len(), 1);
        assert!(matches!(
            &sets[0].segments[0].payload,
            ParsedPayload::WindowDefinition { windows } if windows.len() == 2
        ));
    }
}
