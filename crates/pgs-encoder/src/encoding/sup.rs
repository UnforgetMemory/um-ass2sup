use crate::domain::segment::Segment;

/// Serialize a sequence of PGS segments to SUP binary format.
pub fn sup_to_bytes(segments: &[Segment]) -> Vec<u8> {
    let mut output = Vec::new();
    for seg in segments {
        output.extend(seg.to_bytes());
    }
    output
}
