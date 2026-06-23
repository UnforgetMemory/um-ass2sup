use pgs_encoder::decoder::{decode_sup, ParsedPayload, ParsedSegment};
use pgs_encoder::types::CompositionState;

fn main() {
    let sup_path = std::env::args()
        .nth(1)
        .expect("Usage: dump_pcs <input.sup> [label]");
    let label = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "unknown".to_string());

    let data = std::fs::read(&sup_path).expect("read sup");
    let display_sets = decode_sup(&data).expect("decode sup");
    println!("=== {}: {} ===", label, sup_path);
    println!("Total display sets: {}", display_sets.len());

    // Find PTS collisions - multiple display sets at same PTS
    println!("\n=== PTS collisions (multiple DS at same PTS) ===");
    let mut pts_map: std::collections::BTreeMap<u64, Vec<(usize, &ParsedSegment)>> =
        std::collections::BTreeMap::new();
    for (ds_idx, ds) in display_sets.iter().enumerate() {
        for seg in &ds.segments {
            if let ParsedPayload::PresentationComposition { .. } = &seg.payload {
                pts_map.entry(seg.pts).or_default().push((ds_idx, seg));
            }
        }
    }
    let mut collision_count = 0;
    for (pts, segs) in pts_map.iter() {
        if segs.len() > 1 {
            collision_count += 1;
            println!(
                "PTS {} ({}ms) has {} display sets:",
                pts,
                pts / 90,
                segs.len()
            );
            for (ds_idx, seg) in segs {
                if let ParsedPayload::PresentationComposition {
                    composition_number,
                    state,
                    palette_update,
                    objects,
                    ..
                } = &seg.payload
                {
                    let state_str = match state {
                        CompositionState::NormalCase => "NormalCase",
                        CompositionState::AcquirePoint => "AcquirePoint",
                        CompositionState::EpochStart => "EpochStart",
                        CompositionState::EpochContinue => "EpochContinue",
                    };
                    println!(
                        "  DS {:04}: state={:?} num_obj={:02} comp_num={:04} palette_update={} [{}]",
                        ds_idx, state_str, objects.len() as u8, composition_number, palette_update,
                        objects.iter().map(|c| format!("obj{}@({},{})", c.object_id, c.x, c.y)).collect::<Vec<_>>().join(" ")
                    );
                }
            }
        }
    }
    println!("Total PTS collisions: {}", collision_count);

    // Show all EpochStart occurrences
    println!("\n=== All EpochStart PCS ===");
    for (ds_idx, ds) in display_sets.iter().enumerate() {
        for seg in &ds.segments {
            if let ParsedPayload::PresentationComposition {
                state,
                composition_number,
                palette_update,
                objects,
                ..
            } = &seg.payload
            {
                if matches!(state, CompositionState::EpochStart) {
                    println!(
                        "[{:>10}ms] DS {:04} EpochStart num_obj={:02} comp_num={:04} palette_update={} objects={}",
                        seg.pts / 90, ds_idx, objects.len() as u8, composition_number, palette_update,
                        objects.iter().map(|c| format!("obj{}@({},{})", c.object_id, c.x, c.y)).collect::<Vec<_>>().join(" ")
                    );
                }
            }
        }
    }
}
