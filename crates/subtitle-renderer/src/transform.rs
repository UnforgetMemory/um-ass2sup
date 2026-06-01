/// 2D affine transform using a 2x3 matrix.
///
/// Supports rotation, scaling, translation, and shear operations for subtitle
/// rendering effects like `\frz`, `\fscx`, `\fscy`, `\fax`, `\fay`.
///
/// Matrix layout: `[a, b, tx; c, d, ty]`
///
/// Transforms a point `(x, y)` as:
/// ```text
/// x' = a*x + b*y + tx
/// y' = c*x + d*y + ty
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineTransform {
    pub matrix: [[f32; 3]; 2],
}

impl AffineTransform {
    /// Create an identity transform (no-op).
    pub fn identity() -> Self {
        Self {
            matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        }
    }

    /// Translation by `(tx, ty)`.
    pub fn translate(tx: f32, ty: f32) -> Self {
        Self {
            matrix: [[1.0, 0.0, tx], [0.0, 1.0, ty]],
        }
    }

    /// Non-uniform scale by `(sx, sy)`.
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            matrix: [[sx, 0.0, 0.0], [0.0, sy, 0.0]],
        }
    }

    /// Rotation around the origin (0, 0) by `angle_deg` degrees (counter-clockwise).
    pub fn rotate(angle_deg: f32) -> Self {
        let rad = angle_deg.to_radians();
        let (s, c) = rad.sin_cos();
        Self {
            matrix: [[c, -s, 0.0], [s, c, 0.0]],
        }
    }

    /// Rotation around point `(cx, cy)` by `angle_deg` degrees.
    ///
    /// Equivalent to: translate(-cx, -cy) → rotate → translate(cx, cy).
    pub fn rotate_at(angle_deg: f32, cx: f32, cy: f32) -> Self {
        let rad = angle_deg.to_radians();
        let (s, c) = rad.sin_cos();
        // T(cx,cy) * R * T(-cx,-cy)
        Self {
            matrix: [
                [c, -s, cx - c * cx + s * cy],
                [s, c, cy - s * cx - c * cy],
            ],
        }
    }

    /// Shear by `(sx, sy)` where `sx` is horizontal shear and `sy` is vertical shear.
    pub fn shear(sx: f32, sy: f32) -> Self {
        Self {
            matrix: [[1.0, sx, 0.0], [sy, 1.0, 0.0]],
        }
    }

    /// Compose two transforms: `self * other`.
    ///
    /// The resulting transform applies `self` first, then `other`.
    /// If `self` is A and `other` is B, the result is B ∘ A,
    /// meaning for a point p: result(p) = B(A(p)).
    ///
    /// # Example
    /// ```
    /// use subtitle_renderer::transform::AffineTransform;
    /// let t = AffineTransform::translate(10.0, 20.0);
    /// let s = AffineTransform::scale(2.0, 2.0);
    /// let composed = t.then(&s);  // translate then scale
    /// ```
    pub fn then(&self, other: &AffineTransform) -> AffineTransform {
        let a = self.matrix;
        let b = other.matrix;
        AffineTransform {
            matrix: [
                [
                    b[0][0] * a[0][0] + b[0][1] * a[1][0],
                    b[0][0] * a[0][1] + b[0][1] * a[1][1],
                    b[0][0] * a[0][2] + b[0][1] * a[1][2] + b[0][2],
                ],
                [
                    b[1][0] * a[0][0] + b[1][1] * a[1][0],
                    b[1][0] * a[0][1] + b[1][1] * a[1][1],
                    b[1][0] * a[0][2] + b[1][1] * a[1][2] + b[1][2],
                ],
            ],
        }
    }

    /// Apply the forward transform to a point `(x, y)`.
    ///
    /// Returns the transformed point `(x', y')` using the 2x3 affine matrix:
    /// ```text
    /// x' = a*x + b*y + tx
    /// y' = c*x + d*y + ty
    /// ```
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let m = &self.matrix;
        (
            m[0][0] * x + m[0][1] * y + m[0][2],
            m[1][0] * x + m[1][1] * y + m[1][2],
        )
    }

    /// Compute the inverse of this transform, if it exists (determinant != 0).
    pub fn inverse(&self) -> Option<AffineTransform> {
        let m = &self.matrix;
        let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
        if det.abs() < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(AffineTransform {
            matrix: [
                [
                    m[1][1] * inv_det,
                    -m[0][1] * inv_det,
                    (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * inv_det,
                ],
                [
                    -m[1][0] * inv_det,
                    m[0][0] * inv_det,
                    (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * inv_det,
                ],
            ],
        })
    }

    /// Returns `true` if this transform is the identity (within floating-point tolerance).
    pub fn is_identity(&self) -> bool {
        let m = &self.matrix;
        (m[0][0] - 1.0).abs() < 1e-6
            && (m[0][1]).abs() < 1e-6
            && (m[0][2]).abs() < 1e-6
            && (m[1][0]).abs() < 1e-6
            && (m[1][1] - 1.0).abs() < 1e-6
            && (m[1][2]).abs() < 1e-6
    }

    /// Transform an RGBA pixmap.
    ///
    /// For each destination pixel, computes the corresponding source pixel via the
    /// inverse transform and uses bilinear interpolation for sub-pixel accuracy.
    /// Pixels mapping outside the source bounds are transparent `(0, 0, 0, 0)`.
    ///
    /// `src` is an RGBA buffer of `src_w × src_h` pixels. The output is `dst_w × dst_h`.
    pub fn apply_to_pixmap(
        &self,
        src: &[u8],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
    ) -> Vec<u8> {
        let inv = match self.inverse() {
            Some(t) => t,
            None => return vec![0u8; (dst_w * dst_h * 4) as usize],
        };

        let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
        let src_w_f = src_w as f32;
        let src_h_f = src_h as f32;

        for dy in 0..dst_h {
            for dx in 0..dst_w {
                // Map destination pixel center to source coordinates
                let (sx, sy) = inv.apply(dx as f32 + 0.5, dy as f32 + 0.5);
                // Adjust back to pixel-corner coordinates for interpolation
                let sx = sx - 0.5;
                let sy = sy - 0.5;

                // Bounds check
                if sx < -1.0 || sy < -1.0 || sx >= src_w_f || sy >= src_h_f {
                    continue;
                }

                // Bilinear interpolation
                let x0 = sx.floor() as i32;
                let y0 = sy.floor() as i32;
                let x1 = x0 + 1;
                let y1 = y0 + 1;

                let fx = sx - x0 as f32;
                let fy = sy - y0 as f32;

                // Sample the four neighbors, clamping to edge for boundary pixels
                let s00 = sample_pixel(src, src_w, src_h, x0, y0);
                let s10 = sample_pixel(src, src_w, src_h, x1, y0);
                let s01 = sample_pixel(src, src_w, src_h, x0, y1);
                let s11 = sample_pixel(src, src_w, src_h, x1, y1);

                // Weighted blend
                let w00 = (1.0 - fx) * (1.0 - fy);
                let w10 = fx * (1.0 - fy);
                let w01 = (1.0 - fx) * fy;
                let w11 = fx * fy;

                let dst_idx = ((dy * dst_w + dx) * 4) as usize;
                for c in 0..4 {
                    let val = s00[c] as f32 * w00
                        + s10[c] as f32 * w10
                        + s01[c] as f32 * w01
                        + s11[c] as f32 * w11;
                    dst[dst_idx + c] = val.round().clamp(0.0, 255.0) as u8;
                }
            }
        }

        dst
    }
}

/// Sample a pixel from an RGBA buffer, returning `(0,0,0,0)` for out-of-bounds.
/// For boundary pixels, clamps to the nearest valid coordinate.
#[inline]
fn sample_pixel(src: &[u8], w: u32, h: u32, x: i32, y: i32) -> [u8; 4] {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
        return [0, 0, 0, 0];
    }
    let idx = ((y as u32 * w + x as u32) * 4) as usize;
    [src[idx], src[idx + 1], src[idx + 2], src[idx + 3]]
}

impl Default for AffineTransform {
    fn default() -> Self {
        Self::identity()
    }
}
