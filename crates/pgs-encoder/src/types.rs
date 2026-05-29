/// PGS segment type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SegmentType {
    /// Palette Definition Segment
    Pds = 0x14,
    /// Object Definition Segment
    Ods = 0x15,
    /// Presentation Composition Segment
    Pcs = 0x16,
    /// Window Definition Segment
    Wds = 0x17,
    /// End of Display Set
    End = 0x80,
}

impl SegmentType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x14 => Some(Self::Pds),
            0x15 => Some(Self::Ods),
            0x16 => Some(Self::Pcs),
            0x17 => Some(Self::Wds),
            0x80 => Some(Self::End),
            _ => None,
        }
    }
}

/// Composition state for PCS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompositionState {
    /// New epoch starts
    EpochStart = 0x00,
    /// Acquisition point
    AcquirePoint = 0x40,
    /// Normal case
    NormalCase = 0x80,
}

/// Object composition entry in PCS
#[derive(Debug, Clone)]
pub struct ObjectComposition {
    pub object_id: u16,
    pub window_id: u8,
    pub cropped: bool,
    pub forced: bool,
    pub x: u16,
    pub y: u16,
    pub crop_x: u16,
    pub crop_y: u16,
    pub crop_w: u16,
    pub crop_h: u16,
}

/// PCS — Presentation Composition Segment payload
#[derive(Debug, Clone)]
pub struct PcsPayload {
    pub width: u16,
    pub height: u16,
    pub frame_rate: u8,
    pub composition_number: u16,
    pub composition_state: CompositionState,
    pub palette_update: bool,
    pub palette_id: u8,
    pub num_objects: u8,
    pub compositions: Vec<ObjectComposition>,
}

/// Window definition entry in WDS
#[derive(Debug, Clone)]
pub struct WindowDef {
    pub window_id: u8,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// WDS — Window Definition Segment payload
#[derive(Debug, Clone)]
pub struct WdsPayload {
    pub num_windows: u8,
    pub windows: Vec<WindowDef>,
}

/// Palette entry in YCbCrA color space
#[derive(Debug, Clone, Copy)]
pub struct PaletteEntry {
    pub index: u8,
    pub y: u8,
    pub cb: u8,
    pub cr: u8,
    pub alpha: u8,
}

/// PDS — Palette Definition Segment payload
#[derive(Debug, Clone)]
pub struct PdsPayload {
    pub palette_id: u8,
    pub version: u8,
    pub entries: Vec<PaletteEntry>,
}

/// ODS — Object Definition Segment payload
#[derive(Debug, Clone)]
pub struct OdsPayload {
    pub object_id: u16,
    pub object_version: u8,
    pub last_in_sequence: bool,
    pub width: u16,
    pub height: u16,
    pub rle_data: Vec<u8>,
}

/// Segment payload variants
#[derive(Debug, Clone)]
pub enum SegmentPayload {
    Pcs(PcsPayload),
    Wds(WdsPayload),
    Pds(PdsPayload),
    Ods(OdsPayload),
    End,
}

/// A single PGS segment with header info
#[derive(Debug, Clone)]
pub struct Segment {
    pub segment_type: SegmentType,
    pub pts: u64,
    pub dts: u64,
    pub payload: SegmentPayload,
}

/// A complete SUP file containing all segments
#[derive(Debug, Clone)]
pub struct SupFile {
    pub segments: Vec<Segment>,
}

impl SupFile {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn add_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    /// Write the SUP file to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for segment in &self.segments {
            output.extend(segment.to_bytes());
        }
        output
    }
}

impl Default for SupFile {
    fn default() -> Self {
        Self::new()
    }
}

impl Segment {
    /// Serialize segment to PGS binary format
    /// Header: "PG" (2) + PTS (4) + DTS (4) + type (1) + size (2) = 13 bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let payload_bytes = self.payload.to_bytes();
        let size = payload_bytes.len() as u16;

        let mut output = Vec::with_capacity(13 + payload_bytes.len());
        // Magic bytes
        output.push(b'P');
        output.push(b'G');
        // PTS (big-endian u32)
        output.extend_from_slice(&(self.pts as u32).to_be_bytes());
        // DTS (big-endian u32)
        output.extend_from_slice(&(self.dts as u32).to_be_bytes());
        // Segment type
        output.push(self.segment_type as u8);
        // Payload size (big-endian u16)
        output.extend_from_slice(&size.to_be_bytes());
        // Payload
        output.extend(payload_bytes);
        output
    }
}

