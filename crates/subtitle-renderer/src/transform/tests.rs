//! Unit tests for [`crate::transform`], kept in a separate file so the
//! production transform code stays focused on the math.
//!
//! Tests need access to private helpers (`sample_pixel`), so this is a
//! `#[cfg(test)]` submodule of `transform.rs` rather than an integration test.

use super::*;

/// Scalar reference implementation of apply_to_pixmap for verifying
/// that the SIMD-optimized version produces bit-identical results.
fn apply_to_pixmap_scalar(
    t: &AffineTransform,
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    let inv = match t.inverse() {
        Some(t) => t,
        None => return vec![0u8; (dst_w * dst_h * 4) as usize],
    };

    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
    let src_w_f = src_w as f32;
    let src_h_f = src_h as f32;

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let (sx, sy) = inv.apply(dx as f32 + 0.5, dy as f32 + 0.5);
            let sx = sx - 0.5;
            let sy = sy - 0.5;

            if sx < -1.0 || sy < -1.0 || sx >= src_w_f || sy >= src_h_f {
                continue;
            }

            let x0 = sx.floor() as i32;
            let y0 = sy.floor() as i32;
            let x1 = x0 + 1;
            let y1 = y0 + 1;

            let fx = sx - x0 as f32;
            let fy = sy - y0 as f32;

            let s00 = sample_pixel(src, src_w, src_h, x0, y0);
            let s10 = sample_pixel(src, src_w, src_h, x1, y0);
            let s01 = sample_pixel(src, src_w, src_h, x0, y1);
            let s11 = sample_pixel(src, src_w, src_h, x1, y1);

            let w00 = (1.0 - fx) * (1.0 - fy);
            let w10 = fx * (1.0 - fy);
            let w01 = (1.0 - fx) * fy;
            let w11 = fx * fy;

            let dst_idx = ((dy * dst_w + dx) * 4) as usize;
            for c in 0..4 {
                let val = f32::from(s00[c]) * w00
                    + f32::from(s10[c]) * w10
                    + f32::from(s01[c]) * w01
                    + f32::from(s11[c]) * w11;
                dst[dst_idx + c] = val.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    dst
}

#[test]
fn simd_matches_scalar_identity() {
    let w = 8u32;
    let h = 8u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            src[idx] = ((x * 32) % 256) as u8;
            src[idx + 1] = ((y * 32) % 256) as u8;
            src[idx + 2] = (x * y) as u8;
            src[idx + 3] = if (x + y) % 2 == 0 { 255 } else { 128 };
        }
    }

    let t = AffineTransform::identity();
    let simd_result = t.apply_to_pixmap(&src, w, h, w, h);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, w, h, w, h);
    assert_eq!(simd_result, scalar_result, "identity transform failed");
}

#[test]
fn simd_matches_scalar_translate() {
    let w = 8u32;
    let h = 8u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            src[idx] = 255;
            src[idx + 1] = 128;
            src[idx + 2] = 64;
            src[idx + 3] = 255;
        }
    }

    let t = AffineTransform::translate(2.5, 1.5);
    let simd_result = t.apply_to_pixmap(&src, w, h, w, h);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, w, h, w, h);
    assert_eq!(simd_result, scalar_result, "translate transform failed");
}

#[test]
fn simd_matches_scalar_rotate() {
    let w = 8u32;
    let h = 8u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            src[idx] = 255;
            src[idx + 1] = 255;
            src[idx + 2] = 255;
            src[idx + 3] = 255;
        }
    }

    let t = AffineTransform::rotate_at(30.0, 3.5, 3.5);
    let simd_result = t.apply_to_pixmap(&src, w, h, w, h);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, w, h, w, h);
    assert_eq!(simd_result, scalar_result, "rotate transform failed");
}

#[test]
fn simd_matches_scalar_scale() {
    let w = 4u32;
    let h = 4u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            src[idx] = 255;
            src[idx + 1] = 255;
            src[idx + 2] = 255;
            src[idx + 3] = 255;
        }
    }

    let t = AffineTransform::scale(2.0, 2.0);
    let simd_result = t.apply_to_pixmap(&src, w, h, 8, 8);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, w, h, 8, 8);
    assert_eq!(simd_result, scalar_result, "scale transform failed");
}

#[test]
fn simd_matches_scalar_complex() {
    let w = 6u32;
    let h = 6u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            src[idx] = (x * 51) as u8;
            src[idx + 1] = (y * 51) as u8;
            src[idx + 2] = 128;
            src[idx + 3] = if x == 0 || y == 0 { 64 } else { 255 };
        }
    }

    let t = AffineTransform::rotate_at(45.0, 2.5, 2.5)
        .then(&AffineTransform::scale(1.5, 1.5))
        .then(&AffineTransform::shear(0.1, 0.0));
    let simd_result = t.apply_to_pixmap(&src, w, h, w, h);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, w, h, w, h);
    assert_eq!(simd_result, scalar_result, "complex transform failed");
}

