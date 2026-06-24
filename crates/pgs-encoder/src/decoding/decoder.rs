use crate::domain::composition::{CompositionState, WindowDef};
use crate::domain::palette::PaletteEntry;
use crate::domain::segment::SegmentType;
use std::fmt;

#[derive(Debug, Clone)]
pub struct ParsedObjectComposition {
    pub object_id: u16,
    pub window_id: u8,
    pub forced: bool,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone)]
pub enum ParsedPayload {
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
    WindowDefinition {
        windows: Vec<WindowDef>,
    },
    PaletteDefinition {
        palette_id: u8,
        version: u8,
        entries: Vec<PaletteEntry>,
    },
    ObjectDefinition {
        object_id: u16,
        version: u8,
        width: u16,
        height: u16,
        first_in_sequence: bool,
        last_in_sequence: bool,
        data: Vec<u8>,
    },
    End,
}

#[derive(Debug, Clone)]
pub struct ParsedSegment {
    pub pts: u64,
    pub dts: u64,
    pub payload: ParsedPayload,
}

#[derive(Debug, Clone)]
pub struct DisplaySet {
    pub segments: Vec<ParsedSegment>,
}

#[derive(Debug, Clone)]
pub enum DecodeError {
    InvalidMagic,
    DataTooShort,
    TruncatedPayload,
    InvalidSegmentType(u8),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::InvalidMagic => write!(f, "invalid PG magic"),
            DecodeError::DataTooShort => write!(f, "data too short"),
            DecodeError::TruncatedPayload => write!(f, "truncated payload"),
            DecodeError::InvalidSegmentType(t) => write!(f, "invalid segment type: {t:#x}"),
        }
    }
}

fn read_be16(data: &[u8], offset: usize) -> Option<u16> {
    if offset + 2 > data.len() {
        None
    } else {
        Some(u16::from_be_bytes([data[offset], data[offset + 1]]))
    }
}
fn read_be32(data: &[u8], offset: usize) -> Option<u32> {
    if offset + 4 > data.len() {
        None
    } else {
        Some(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]))
    }
}

fn parse_pds(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.len() < 2 {
        return Err(DecodeError::TruncatedPayload);
    }
    let mut entries = Vec::new();
    for i in 0..(data[2..].len() / 5) {
        let o = 2 + i * 5;
        entries.push(PaletteEntry {
            index: data[o],
            y: data[o + 1],
            cr: data[o + 2],
            cb: data[o + 3],
            alpha: data[o + 4],
        });
    }
    Ok(ParsedPayload::PaletteDefinition {
        palette_id: data[0],
        version: data[1],
        entries,
    })
}

fn parse_ods(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.len() < 4 {
        return Err(DecodeError::TruncatedPayload);
    }
    let object_id = read_be16(data, 0).ok_or(DecodeError::TruncatedPayload)?;
    let flags = data[3];
    let first = flags & 0x80 != 0;
    let last = flags & 0x40 != 0;
    if first {
        if data.len() < 11 {
            return Err(DecodeError::TruncatedPayload);
        }
        Ok(ParsedPayload::ObjectDefinition {
            object_id,
            version: data[2],
            width: read_be16(data, 7).ok_or(DecodeError::TruncatedPayload)?,
            height: read_be16(data, 9).ok_or(DecodeError::TruncatedPayload)?,
            first_in_sequence: true,
            last_in_sequence: last,
            data: data[11..].to_vec(),
        })
    } else {
        Ok(ParsedPayload::ObjectDefinition {
            object_id,
            version: data[2],
            width: 0,
            height: 0,
            first_in_sequence: false,
            last_in_sequence: last,
            data: data[4..].to_vec(),
        })
    }
}

