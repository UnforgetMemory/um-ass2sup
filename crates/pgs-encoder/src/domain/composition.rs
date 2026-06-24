/// Composition state for a PCS segment (ISO 14496-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionState {
    /// Normal case — objects within epoch may have changed.
    NormalCase = 0x00,
    /// Acquisition point — decoder may start decoding from this point.
    AcquirePoint = 0x40,
    /// Epoch start — new epoch, decoder must flush previous state.
    EpochStart = 0x80,
    /// Epoch continue — same epoch, composition state unchanged.
    EpochContinue = 0xC0,
}

impl CompositionState {
    /// Returns the raw byte value for PGS serialization.
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// An object composition within a PCS segment.
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

/// Window definition for WDS segment.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowDef {
    pub window_id: u8,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}
