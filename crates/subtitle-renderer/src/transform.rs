use wide::f32x4;

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
            matrix: [[c, -s, cx - c * cx + s * cy], [s, c, cy - s * cx - c * cy]],
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

                // SIMD: process all 4 RGBA channels in parallel with f32x4
                let s00_v = f32x4::from([
                    f32::from(s00[0]),
                    f32::from(s00[1]),
                    f32::from(s00[2]),
                    f32::from(s00[3]),
                ]);
                let s10_v = f32x4::from([
                    f32::from(s10[0]),
                    f32::from(s10[1]),
                    f32::from(s10[2]),
                    f32::from(s10[3]),
                ]);
                let s01_v = f32x4::from([
                    f32::from(s01[0]),
                    f32::from(s01[1]),
                    f32::from(s01[2]),
                    f32::from(s01[3]),
                ]);
                let s11_v = f32x4::from([
                    f32::from(s11[0]),
                    f32::from(s11[1]),
                    f32::from(s11[2]),
                    f32::from(s11[3]),
                ]);

                let result = s00_v * f32x4::splat(w00)
                    + s10_v * f32x4::splat(w10)
                    + s01_v * f32x4::splat(w01)
                    + s11_v * f32x4::splat(w11);

                let arr = result.to_array();
                dst[dst_idx] = arr[0].round().clamp(0.0, 255.0) as u8;
                dst[dst_idx + 1] = arr[1].round().clamp(0.0, 255.0) as u8;
                dst[dst_idx + 2] = arr[2].round().clamp(0.0, 255.0) as u8;
                dst[dst_idx + 3] = arr[3].round().clamp(0.0, 255.0) as u8;
            }
        }

        dst
    }

    #[allow(clippy::too_many_arguments)]
    /// Transform an RGBA pixmap with 3D perspective projection.
    ///
    /// Like [`apply_to_pixmap`](Self::apply_to_pixmap) but adds a perspective w-divide step before the
    /// affine inverse mapping, implementing ASS `\frx` / `\fry` rotation effects.
    ///
    /// `perspective_x` and `perspective_y` are the ASS `\frx` / `\fry` angles in
    /// degrees. `origin_x` / `origin_y` define the rotation center; when set to
    /// `0.0` they default to the center of the destination image.
    pub fn apply_with_perspective(
        &self,
        src: &[u8],
        src_w: u32,
        src_h: u32,
        dst_w: u32,
        dst_h: u32,
        perspective_x: f32,
        perspective_y: f32,
        origin_x: f32,
        origin_y: f32,
    ) -> Vec<u8> {
        let inv = match self.inverse() {
            Some(t) => t,
            None => return vec![0u8; (dst_w * dst_h * 4) as usize],
        };

        let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];
        let src_w_f = src_w as f32;
        let src_h_f = src_h as f32;

        // Perspective centre: default to image centre when origin is 0
        let cx = if origin_x != 0.0 {
            origin_x
        } else {
            dst_w as f32 * 0.5
        };
        let cy = if origin_y != 0.0 {
            origin_y
        } else {
            dst_h as f32 * 0.5
        };

        let half_dst_w = dst_w as f32 * 0.5;
        let half_dst_h = dst_h as f32 * 0.5;

        // Perspective divisor coefficients
        //   \fry → horizontal perspective (ax scales with rel_x)
        //   \frx → vertical perspective   (ay scales with rel_y)
        let ax = perspective_y.to_radians().sin() / half_dst_w;
        let ay = perspective_x.to_radians().sin() / half_dst_h;

        for dy in 0..dst_h {
            for dx in 0..dst_w {
                // 1. Relative position from rotation centre
                let rel_x = dx as f32 + 0.5 - cx;
                let rel_y = dy as f32 + 0.5 - cy;

                // 2. Perspective w-divide (inverse)
                let w_inv = 1.0 - rel_x * ax - rel_y * ay;
                let w_inv = w_inv.max(0.01); // clamp to prevent division issues

                // 3. Pre-perspective position
                let pre_x = rel_x / w_inv + cx;
                let pre_y = rel_y / w_inv + cy;

                // 4. Apply affine inverse to get source coordinate
                let (sx, sy) = inv.apply(pre_x, pre_y);
                let sx = sx - 0.5;
                let sy = sy - 0.5;

                // 5. Bounds check & bilinear interpolation (same as apply_to_pixmap)
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

                let s00_v = f32x4::from([
                    f32::from(s00[0]),
                    f32::from(s00[1]),
                    f32::from(s00[2]),
                    f32::from(s00[3]),
                ]);
                let s10_v = f32x4::from([
                    f32::from(s10[0]),
                    f32::from(s10[1]),
                    f32::from(s10[2]),
                    f32::from(s10[3]),
                ]);
                let s01_v = f32x4::from([
                    f32::from(s01[0]),
                    f32::from(s01[1]),
                    f32::from(s01[2]),
                    f32::from(s01[3]),
                ]);
                let s11_v = f32x4::from([
                    f32::from(s11[0]),
                    f32::from(s11[1]),
                    f32::from(s11[2]),
                    f32::from(s11[3]),
                ]);

                let result = s00_v * f32x4::splat(w00)
                    + s10_v * f32x4::splat(w10)
                    + s01_v * f32x4::splat(w01)
                    + s11_v * f32x4::splat(w11);

                let arr = result.to_array();
                dst[dst_idx] = arr[0].round().clamp(0.0, 255.0) as u8;
                dst[dst_idx + 1] = arr[1].round().clamp(0.0, 255.0) as u8;
                dst[dst_idx + 2] = arr[2].round().clamp(0.0, 255.0) as u8;
                dst[dst_idx + 3] = arr[3].round().clamp(0.0, 255.0) as u8;
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

#[cfg(test)]
mod tests;
