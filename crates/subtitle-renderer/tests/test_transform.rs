use subtitle_renderer::AffineTransform;

const EPSILON: f32 = 1e-4;

fn approx_eq(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < EPSILON && (a.1 - b.1).abs() < EPSILON
}

// --- identity ---

#[test]
fn test_identity_transform() {
    let t = AffineTransform::identity();
    assert!(approx_eq(t.apply(0.0, 0.0), (0.0, 0.0)));
    assert!(approx_eq(t.apply(3.5, -7.2), (3.5, -7.2)));
    assert!(t.is_identity());
}

#[test]
fn test_identity_compose() {
    let t = AffineTransform::identity().then(&AffineTransform::identity());
    assert!(t.is_identity());
}

// --- translate ---

#[test]
fn test_translate() {
    let t = AffineTransform::translate(10.0, -5.0);
    assert!(approx_eq(t.apply(0.0, 0.0), (10.0, -5.0)));
    assert!(approx_eq(t.apply(3.0, 4.0), (13.0, -1.0)));
}

// --- scale ---

#[test]
fn test_scale_2x() {
    let t = AffineTransform::scale(2.0, 2.0);
    assert!(approx_eq(t.apply(1.0, 1.0), (2.0, 2.0)));
    assert!(approx_eq(t.apply(5.0, -3.0), (10.0, -6.0)));
}

#[test]
fn test_scale_non_uniform() {
    let t = AffineTransform::scale(3.0, 0.5);
    assert!(approx_eq(t.apply(2.0, 4.0), (6.0, 2.0)));
}

// --- rotate ---

#[test]
fn test_rotate_90_degrees() {
    let t = AffineTransform::rotate(90.0);
    let (x, y) = t.apply(1.0, 0.0);
    assert!((x - 0.0).abs() < EPSILON, "x should be ~0, got {x}");
    assert!((y - 1.0).abs() < EPSILON, "y should be ~1, got {y}");
}

#[test]
fn test_rotate_180_degrees() {
    let t = AffineTransform::rotate(180.0);
    assert!(approx_eq(t.apply(1.0, 0.0), (-1.0, 0.0)));
    assert!(approx_eq(t.apply(0.0, 2.0), (0.0, -2.0)));
}

#[test]
fn test_rotate_270_degrees() {
    let t = AffineTransform::rotate(270.0);
    let (x, y) = t.apply(1.0, 0.0);
    assert!((x).abs() < EPSILON, "x should be ~0, got {x}");
    assert!((y - (-1.0)).abs() < EPSILON, "y should be ~-1, got {y}");
}

#[test]
fn test_rotate_360_is_identity() {
    let t = AffineTransform::rotate(360.0);
    assert!(t.is_identity());
}

// --- rotate_at ---

#[test]
fn test_rotate_at_point() {
    // Rotate (2, 0) by 90 degrees around (1, 0) → should give (1, 1)
    let t = AffineTransform::rotate_at(90.0, 1.0, 0.0);
    let (x, y) = t.apply(2.0, 0.0);
    assert!((x - 1.0).abs() < EPSILON, "x should be ~1, got {x}");
    assert!((y - 1.0).abs() < EPSILON, "y should be ~1, got {y}");
}

#[test]
fn test_rotate_at_center_unchanged() {
    // Rotating the center point around itself should not move it
    let t = AffineTransform::rotate_at(45.0, 5.0, 3.0);
    assert!(approx_eq(t.apply(5.0, 3.0), (5.0, 3.0)));
}

// --- shear ---

#[test]
fn test_shear_horizontal() {
    let t = AffineTransform::shear(0.5, 0.0);
    // (1, 0) → (1, 0)  (no vertical component to shift horizontally)
    assert!(approx_eq(t.apply(1.0, 0.0), (1.0, 0.0)));
    // (0, 1) → (0.5, 1)  (y component causes horizontal shift)
    assert!(approx_eq(t.apply(0.0, 1.0), (0.5, 1.0)));
}

#[test]
fn test_shear_vertical() {
    let t = AffineTransform::shear(0.0, 0.3);
    // (1, 0) → (1, 0.3)  (x component causes vertical shift)
    assert!(approx_eq(t.apply(1.0, 0.0), (1.0, 0.3)));
    // (0, 1) → (0, 1)
    assert!(approx_eq(t.apply(0.0, 1.0), (0.0, 1.0)));
}

// --- compose (then) ---

#[test]
fn test_compose_rotate_then_translate() {
    // Rotate 90° then translate by (10, 0)
    let t = AffineTransform::rotate(90.0).then(&AffineTransform::translate(10.0, 0.0));
    // (1, 0) → rotate → (0, 1) → translate → (10, 1)
    assert!(approx_eq(t.apply(1.0, 0.0), (10.0, 1.0)));
}

#[test]
fn test_compose_scale_then_rotate() {
    // Scale 2x then rotate 90°
    let t = AffineTransform::scale(2.0, 2.0).then(&AffineTransform::rotate(90.0));
    // (1, 0) → scale → (2, 0) → rotate → (0, 2)
    assert!(approx_eq(t.apply(1.0, 0.0), (0.0, 2.0)));
}

#[test]
fn test_compose_three_transforms() {
    let t = AffineTransform::translate(1.0, 0.0)
        .then(&AffineTransform::scale(2.0, 2.0))
        .then(&AffineTransform::translate(-1.0, 0.0));
    // (1, 0) → translate → (2, 0) → scale → (4, 0) → translate → (3, 0)
    assert!(approx_eq(t.apply(1.0, 0.0), (3.0, 0.0)));
}

// --- inverse ---