impl SegmentPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SegmentPayload::Pcs(pcs) => pcs.to_bytes(),
            SegmentPayload::Wds(wds) => wds.to_bytes(),
            SegmentPayload::Pds(pds) => pds.to_bytes(),
            SegmentPayload::Ods(ods) => ods.to_bytes(),
            SegmentPayload::End => Vec::new(),
        }
    }
}

impl PcsPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        // Width (u16 BE)
        output.extend_from_slice(&self.width.to_be_bytes());
        // Height (u16 BE)
        output.extend_from_slice(&self.height.to_be_bytes());
        // Frame rate
        output.push(self.frame_rate);
        // Composition number (u16 BE)
        output.extend_from_slice(&self.composition_number.to_be_bytes());
        // Composition state
        output.push(self.composition_state as u8);
        // Palette update flag (1 bit) + palette_id (7 bits)
        let palette_byte = if self.palette_update { 0x80 } else { 0x00 } | (self.palette_id & 0x7F);
        output.push(palette_byte);
        // Number of objects
        output.push(self.num_objects);
        // Object compositions
        for comp in &self.compositions {
            // Object ID (u16 BE)
            output.extend_from_slice(&comp.object_id.to_be_bytes());
            // Window ID
            output.push(comp.window_id);
            // Object cropped flag (1 bit) + forced flag (1 bit) + reserved (6 bits)
            let flags = if comp.cropped { 0x80 } else { 0x00 }
                | if comp.forced { 0x40 } else { 0x00 };
            output.push(flags);
            // Composition X (u16 BE)
            output.extend_from_slice(&comp.x.to_be_bytes());
            // Composition Y (u16 BE)
            output.extend_from_slice(&comp.y.to_be_bytes());
            // If cropped, add crop values
            if comp.cropped {
                output.extend_from_slice(&comp.crop_x.to_be_bytes());
                output.extend_from_slice(&comp.crop_y.to_be_bytes());
                output.extend_from_slice(&comp.crop_w.to_be_bytes());
                output.extend_from_slice(&comp.crop_h.to_be_bytes());
            }
        }
        output
    }
}

impl WdsPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        // Number of windows
        output.push(self.num_windows);
        for win in &self.windows {
            // Window ID
            output.push(win.window_id);
            // X (u16 BE)
            output.extend_from_slice(&win.x.to_be_bytes());
            // Y (u16 BE)
            output.extend_from_slice(&win.y.to_be_bytes());
            // Width (u16 BE)
            output.extend_from_slice(&win.width.to_be_bytes());
            // Height (u16 BE)
            output.extend_from_slice(&win.height.to_be_bytes());
        }
        output
    }
}

impl PdsPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        // Palette ID
        output.push(self.palette_id);
        // Version
        output.push(self.version);
        // Palette entries: each is 5 bytes (index, Y, Cb, Cr, alpha)
        for entry in &self.entries {
            output.push(entry.index);
            output.push(entry.y);
            output.push(entry.cb);
            output.push(entry.cr);
            output.push(entry.alpha);
        }
        output
    }
}

impl OdsPayload {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        // Object ID (u16 BE)
        output.extend_from_slice(&self.object_id.to_be_bytes());
        // Object version
        output.push(self.object_version);
        // Last in sequence flag (1 bit) + reserved (7 bits)
        output.push(if self.last_in_sequence { 0x80 } else { 0x00 });
        // Width (u16 BE)
        output.extend_from_slice(&self.width.to_be_bytes());
        // Height (u16 BE)
        output.extend_from_slice(&self.height.to_be_bytes());
        // RLE data length (u32 BE) — includes the 4-byte length field itself
        let data_len = self.rle_data.len() as u32 + 4;
        output.extend_from_slice(&data_len.to_be_bytes());
        // RLE data
        output.extend(&self.rle_data);
        output
    }
}

/// Frame rate code mapping
pub fn frame_rate_code(fps: f64) -> u8 {
    if fps <= 24.0 {
        0x10
    } else if fps <= 25.0 {
        0x20
    } else if fps <= 30.0 {
        0x40
    } else if fps <= 50.0 {
        0x50
    } else if fps <= 60.0 {
        0x70
    } else {
        0x10 // default to 24p
    }
}
