/// PGS segment type identifiers (ISO 14496-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentType {
    /// Presentation Composition Segment
    Pcs = 0x16,
    /// Window Definition Segment
    Wds = 0x17,
    /// Palette Definition Segment
    Pds = 0x14,
    /// Object Definition Segment
    Ods = 0x15,
    /// End of Display Set Segment
    End = 0x80,
}

impl SegmentType {
    /// Create from raw byte value.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x16 => Some(Self::Pcs),
            0x17 => Some(Self::Wds),
            0x14 => Some(Self::Pds),
            0x15 => Some(Self::Ods),
            0x80 => Some(Self::End),
            _ => None,
        }
    }
}

/// A PGS segment with parsed payload.
#[derive(Debug, Clone)]
pub struct Segment {
    pub segment_type: SegmentType,
    pub pts: u64,
    pub dts: u64,
    pub payload: SegmentPayload,
}

/// Parsed payload variants for each PGS segment type.
#[derive(Debug, Clone)]
pub enum SegmentPayload {
    Pcs(PcsPayload),
    Wds(WdsPayload),
    Pds(PdsPayload),
    Ods(OdsPayload),
    End,
}

/// Presentation Composition Segment payload.
#[derive(Debug, Clone)]
pub struct PcsPayload {
    pub width: u16,
    pub height: u16,
    pub frame_rate: u8,
    pub composition_number: u16,
    pub composition_state: super::composition::CompositionState,
    pub palette_update: bool,
    pub palette_id: u8,
    pub num_objects: u8,
    pub compositions: Vec<super::composition::ObjectComposition>,
}

/// Window Definition Segment payload.
#[derive(Debug, Clone)]
pub struct WdsPayload {
    pub num_windows: u8,
    pub windows: Vec<super::composition::WindowDef>,
}

/// Palette Definition Segment payload.
#[derive(Debug, Clone)]
pub struct PdsPayload {
    pub palette_id: u8,
    pub version: u8,
    pub entries: Vec<super::palette::PaletteEntry>,
}

/// Object Definition Segment payload.
#[derive(Debug, Clone)]
pub struct OdsPayload {
    pub object_id: u16,
    pub object_version: u8,
    pub first_in_sequence: bool,
    pub last_in_sequence: bool,
    pub width: u16,
    pub height: u16,
    pub rle_data: Vec<u8>,
    pub total_rle_size: usize,
}

/// SUP file containing a sequence of PGS segments.
#[derive(Debug, Clone, Default)]
pub struct SupFile {
    pub segments: Vec<Segment>,
}

impl SupFile {
    /// Create a new empty SUP file container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a PGS segment to the SUP file.
    pub fn add_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    /// Serialize the SUP file to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for seg in &self.segments {
            output.extend(seg.to_bytes());
        }
        output
    }
}

impl Segment {
    /// Serialize a segment to its PGS binary format.
    ///
    /// Format: "PG"(2) + PTS(4) + DTS(4) + type(1) + size(2) + payload
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_bytes = self.payload_to_bytes();
        let total_len = 13 + payload_bytes.len();
        let mut buf = Vec::with_capacity(total_len);

        buf.push(b'P');
        buf.push(b'G');
        buf.extend_from_slice(&(self.pts as u32).to_be_bytes());
        buf.extend_from_slice(&(self.dts as u32).to_be_bytes());
        buf.push(self.segment_type as u8);
        buf.extend_from_slice(&(payload_bytes.len() as u16).to_be_bytes());
        buf.extend_from_slice(&payload_bytes);

