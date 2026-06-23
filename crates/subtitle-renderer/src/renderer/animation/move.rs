/// Interpolate a `\move(x1, y1, x2, y2, t1, t2)` at a given elapsed time.
pub fn interpolate_move(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    t1: u64,
    t2: u64,
    elapsed: u64,
) -> (f32, f32) {
    let duration = t2.saturating_sub(t1);
    if duration == 0 {
        return (x2, y2);
    }
    let progress = ((elapsed.saturating_sub(t1)) as f32 / duration as f32).clamp(0.0, 1.0);
    (x1 + (x2 - x1) * progress, y1 + (y2 - y1) * progress)
}