fn parse_pcs(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.len() < 11 {
        return Err(DecodeError::TruncatedPayload);
    }
    let mut objects = Vec::new();
    let mut off = 11usize;
    for _ in 0..data[10] {
        let sz = if off + 4 <= data.len() && data[off + 3] & 0x80 != 0 {
            16
        } else {
            8
        };
        if off + sz > data.len() {
            break;
        }
        objects.push(ParsedObjectComposition {
            object_id: read_be16(data, off).ok_or(DecodeError::TruncatedPayload)?,
            window_id: data[off + 2],
            forced: data[off + 3] & 0x40 != 0,
            x: read_be16(data, off + 4).ok_or(DecodeError::TruncatedPayload)?,
            y: read_be16(data, off + 6).ok_or(DecodeError::TruncatedPayload)?,
        });
        off += sz;
    }
    let state = match data[7] {
        0x40 => CompositionState::AcquirePoint,
        0x80 => CompositionState::EpochStart,
        _ => CompositionState::NormalCase,
    };
    Ok(ParsedPayload::PresentationComposition {
        width: read_be16(data, 0).ok_or(DecodeError::TruncatedPayload)?,
        height: read_be16(data, 2).ok_or(DecodeError::TruncatedPayload)?,
        frame_rate: data[4],
        composition_number: read_be16(data, 5).ok_or(DecodeError::TruncatedPayload)?,
        state,
        palette_update: data[8] & 0x80 != 0,
        palette_id: data[9] & 0x7F,
        objects,
    })
}

fn parse_wds(data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    if data.is_empty() {
        return Err(DecodeError::TruncatedPayload);
    }
    let mut windows = Vec::new();
    let mut off = 1usize;
    for _ in 0..data[0] {
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

fn parse_payload(t: u8, data: &[u8]) -> Result<ParsedPayload, DecodeError> {
    match SegmentType::from_u8(t).ok_or(DecodeError::InvalidSegmentType(t))? {
        SegmentType::Pds => parse_pds(data),
        SegmentType::Ods => parse_ods(data),
        SegmentType::Pcs => parse_pcs(data),
        SegmentType::Wds => parse_wds(data),
        SegmentType::End => Ok(ParsedPayload::End),
    }
}

pub fn decode_segment(data: &[u8], offset: usize) -> Result<(ParsedSegment, usize), DecodeError> {
    if offset + 13 > data.len() {
        return Err(DecodeError::DataTooShort);
    }
    if data[offset] != 0x50 || data[offset + 1] != 0x47 {
        return Err(DecodeError::InvalidMagic);
    }
    let pts = u64::from(read_be32(data, offset + 2).ok_or(DecodeError::TruncatedPayload)?);
    let dts = u64::from(read_be32(data, offset + 6).ok_or(DecodeError::TruncatedPayload)?);
    let sz = read_be16(data, offset + 11).ok_or(DecodeError::TruncatedPayload)? as usize;
    Ok((
        ParsedSegment {
            pts: pts & 0x1FFFFFFFF,
            dts: dts & 0x1FFFFFFFF,
            payload: parse_payload(data[offset + 10], &data[offset + 13..offset + 13 + sz])?,
        },
        13 + sz,
    ))
}

pub fn decode_sup(data: &[u8]) -> Result<Vec<DisplaySet>, DecodeError> {
    let mut off = 0;
    let mut sets = Vec::new();
    let mut cur = DisplaySet {
        segments: Vec::new(),
    };
    while off < data.len() {
        let (s, c) = decode_segment(data, off)?;
        off += c;
        if matches!(s.payload, ParsedPayload::End) {
            cur.segments.push(s);
            sets.push(cur);
            cur = DisplaySet {
                segments: Vec::new(),
            };
        } else {
            cur.segments.push(s);
        }
    }
    if !cur.segments.is_empty() {
        sets.push(cur);
    }
    Ok(sets)
}

pub fn verify_roundtrip(original: &[u8]) -> Result<(), String> {
    let sets = decode_sup(original).map_err(|e| format!("{e}"))?;
    if sets.is_empty() && !original.is_empty() {
        return Err("no display sets".into());
    }
    for (i, ds) in sets.iter().enumerate() {
        if ds.segments.is_empty() {
            return Err(format!("set {i} empty"));
        }
        if !matches!(ds.segments.last().unwrap().payload, ParsedPayload::End) {
            return Err(format!("set {i} missing END"));
        }
    }
    Ok(())
}