        buf
    }

    /// Lightweight size of the serialized segment, without allocating.
    ///
    /// Computes the exact byte length that [`Self::to_bytes`] would produce
    /// (13-byte header + payload length) using field-length arithmetic instead
    /// of materializing the buffer. Used for size accounting on the hot path.
    pub fn serialized_size(&self) -> usize {
        13 + self.payload_size()
    }

    fn payload_size(&self) -> usize {
        match &self.payload {
            SegmentPayload::End => 0,
            SegmentPayload::Pcs(ref p) => {
                let fixed = 11;
                let per_composition =
                    |c: &super::composition::ObjectComposition| if c.cropped { 16 } else { 8 };
                fixed + p.compositions.iter().map(per_composition).sum::<usize>()
            }
            SegmentPayload::Wds(ref w) => 1 + w.windows.len() * 9,
            SegmentPayload::Pds(ref p) => 2 + p.entries.len() * 5,
            SegmentPayload::Ods(ref o) => {
                4 + if o.first_in_sequence { 7 } else { 0 } + o.rle_data.len()
            }
        }
    }

    fn payload_to_bytes(&self) -> Vec<u8> {
        match &self.payload {
            SegmentPayload::End => Vec::new(),
            SegmentPayload::Pcs(ref p) => {
                let mut buf = Vec::with_capacity(11 + p.compositions.len() * 8);
                buf.extend_from_slice(&p.width.to_be_bytes());
                buf.extend_from_slice(&p.height.to_be_bytes());
                buf.push(p.frame_rate);
                buf.extend_from_slice(&p.composition_number.to_be_bytes());
                buf.push(p.composition_state.to_u8());
                buf.push(if p.palette_update { 0x80 } else { 0x00 });
                buf.push(p.palette_id & 0x7F);
                buf.push(p.num_objects);
                for comp in &p.compositions {
                    buf.extend_from_slice(&comp.object_id.to_be_bytes());
                    buf.push(comp.window_id);
                    let flags = if comp.cropped { 0x80 } else { 0x00 }
                        | if comp.forced { 0x40 } else { 0x00 };
                    buf.push(flags);
                    buf.extend_from_slice(&comp.x.to_be_bytes());
                    buf.extend_from_slice(&comp.y.to_be_bytes());
                    if comp.cropped {
                        buf.extend_from_slice(&comp.crop_x.to_be_bytes());
                        buf.extend_from_slice(&comp.crop_y.to_be_bytes());
                        buf.extend_from_slice(&comp.crop_w.to_be_bytes());
                        buf.extend_from_slice(&comp.crop_h.to_be_bytes());
                    }
                }
                buf
            }
            SegmentPayload::Wds(ref w) => {
                let mut buf = Vec::with_capacity(1 + w.windows.len() * 11);
                buf.push(w.num_windows);
                for win in &w.windows {
                    buf.push(win.window_id);
                    buf.extend_from_slice(&win.x.to_be_bytes());
                    buf.extend_from_slice(&win.y.to_be_bytes());
                    buf.extend_from_slice(&win.width.to_be_bytes());
                    buf.extend_from_slice(&win.height.to_be_bytes());
                }
                buf
            }
            SegmentPayload::Pds(ref p) => {
                let mut buf = Vec::with_capacity(2 + p.entries.len() * 5);
                buf.push(p.palette_id);
                buf.push(p.version);
                for entry in &p.entries {
                    buf.push(entry.index);
                    buf.push(entry.y);
                    buf.push(entry.cr);
                    buf.push(entry.cb);
                    buf.push(entry.alpha);
                }
                buf
            }
            SegmentPayload::Ods(ref o) => {
                let sequence_flags = match (o.first_in_sequence, o.last_in_sequence) {
                    (true, false) => 0x80u8,
                    (false, true) => 0x40u8,
                    (true, true) => 0xC0u8,
                    (false, false) => 0x00u8,
                };
                let mut buf = Vec::with_capacity(11 + o.rle_data.len());
                buf.extend_from_slice(&o.object_id.to_be_bytes());
                buf.push(o.object_version);
                buf.push(sequence_flags);
                if o.first_in_sequence {
                    let total_size = (4 + o.total_rle_size) as u32;
                    buf.push((total_size >> 16) as u8);
                    buf.push((total_size >> 8) as u8);
                    buf.push(total_size as u8);
                    buf.extend_from_slice(&o.width.to_be_bytes());
                    buf.extend_from_slice(&o.height.to_be_bytes());
                }
                buf.extend_from_slice(&o.rle_data);
                buf
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::composition::{CompositionState, ObjectComposition, WindowDef};
    use crate::domain::palette::PaletteEntry;

    fn make_segment(payload: SegmentPayload) -> Segment {
        let segment_type = match &payload {
            SegmentPayload::Pcs(_) => SegmentType::Pcs,
            SegmentPayload::Wds(_) => SegmentType::Wds,
            SegmentPayload::Pds(_) => SegmentType::Pds,
            SegmentPayload::Ods(_) => SegmentType::Ods,
            SegmentPayload::End => SegmentType::End,
        };
        Segment {
            segment_type,
            pts: 90000,
            dts: 89999,
            payload,
        }
    }

    fn assert_size_matches(seg: &Segment) {
        assert_eq!(seg.serialized_size(), seg.to_bytes().len());
    }

    #[test]
    fn serialized_size_matches_to_bytes_for_end() {
        assert_size_matches(&make_segment(SegmentPayload::End));
    }

    #[test]
    fn serialized_size_matches_to_bytes_for_pcs() {
        let pcs = PcsPayload {
            width: 1920,
            height: 1080,
            frame_rate: 0x10,
            composition_number: 1,
            composition_state: CompositionState::EpochStart,
            palette_update: true,
            palette_id: 0,
            num_objects: 2,
            compositions: vec![
                ObjectComposition {
                    object_id: 1,
                    window_id: 0,
                    cropped: false,
                    forced: false,
                    x: 100,
                    y: 200,
                    crop_x: 0,
                    crop_y: 0,
                    crop_w: 0,
                    crop_h: 0,
                },
                ObjectComposition {
                    object_id: 2,
                    window_id: 0,
                    cropped: true,
                    forced: true,
                    x: 300,
                    y: 400,
                    crop_x: 10,
                    crop_y: 20,
                    crop_w: 30,
                    crop_h: 40,
                },
            ],
        };
        assert_size_matches(&make_segment(SegmentPayload::Pcs(pcs)));
    }

    #[test]
    fn serialized_size_matches_to_bytes_for_wds() {
        let wds = WdsPayload {
            num_windows: 2,
            windows: vec![
                WindowDef {
                    window_id: 0,
                    x: 100,
                    y: 200,
                    width: 500,
                    height: 60,
                },
                WindowDef {
                    window_id: 1,
                    x: 0,
                    y: 0,
                    width: 1920,
                    height: 1080,
                },
            ],
        };
        assert_size_matches(&make_segment(SegmentPayload::Wds(wds)));
    }

    #[test]
    fn serialized_size_matches_to_bytes_for_pds() {
        let pds = PdsPayload {
            palette_id: 0,
            version: 1,
            entries: vec![
                PaletteEntry {
                    index: 0,
                    y: 16,
                    cb: 128,
                    cr: 128,
                    alpha: 0,
                },
                PaletteEntry {
                    index: 1,
                    y: 235,
                    cb: 240,
                    cr: 128,
                    alpha: 255,
                },
                PaletteEntry {
                    index: 255,
                    y: 16,
                    cb: 16,
                    cr: 240,
                    alpha: 128,
                },
            ],
        };
        assert_size_matches(&make_segment(SegmentPayload::Pds(pds)));
    }

    #[test]
    fn serialized_size_matches_to_bytes_for_ods() {
        // All four (first_in_sequence, last_in_sequence) combinations.
        for (first, last) in [(true, false), (false, true), (true, true), (false, false)] {
            let ods = OdsPayload {
                object_id: 0,
                object_version: 0,
                first_in_sequence: first,
                last_in_sequence: last,
                width: 100,
                height: 50,
                rle_data: vec![0x00, 0x64, 0x00, 0x00, 0x41, 0xFF, 0x00, 0x2A],
                total_rle_size: 8,
            };
            assert_size_matches(&make_segment(SegmentPayload::Ods(ods)));
        }
    }

    #[test]
    fn serialized_size_matches_to_bytes_for_ods_large_rle() {
        let ods = OdsPayload {
            object_id: 0,
            object_version: 1,
            first_in_sequence: true,
            last_in_sequence: true,
            width: 1920,
            height: 1080,
            rle_data: vec![0xAB; 4096],
            total_rle_size: 4096,
        };
        assert_size_matches(&make_segment(SegmentPayload::Ods(ods)));
    }
}