#[test]
fn test_inverse_identity() {
    let t = AffineTransform::identity();
    let inv = t.inverse().unwrap();
    assert!(inv.is_identity());
}

#[test]
fn test_inverse_translate() {
    let t = AffineTransform::translate(5.0, -3.0);
    let inv = t.inverse().unwrap();
    let (x, y) = inv.apply(5.0, -3.0);
    assert!((x).abs() < EPSILON);
    assert!((y).abs() < EPSILON);
}

#[test]
fn test_inverse_roundtrip() {
    let t = AffineTransform::rotate(30.0)
        .then(&AffineTransform::scale(2.0, 0.5))
        .then(&AffineTransform::translate(10.0, -5.0));
    let inv = t.inverse().unwrap();
    let original = (3.7, -2.1);
    let transformed = t.apply(original.0, original.1);
    let recovered = inv.apply(transformed.0, transformed.1);
    assert!(
        (original.0 - recovered.0).abs() < 1e-3,
        "x roundtrip failed: {} vs {}",
        original.0,
        recovered.0
    );
    assert!(
        (original.1 - recovered.1).abs() < 1e-3,
        "y roundtrip failed: {} vs {}",
        original.1,
        recovered.1
    );
}

// --- apply_to_pixmap ---

#[test]
fn test_apply_to_pixmap_identity() {
    // 4x4 image with a white pixel at (1,1)
    let w = 4u32;
    let h = 4u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    let idx = ((w + 1) * 4) as usize;
    src[idx] = 255;
    src[idx + 1] = 255;
    src[idx + 2] = 255;
    src[idx + 3] = 255;

    let t = AffineTransform::identity();
    let dst = t.apply_to_pixmap(&src, w, h, w, h);

    assert_eq!(dst, src, "identity transform should not change pixmap");
}

#[test]
fn test_apply_to_pixmap_translate() {
    let w = 4u32;
    let h = 4u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    // White pixel at (0, 0)
    src[0] = 255;
    src[1] = 255;
    src[2] = 255;
    src[3] = 255;

    // Translate by (2, 1)
    let t = AffineTransform::translate(2.0, 1.0);
    let dst = t.apply_to_pixmap(&src, w, h, w, h);

    // Pixel should now be at (2, 1)
    let dst_idx = ((w + 2) * 4) as usize;
    assert_eq!(dst[dst_idx], 255, "R at (2,1)");
    assert_eq!(dst[dst_idx + 3], 255, "A at (2,1)");

    // Original position should be transparent
    assert_eq!(dst[3], 0, "original position should be transparent");
}

#[test]
fn test_apply_to_pixmap_scale_2x() {
    // 4x4 source with a 2x2 white block at (1,1)-(2,2)
    let w = 4u32;
    let h = 4u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for dy in 0..2u32 {
        for dx in 0..2u32 {
            let idx = (((1 + dy) * w + (1 + dx)) * 4) as usize;
            src[idx] = 255;
            src[idx + 1] = 255;
            src[idx + 2] = 255;
            src[idx + 3] = 255;
        }
    }

    // Scale 2x, output 8x8
    let dst_w = 8u32;
    let dst_h = 8u32;
    let t = AffineTransform::scale(2.0, 2.0);
    let dst = t.apply_to_pixmap(&src, w, h, dst_w, dst_h);

    // Interior pixel (3,3) maps to (1.25, 1.25) in source — fully inside the 2x2 block
    let check_idx = ((3 * dst_w + 3) * 4) as usize;
    assert_eq!(
        dst[check_idx + 3],
        255,
        "interior scaled pixel at (3,3) should be fully opaque"
    );

    // Edge pixel (2,2) is blended with transparent neighbors — partial alpha
    let edge_idx = ((2 * dst_w + 2) * 4) as usize;
    assert!(
        dst[edge_idx + 3] > 0,
        "edge pixel at (2,2) should have some alpha from interpolation"
    );

    // Corner pixel at (0,0) should be transparent
    assert_eq!(dst[3], 0, "corner should be transparent");
}

#[test]
fn test_apply_to_pixmap_out_of_bounds_is_transparent() {
    let w = 4u32;
    let h = 4u32;
    let src = vec![255u8; (w * h * 4) as usize]; // fully white

    // Translate far outside
    let t = AffineTransform::translate(100.0, 100.0);
    let dst = t.apply_to_pixmap(&src, w, h, w, h);

    assert!(
        dst.iter().all(|&b| b == 0),
        "all pixels should be transparent when source is out of bounds"
    );
}

#[test]
fn test_apply_to_pixmap_rotation_quality() {
    // Place a bright pixel at (4, 0) in 8x8, rotate 90° CCW around center (3.5, 3.5)
    let w = 8u32;
    let h = 8u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    // Bright red pixel at (4, 0)
    let idx = (4 * 4) as usize; // row 0, col 4
    src[idx] = 255;
    src[idx + 1] = 0;
    src[idx + 2] = 0;
    src[idx + 3] = 255;

    // Forward: pixel center (4.5, 0.5) → rotate 90° CCW around (3.5, 3.5) → (6.5, 4.5)
    // So destination pixel (6, 4) should contain the rotated pixel
    let t = AffineTransform::rotate_at(90.0, 3.5, 3.5);
    let dst = t.apply_to_pixmap(&src, w, h, w, h);

    let check_idx = ((4 * w + 6) * 4) as usize;
    assert!(
        dst[check_idx + 3] > 0,
        "rotated pixel should appear at approximately (6, 4), got alpha={}",
        dst[check_idx + 3]
    );
}

// --- default ---

#[test]
fn test_default_is_identity() {
    let t = AffineTransform::default();
    assert!(t.is_identity());
}