#[test]
fn simd_matches_scalar_out_of_bounds() {
    let w = 4u32;
    let h = 4u32;
    let src = vec![255u8; (w * h * 4) as usize];

    let t = AffineTransform::translate(100.0, 100.0);
    let simd_result = t.apply_to_pixmap(&src, w, h, w, h);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, w, h, w, h);
    assert_eq!(simd_result, scalar_result, "out-of-bounds transform failed");
    assert!(
        simd_result.iter().all(|&b| b == 0),
        "all pixels should be transparent"
    );
}

#[test]
fn simd_matches_scalar_non_square() {
    let src_w = 10u32;
    let src_h = 6u32;
    let dst_w = 12u32;
    let dst_h = 8u32;
    let mut src = vec![0u8; (src_w * src_h * 4) as usize];
    for y in 0..src_h {
        for x in 0..src_w {
            let idx = ((y * src_w + x) * 4) as usize;
            src[idx] = ((x * 25) % 256) as u8;
            src[idx + 1] = ((y * 40) % 256) as u8;
            src[idx + 2] = 100;
            src[idx + 3] = 200;
        }
    }

    let t = AffineTransform::rotate_at(15.0, 5.0, 3.0);
    let simd_result = t.apply_to_pixmap(&src, src_w, src_h, dst_w, dst_h);
    let scalar_result = apply_to_pixmap_scalar(&t, &src, src_w, src_h, dst_w, dst_h);
    assert_eq!(simd_result, scalar_result, "non-square transform failed");
}

// ── Perspective tests ────────────────────────────────────────────────

/// Create a small RGBA test image (8×8) with varied pixel data.
fn make_test_pixmap(w: u32, h: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            buf[i] = ((x * 36) % 256) as u8;
            buf[i + 1] = ((y * 36) % 256) as u8;
            buf[i + 2] = ((x + y) * 20 % 256) as u8;
            buf[i + 3] = if (x + y) % 2 == 0 { 255 } else { 200 };
        }
    }
    buf
}

#[test]
fn test_perspective_identity() {
    let w = 8u32;
    let h = 8u32;
    let src = make_test_pixmap(w, h);
    let t = AffineTransform::identity();

    let plain = t.apply_to_pixmap(&src, w, h, w, h);
    let persp = t.apply_with_perspective(&src, w, h, w, h, 0.0, 0.0, 0.0, 0.0);

    assert_eq!(
        plain, persp,
        "perspective with 0,0 angles should match identity apply_to_pixmap"
    );
}

#[test]
fn test_perspective_frx_only() {
    let w = 8u32;
    let h = 8u32;
    let src = make_test_pixmap(w, h);
    let t = AffineTransform::identity();

    let plain = t.apply_to_pixmap(&src, w, h, w, h);
    let persp = t.apply_with_perspective(&src, w, h, w, h, 45.0, 0.0, 0.0, 0.0);

    assert_ne!(
        plain, persp,
        "frx=45 should produce different output from identity"
    );
    assert!(!persp.is_empty(), "perspective output should be non-empty");
}

#[test]
fn test_perspective_fry_only() {
    let w = 8u32;
    let h = 8u32;
    let src = make_test_pixmap(w, h);
    let t = AffineTransform::identity();

    let plain = t.apply_to_pixmap(&src, w, h, w, h);
    let persp = t.apply_with_perspective(&src, w, h, w, h, 0.0, 45.0, 0.0, 0.0);

    assert_ne!(
        plain, persp,
        "fry=45 should produce different output from identity"
    );
    assert!(!persp.is_empty(), "perspective output should be non-empty");
}

#[test]
fn test_perspective_combined() {
    let w = 8u32;
    let h = 8u32;
    let src = make_test_pixmap(w, h);
    let t = AffineTransform::identity();

    let plain = t.apply_to_pixmap(&src, w, h, w, h);
    let persp = t.apply_with_perspective(&src, w, h, w, h, 30.0, 20.0, 0.0, 0.0);

    assert_ne!(
        plain, persp,
        "combined perspective should produce different output from identity"
    );
    assert!(!persp.is_empty(), "perspective output should be non-empty");
}

#[test]
fn test_perspective_extreme_angle() {
    let w = 8u32;
    let h = 8u32;
    let src = make_test_pixmap(w, h);
    let t = AffineTransform::identity();

    // Should not panic even at near-90° angles
    let persp = t.apply_with_perspective(&src, w, h, w, h, 89.0, 0.0, 0.0, 0.0);
    assert!(
        !persp.is_empty(),
        "extreme perspective output should be non-empty"
    );
    assert_eq!(
        persp.len(),
        (w * h * 4) as usize,
        "output buffer size should match"
    );
}
